//! Motor SLM real de escritorio respaldado por `llama.cpp` (`llama-cpp-2`).
//!
//! Implementa [`Engine`] sobre un [`VerifiedModel`] verificado e inmovilizado
//! por [`crate::ModelStore`] o [`crate::ModelSlot`]:
//! - Carga `Single` (solo GGUF) o `BaseWithAdapter` (base + adaptador LoRA
//!   aplicado en runtime con su `lora_scale`, sin fusionar pesos).
//! - Construye el prompt ChatML exacto de la plantilla Qwen3 con pensamiento
//!   desactivado (`<|im_start|>assistant\n<think>\n\n</think>\n\n`).
//! - Rechaza cualquier entrada cuyo total de tokens más `max_tokens` supere el
//!   tamaño de contexto configurado.
//! - Soporta gramáticas GBNF, muestreo *greedy* determinista, comprobación de
//!   cancelación/plazo en cada token y reporte de progreso.
//! - Silencia los logs del backend y nunca expone texto de error de `llama.cpp`
//!   ni contenido de prompts/salidas.

use std::num::NonZeroU32;
use std::sync::{Mutex, OnceLock};

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{LlamaLoraAdapter, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token_type::LlamaTokenAttr;
use llama_cpp_2::{LogOptions, send_logs_to_tracing};

use crate::control::{Control, Progress};
use crate::engine::{Engine, GenerateRequest};
use crate::error::{EngineErrorCode, SlmError};
use crate::manifest::{ModelManifest, VerifiedModel};
use crate::{ModelSlot, ModelStore};

/// Tamaño de contexto por defecto para el motor `llama.cpp`.
pub const DEFAULT_CONTEXT_SIZE: u32 = 1024;

/// Número de hilos por defecto para el motor `llama.cpp` en escritorio.
pub const DEFAULT_THREADS: u32 = 4;

/// Tamaño máximo de lote para evaluación del prompt.
const MAX_BATCH_SIZE: u32 = 512;

/// Parámetros de configuración del motor [`LlamaEngine`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LlamaEngineConfig {
    /// Tamaño máximo de contexto (prompt + tokens generados).
    pub n_ctx: u32,
    /// Hilos usados en generación y evaluación de lotes.
    pub n_threads: u32,
}

impl Default for LlamaEngineConfig {
    fn default() -> Self {
        Self {
            n_ctx: DEFAULT_CONTEXT_SIZE,
            n_threads: DEFAULT_THREADS,
        }
    }
}

impl LlamaEngineConfig {
    /// Crea una configuración con los valores por defecto (`1024` tokens, `4` hilos).
    pub const fn new() -> Self {
        Self {
            n_ctx: DEFAULT_CONTEXT_SIZE,
            n_threads: DEFAULT_THREADS,
        }
    }

    /// Ajusta el tamaño de contexto.
    #[must_use]
    pub const fn with_context_size(mut self, n_ctx: u32) -> Self {
        self.n_ctx = n_ctx;
        self
    }

    /// Ajusta el número de hilos de cómputo.
    #[must_use]
    pub const fn with_threads(mut self, n_threads: u32) -> Self {
        self.n_threads = n_threads;
        self
    }
}

struct LoadedLora {
    adapter: LlamaLoraAdapter,
    scale: f32,
}

struct LlamaEngineInner {
    // El destructor del modelo nativo libera sus adaptadores registrados.
    lora: Option<LoadedLora>,
    model: LlamaModel,
}

/// Motor SLM real sobre `llama.cpp` que implementa [`Engine`].
///
/// Para mantener aislamiento sin estado compartido entre solicitudes y respetar
/// la prohibición de `unsafe` en el workspace, cada llamada a [`Engine::generate`]
/// crea un contexto limpio sobre el modelo cargado y aplica el adaptador LoRA
/// si el manifiesto es `BaseWithAdapter`.
pub struct LlamaEngine {
    model_id: String,
    config: LlamaEngineConfig,
    inner: Mutex<LlamaEngineInner>,
}

static BACKEND_STATE: OnceLock<Result<LlamaBackend, EngineErrorCode>> = OnceLock::new();

fn global_backend() -> Result<&'static LlamaBackend, SlmError> {
    let state = BACKEND_STATE.get_or_init(|| {
        // Silencia todos los logs de llama.cpp y ggml para no filtrar
        // prompts, salidas ni detalles internos del backend.
        send_logs_to_tracing(LogOptions::default().with_logs_enabled(false));
        let mut backend = LlamaBackend::init().map_err(|_| EngineErrorCode::LoadFailed)?;
        backend.void_logs();
        Ok(backend)
    });

    match state {
        Ok(backend) => Ok(backend),
        Err(code) => Err(SlmError::Engine(*code)),
    }
}

impl LlamaEngine {
    /// Verifica y carga el paquete manteniendo el bloqueo hasta finalizar la fábrica.
    pub fn load_into<'a>(
        store: &ModelStore,
        manifest: &ModelManifest,
        slot: &'a mut ModelSlot<Self>,
        config: LlamaEngineConfig,
        ctl: &mut Control<'_>,
    ) -> Result<&'a mut Self, SlmError> {
        store.verify_and_load(manifest, slot, ctl, |verified, ctl| {
            Self::from_verified_with_config(verified, config, ctl)
        })
    }

    /// Carga un modelo verificado con una configuración explícita de contexto e hilos.
    fn from_verified_with_config(
        verified: &VerifiedModel,
        config: LlamaEngineConfig,
        ctl: &mut Control<'_>,
    ) -> Result<Self, SlmError> {
        ctl.check()?;

        if config.n_ctx == 0 || config.n_threads == 0 {
            return Err(SlmError::Engine(EngineErrorCode::LoadFailed));
        }

        let backend = global_backend().map_err(|_| load_error(ctl))?;

        let cancel = ctl.cancel.clone();
        let deadline = ctl.deadline.clone();
        let model_params = LlamaModelParams::default().with_progress_callback(move |_progress| {
            !cancel.is_cancelled() && !deadline.is_expired()
        });

        if let Some(adapter) = verified.adapter_file() {
            let result = crate::gguf::validate_adapter(verified.base_file().path(), adapter.path());
            ctl.check()?;
            result.map_err(|()| SlmError::Engine(EngineErrorCode::AdapterInvalid))?;
        }

        let mut model =
            match LlamaModel::load_from_file(backend, verified.base_file().path(), &model_params) {
                Ok(m) => m,
                Err(_) => {
                    ctl.check()?;
                    return Err(SlmError::Engine(EngineErrorCode::LoadFailed));
                }
            };

        ctl.check()?;

        let mut lora = match verified.adapter_file() {
            Some(adapter_file) => {
                let scale = verified.lora_scale().unwrap_or(0.0);
                if !scale.is_finite() || scale <= 0.0 {
                    return Err(SlmError::Engine(EngineErrorCode::LoadFailed));
                }
                let adapter = model
                    .lora_adapter_init(adapter_file.path())
                    .map_err(|_| load_error(ctl))?;
                Some(LoadedLora { adapter, scale })
            }
            None => None,
        };

        // Validación temprana de creación de contexto y aplicación del LoRA en el contexto
        // para garantizar que nunca se opere silenciosamente sin el adaptador.
        let ctx_params = build_context_params(config).map_err(|_| load_error(ctl))?;
        {
            let mut ctx = model
                .new_context(backend, ctx_params)
                .map_err(|_| load_error(ctl))?;
            if let Some(ref mut loaded_lora) = lora {
                ctx.lora_adapter_set(&mut loaded_lora.adapter, loaded_lora.scale)
                    .map_err(|_| load_error(ctl))?;
            }
        }

        ctl.check()?;
        Ok(Self {
            model_id: verified.model_id().to_string(),
            config,
            inner: Mutex::new(LlamaEngineInner { model, lora }),
        })
    }

    /// Devuelve la configuración activa del motor.
    pub const fn config(&self) -> LlamaEngineConfig {
        self.config
    }

    /// Formatea el prompt siguiendo exactamente la plantilla ChatML del v5 con
    /// `enable_thinking = false`.
    pub fn format_prompt(system: &str, prompt: &str) -> String {
        format!(
            "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        )
    }

    /// Verifica que una gramática GBNF compila sin errores contra el vocabulario
    /// del modelo cargado.
    pub fn validate_grammar(&mut self, grammar: &str) -> Result<(), SlmError> {
        if !crate::explain::selection::is_known_grammar(grammar) {
            return Err(SlmError::Engine(EngineErrorCode::InvalidGrammar));
        }
        crate::gbnf::validate(grammar)
            .map_err(|()| SlmError::Engine(EngineErrorCode::InvalidGrammar))?;
        let inner = self
            .inner
            .get_mut()
            .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;
        LlamaSampler::grammar(&inner.model, grammar, "root")
            .map(|_| ())
            .map_err(|_| SlmError::Engine(EngineErrorCode::InvalidGrammar))
    }
}

impl Engine for LlamaEngine {
    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn generate(
        &mut self,
        req: &GenerateRequest,
        ctl: &mut Control<'_>,
    ) -> Result<String, SlmError> {
        ctl.check()?;

        if req.temperature != 0.0 || req.max_tokens == 0 {
            return Err(SlmError::Engine(EngineErrorCode::GenerationFailed));
        }

        let backend = global_backend()?;
        let prompt_str = Self::format_prompt(&req.system, &req.prompt);

        let inner = self
            .inner
            .get_mut()
            .map_err(|_| SlmError::Engine(EngineErrorCode::LockPoisoned))?;

        let (prompt_tokens, total_needed) = {
            let vocab = inner.model.vocab();
            let add_bos = vocab.should_add_bos();
            let prompt_tokens = vocab.tokenize(prompt_str.as_bytes(), add_bos, true);

            if prompt_tokens.is_empty() {
                return Err(SlmError::Engine(EngineErrorCode::GenerationFailed));
            }

            let prompt_len_u32 = u32::try_from(prompt_tokens.len())
                .map_err(|_| SlmError::Engine(EngineErrorCode::ContextExceeded))?;
            let total_needed = prompt_len_u32
                .checked_add(req.max_tokens)
                .ok_or(SlmError::Engine(EngineErrorCode::ContextExceeded))?;
            (prompt_tokens, total_needed)
        };

        if total_needed > self.config.n_ctx {
            return Err(SlmError::Engine(EngineErrorCode::ContextExceeded));
        }

        let mut sampler = build_sampler(&inner.model, req)?;
        let ctx_params = build_context_params(self.config)?;

        let LlamaEngineInner { model, lora } = inner;
        let mut ctx = model
            .new_context(backend, ctx_params)
            .map_err(|_| SlmError::Engine(EngineErrorCode::GenerationFailed))?;

        if let Some(loaded_lora) = lora {
            ctx.lora_adapter_set(&mut loaded_lora.adapter, loaded_lora.scale)
                .map_err(|_| SlmError::Engine(EngineErrorCode::GenerationFailed))?;
        }

        let vocab = model.vocab();
        let batch_cap = usize::try_from(ctx.n_batch().max(1))
            .map_err(|_| SlmError::Engine(EngineErrorCode::GenerationFailed))?;
        let mut batch = LlamaBatch::new(batch_cap, 1);

        let last_logits_idx =
            evaluate_prompt(&mut ctx, &mut batch, &prompt_tokens, batch_cap, ctl)?;
        let mut n_cur = i32::try_from(prompt_tokens.len())
            .map_err(|_| SlmError::Engine(EngineErrorCode::ContextExceeded))?;

        let mut output_bytes = Vec::new();
        let mut current_logits_idx = last_logits_idx;

        for step in 0..req.max_tokens {
            ctl.check()?;

            // `LlamaSampler::sample` ya invoca `llama_sampler_accept` internamente.
            let token = sampler.sample(&ctx, current_logits_idx);
            let is_eog = vocab.is_eog(token);
            if is_eog {
                break;
            }

            let attrs = vocab.attr(token);
            let unexpected =
                attrs.contains(LlamaTokenAttr::Control) || attrs.contains(LlamaTokenAttr::Unknown);
            let piece = vocab.token_to_piece(token, false, None);
            append_output_piece(&mut output_bytes, &piece, unexpected)?;

            ctl.report_and_check(Progress::Generating {
                tokens: step + 1,
                max_tokens: req.max_tokens,
            })?;

            if step + 1 < req.max_tokens {
                ctl.check()?;
                batch.clear();
                batch
                    .add(token, n_cur, &[0], true)
                    .map_err(|_| SlmError::Engine(EngineErrorCode::GenerationFailed))?;
                n_cur = n_cur
                    .checked_add(1)
                    .ok_or(SlmError::Engine(EngineErrorCode::ContextExceeded))?;
                ctx.decode(&mut batch)
                    .map_err(|_| SlmError::Engine(EngineErrorCode::GenerationFailed))?;
                current_logits_idx = 0;
            }
        }

        ctl.check()?;
        decode_output(output_bytes)
    }
}

fn load_error(ctl: &Control<'_>) -> SlmError {
    match ctl.check() {
        Err(err) => err,
        Ok(()) => SlmError::Engine(EngineErrorCode::LoadFailed),
    }
}

fn append_output_piece(
    output: &mut Vec<u8>,
    piece: &[u8],
    unexpected: bool,
) -> Result<(), SlmError> {
    if unexpected {
        return Err(SlmError::Engine(EngineErrorCode::GenerationFailed));
    }
    output.extend_from_slice(piece);
    Ok(())
}

fn decode_output(bytes: Vec<u8>) -> Result<String, SlmError> {
    String::from_utf8(bytes).map_err(|_| SlmError::Engine(EngineErrorCode::GenerationFailed))
}

fn build_context_params(config: LlamaEngineConfig) -> Result<LlamaContextParams, SlmError> {
    let n_ctx =
        NonZeroU32::new(config.n_ctx).ok_or(SlmError::Engine(EngineErrorCode::LoadFailed))?;
    let n_threads = i32::try_from(config.n_threads)
        .map_err(|_| SlmError::Engine(EngineErrorCode::LoadFailed))?;
    let n_batch = config.n_ctx.clamp(1, MAX_BATCH_SIZE);

    Ok(LlamaContextParams::default()
        .with_n_ctx(Some(n_ctx))
        .with_n_batch(n_batch)
        .with_n_threads(n_threads)
        .with_n_threads_batch(n_threads))
}

fn build_sampler(model: &LlamaModel, req: &GenerateRequest) -> Result<LlamaSampler, SlmError> {
    let mut chain = Vec::new();
    if let Some(ref grammar) = req.grammar {
        if !crate::explain::selection::is_known_grammar(grammar) {
            return Err(SlmError::Engine(EngineErrorCode::InvalidGrammar));
        }
        crate::gbnf::validate(grammar)
            .map_err(|()| SlmError::Engine(EngineErrorCode::InvalidGrammar))?;
        let g = LlamaSampler::grammar(model, grammar, "root")
            .map_err(|_| SlmError::Engine(EngineErrorCode::InvalidGrammar))?;
        chain.push(g);
    }

    // Greedy es determinista; la semilla no interviene en este sampler.
    chain.push(LlamaSampler::greedy());

    Ok(LlamaSampler::chain_simple(chain))
}

fn evaluate_prompt(
    ctx: &mut llama_cpp_2::context::LlamaContext<'_>,
    batch: &mut LlamaBatch<'_>,
    prompt_tokens: &[llama_cpp_2::token::LlamaToken],
    batch_cap: usize,
    ctl: &mut Control<'_>,
) -> Result<i32, SlmError> {
    let total = prompt_tokens.len();
    let mut offset = 0usize;
    let mut last_idx = 0i32;

    for chunk in prompt_tokens.chunks(batch_cap) {
        ctl.check()?;
        batch.clear();

        let chunk_len = chunk.len();
        for (i, token) in chunk.iter().enumerate() {
            let global_pos = offset + i;
            let pos = i32::try_from(global_pos)
                .map_err(|_| SlmError::Engine(EngineErrorCode::ContextExceeded))?;
            let is_last = global_pos + 1 == total;
            batch
                .add(*token, pos, &[0], is_last)
                .map_err(|_| SlmError::Engine(EngineErrorCode::GenerationFailed))?;
        }

        ctx.decode(batch)
            .map_err(|_| SlmError::Engine(EngineErrorCode::GenerationFailed))?;

        last_idx = i32::try_from(chunk_len.saturating_sub(1))
            .map_err(|_| SlmError::Engine(EngineErrorCode::GenerationFailed))?;
        offset += chunk_len;
    }

    Ok(last_idx)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::control::{CancelToken, Deadline};
    use crate::manifest::{ModelFile, ModelManifest, ModelPackage, ModelRole};
    use crate::store::ModelStore;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "merken_slm_llama_test_{tag}_{}_{}",
            std::process::id(),
            nanos
        ));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        let mut out = String::with_capacity(64);
        for b in digest {
            use std::fmt::Write;
            let _ = write!(&mut out, "{b:02x}");
        }
        out
    }

    #[test]
    fn output_utf8_may_span_tokens_but_must_be_complete() {
        let mut bytes = Vec::new();
        append_output_piece(&mut bytes, &[0xe2, 0x82], false).unwrap();
        append_output_piece(&mut bytes, &[0xac], false).unwrap();
        assert_eq!(decode_output(bytes).unwrap(), "€");
        assert_eq!(
            decode_output(vec![0xe2, 0x82]),
            Err(SlmError::Engine(EngineErrorCode::GenerationFailed))
        );
        assert_eq!(
            append_output_piece(&mut Vec::new(), b"hidden", true),
            Err(SlmError::Engine(EngineErrorCode::GenerationFailed))
        );
    }

    #[test]
    fn format_prompt_matches_qwen3_chatml_no_think() {
        let formatted = LlamaEngine::format_prompt("SYS", "USR");
        assert_eq!(
            formatted,
            "<|im_start|>system\nSYS<|im_end|>\n<|im_start|>user\nUSR<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
    }

    #[test]
    fn config_defaults_match_v5_runtime() {
        let cfg = LlamaEngineConfig::default();
        assert_eq!(cfg.n_ctx, 1024);
        assert_eq!(cfg.n_threads, 4);

        let custom = cfg.with_context_size(2048).with_threads(8);
        assert_eq!(custom.n_ctx, 2048);
        assert_eq!(custom.n_threads, 8);
    }

    #[test]
    fn corrupt_gguf_file_fails_with_load_failed() {
        let dir = temp_dir("corrupt_gguf");
        let model_bytes = b"not a real gguf file";
        let file_path = dir.join("fake-base.gguf");
        fs::write(&file_path, model_bytes).expect("write fake model");

        let manifest = ModelManifest {
            model_id: "fake-v5".to_string(),
            role: ModelRole::Interpreter,
            version: "1".to_string(),
            package: ModelPackage::Single {
                file: ModelFile {
                    name: "fake-base.gguf".to_string(),
                    sha256: sha256_hex(model_bytes),
                    bytes: u64::try_from(model_bytes.len()).expect("len"),
                },
            },
        };

        let store = ModelStore::new(&dir).expect("store");
        let cancel = CancelToken::new();
        let mut ctl = Control::new(cancel, Deadline::after(Duration::from_secs(30)), |_| {});
        let mut slot = ModelSlot::new();
        let err = match LlamaEngine::load_into(
            &store,
            &manifest,
            &mut slot,
            LlamaEngineConfig::default(),
            &mut ctl,
        ) {
            Ok(_) => panic!("expected invalid gguf to fail"),
            Err(e) => e,
        };
        assert_eq!(err, SlmError::Engine(EngineErrorCode::LoadFailed));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn pre_cancelled_control_aborts_before_loading_file() {
        let dir = temp_dir("cancelled");
        let model_bytes = b"not a real gguf file";
        let file_path = dir.join("fake-base.gguf");
        fs::write(&file_path, model_bytes).expect("write fake model");

        let manifest = ModelManifest {
            model_id: "fake-v5".to_string(),
            role: ModelRole::Interpreter,
            version: "1".to_string(),
            package: ModelPackage::Single {
                file: ModelFile {
                    name: "fake-base.gguf".to_string(),
                    sha256: sha256_hex(model_bytes),
                    bytes: u64::try_from(model_bytes.len()).expect("len"),
                },
            },
        };

        let store = ModelStore::new(&dir).expect("store");
        let cancel = CancelToken::new();
        let mut slot = ModelSlot::new();
        cancel.cancel();
        let mut ctl = Control::new(cancel, Deadline::after(Duration::from_secs(30)), |_| {});

        let err = match LlamaEngine::load_into(
            &store,
            &manifest,
            &mut slot,
            LlamaEngineConfig::default(),
            &mut ctl,
        ) {
            Ok(_) => panic!("expected cancelled error"),
            Err(e) => e,
        };
        assert_eq!(err, SlmError::Cancelled);

        let expired_cancel = CancelToken::new();
        let mut expired_ctl = Control::new(expired_cancel, Deadline::expired(), |_| {});
        let err = match LlamaEngine::load_into(
            &store,
            &manifest,
            &mut slot,
            LlamaEngineConfig::default(),
            &mut expired_ctl,
        ) {
            Ok(_) => panic!("expected timeout error"),
            Err(e) => e,
        };
        assert_eq!(err, SlmError::Timeout);

        let _ = fs::remove_dir_all(&dir);
    }
}
