//! Single-worker SLM service. All generated content remains a proposal.
use crate::{
    AiNote, CancelToken, Control, Deadline, Engine, Explainer, FactLine, Ficha, Interpreter,
    ModelFile, ModelManifest, ModelPackage, ModelRole, ModelSlot, ModelStore, Progress, SlmError,
    SlmExplainer, SlmInterpreter, offered_statements, render_facts,
};
use merken_core::{
    evaluate::Evaluation,
    interpret::{MAX_INPUT_CHARS, interpret},
    proposal::Proposal,
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::Duration,
};

pub const V5_ID: &str = "space-health-v5";
const BASE_HASH: &str = "4fce4f8510d16a058a9e6eef90d6ffd992135259e0fcadd42e69b06d1243dea1";
/// Trusted manifest shipped with the application, never loaded from the model directory.
pub fn v5_manifest() -> ModelManifest {
    ModelManifest {
        model_id: V5_ID.into(),
        role: ModelRole::Explainer,
        version: "v5".into(),
        package: ModelPackage::BaseWithAdapter {
            base: ModelFile {
                name: "space-health-base-Q4_K_M.gguf".into(),
                bytes: 1_107_409_120,
                sha256: BASE_HASH.into(),
            },
            adapter: ModelFile {
                name: "space-health-lora-f16.gguf".into(),
                bytes: 34_892_576,
                sha256: "ca3d2b23e39547d0ebafb2e47793b4fba3a2943978e59205328dac47f5dd750f".into(),
            },
            lora_scale: 1.0,
            compatible_base_sha256: Some(BASE_HASH.into()),
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceError {
    Busy,
    InvalidValue,
    Unavailable,
}
#[derive(Debug, Serialize)]
pub struct InterpretResponse {
    pub proposal: Proposal,
    pub layer2_proposal: Option<Proposal>,
    pub unresolved: Vec<String>,
    pub layer2: &'static str,
}
#[derive(Debug, Serialize)]
pub struct ExplainResponse {
    pub facts: Vec<FactLine>,
    pub note: Option<AiNote>,
    pub note_status: &'static str,
    pub model_id: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ModelStatus {
    pub model_id: String,
    pub installed: bool,
    pub verified: bool,
    pub loaded: bool,
    pub available: bool,
}
#[derive(Debug, Serialize)]
pub struct SlmStatus {
    pub interpreter: ModelStatus,
    pub explainer: ModelStatus,
}

/// The factory loads into the single slot, maintaining the store's verify/load lock.
pub trait EngineFactory: Send + 'static {
    type Engine: Engine + 'static;
    fn available(&self) -> bool;
    fn load(
        &mut self,
        store: &ModelStore,
        manifest: &ModelManifest,
        slot: &mut ModelSlot<Self::Engine>,
        ctl: &mut Control<'_>,
    ) -> Result<(), SlmError>;
}
pub struct NoEngineFactory;
impl EngineFactory for NoEngineFactory {
    type Engine = crate::MockEngine;
    fn available(&self) -> bool {
        false
    }
    fn load(
        &mut self,
        _: &ModelStore,
        _: &ModelManifest,
        _: &mut ModelSlot<Self::Engine>,
        _: &mut Control<'_>,
    ) -> Result<(), SlmError> {
        Err(SlmError::ModelMissing)
    }
}
#[cfg(feature = "llama")]
pub struct LlamaFactory;
#[cfg(feature = "llama")]
impl EngineFactory for LlamaFactory {
    type Engine = crate::LlamaEngine;
    fn available(&self) -> bool {
        true
    }
    fn load(
        &mut self,
        store: &ModelStore,
        manifest: &ModelManifest,
        slot: &mut ModelSlot<Self::Engine>,
        ctl: &mut Control<'_>,
    ) -> Result<(), SlmError> {
        crate::LlamaEngine::load_into(
            store,
            manifest,
            slot,
            crate::LlamaEngineConfig::default(),
            ctl,
        )
        .map(|_| ())
    }
}
struct BorrowedEngine<'a, E>(&'a mut E);
impl<E: Engine> Engine for BorrowedEngine<'_, E> {
    fn generate(
        &mut self,
        request: &crate::GenerateRequest,
        ctl: &mut Control<'_>,
    ) -> Result<String, SlmError> {
        self.0.generate(request, ctl)
    }
    fn model_id(&self) -> &str {
        self.0.model_id()
    }
}
pub type ProgressCallback = Arc<dyn Fn(Progress) + Send + Sync>;
enum Job {
    Interpret {
        text: String,
        deadline: Deadline,
        reply: mpsc::Sender<InterpretResponse>,
    },
    Explain {
        eval: Box<Evaluation>,
        fichas: Vec<Ficha>,
        deadline: Deadline,
        reply: mpsc::Sender<ExplainResponse>,
    },
    Status {
        deadline: Deadline,
        reply: mpsc::Sender<SlmStatus>,
    },
}
#[derive(Clone)]
pub struct SlmService {
    tx: SyncSender<Job>,
    current: Arc<Mutex<Option<CancelToken>>>,
}
impl SlmService {
    /// Validates the embedded curated list and starts one worker with a bounded queue.
    pub fn start<F: EngineFactory>(
        root: PathBuf,
        factory: F,
        progress: Option<ProgressCallback>,
    ) -> Result<(Self, Vec<Ficha>), SlmError> {
        Self::start_with_interpreter(root, factory, progress, None)
    }
    /// An interpreter manifest must be trusted and bundled by the application.
    pub fn start_with_interpreter<F: EngineFactory>(
        root: PathBuf,
        factory: F,
        progress: Option<ProgressCallback>,
        interpreter: Option<ModelManifest>,
    ) -> Result<(Self, Vec<Ficha>), SlmError> {
        let fichas = Ficha::vec_from_json(include_str!("../fichas/fichas.json"))?;
        let (tx, rx) = mpsc::sync_channel(2);
        let current = Arc::new(Mutex::new(None));
        let worker_current = Arc::clone(&current);
        thread::spawn(move || worker(root, factory, rx, worker_current, progress, interpreter));
        Ok((Self { tx, current }, fichas))
    }
    fn send(&self, job: Job) -> Result<(), ServiceError> {
        self.tx.try_send(job).map_err(|e| match e {
            TrySendError::Full(_) => ServiceError::Busy,
            TrySendError::Disconnected(_) => ServiceError::Unavailable,
        })
    }
    pub fn interpret(
        &self,
        text: String,
        deadline: Deadline,
    ) -> Result<Receiver<InterpretResponse>, ServiceError> {
        if text.chars().count() > MAX_INPUT_CHARS {
            return Err(ServiceError::InvalidValue);
        }
        let (reply, rx) = mpsc::channel();
        self.send(Job::Interpret {
            text,
            deadline,
            reply,
        })?;
        Ok(rx)
    }
    pub fn explain(
        &self,
        eval: Evaluation,
        fichas: Vec<Ficha>,
        deadline: Deadline,
    ) -> Result<Receiver<ExplainResponse>, ServiceError> {
        let (reply, rx) = mpsc::channel();
        self.send(Job::Explain {
            eval: Box::new(eval),
            fichas,
            deadline,
            reply,
        })?;
        Ok(rx)
    }
    pub fn status(&self, deadline: Deadline) -> Result<Receiver<SlmStatus>, ServiceError> {
        let (reply, rx) = mpsc::channel();
        self.send(Job::Status { deadline, reply })?;
        Ok(rx)
    }
    pub fn cancel(&self) -> bool {
        if let Ok(current) = self.current.lock()
            && let Some(token) = current.as_ref()
        {
            token.cancel();
            return true;
        }
        false
    }
}
fn reason(error: &SlmError) -> &'static str {
    match error {
        SlmError::Cancelled => "cancelled",
        SlmError::Timeout => "timeout",
        SlmError::InvalidOutput(_) => "discarded",
        SlmError::ModelMissing | SlmError::IncompletePair | SlmError::HashMismatch { .. } => {
            "model_unavailable"
        }
        _ => "slm_error",
    }
}
// Solo se omiten conectores aislados; la negación ("no", "not") conserva significado.
fn connector(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "y" | "e" | "pero" | "también" | "tambien" | "and" | "but" | "also"
    )
}

// Las citas no llevan offsets: se cubren todas sus apariciones literales. Los cortes
// conservan límites UTF-8 y los fragmentos devueltos son subcadenas originales.
fn unresolved_fragments(text: &str, proposal: &Proposal) -> Vec<String> {
    let mut spans = Vec::new();
    let record_quotes: Vec<&str> = proposal
        .records
        .iter()
        .map(|r| {
            // La capa 1 puede citar toda la cláusula, incluso tras una conjunción.
            // Solo recortamos si el prefijo reproduce exactamente sus campos.
            r.quote
                .match_indices(char::is_whitespace)
                .find_map(|(i, _)| {
                    let tail = r.quote.get(i..)?.trim_start();
                    let next = tail
                        .split_whitespace()
                        .next()?
                        .trim_matches(|c: char| !c.is_alphanumeric());
                    if !connector(next) {
                        return None;
                    }
                    let prefix = r.quote.get(..i)?.trim_end();
                    let parsed = interpret(prefix);
                    parsed
                        .records
                        .iter()
                        .any(|candidate| {
                            candidate.indicator == r.indicator
                                && candidate.fields == r.fields
                                && candidate.time == r.time
                        })
                        .then_some(prefix)
                })
                .unwrap_or(&r.quote)
        })
        .collect();
    let quotes = record_quotes
        .into_iter()
        .chain(
            proposal
                .records
                .iter()
                .flat_map(|r| r.fields.iter().filter_map(|f| f.quote.as_deref())),
        )
        // Una negación solo cubre texto si niega un indicador medible ("no tengo
        // fiebre"). Las que solo nombran un síntoma o una zona corporal pueden ser
        // expresiones coloquiales que la capa 1 no entiende ("no pegué ojo": "ojo"
        // es una zona): quedan en el residuo para la capa 2, cuya propuesta pasa
        // por `Proposal::check` (polaridad incluida) antes de mostrarse.
        .chain(proposal.negated.iter().map(String::as_str).filter(|n| {
            merken_core::catalog::Indicator::ALL
                .into_iter()
                .filter(|i| *i != merken_core::catalog::Indicator::Symptom)
                .any(|i| merken_core::interpret::mentions_indicator(i, n))
        }))
        .chain(proposal.uninterpreted.iter().map(String::as_str));
    for quote in quotes.filter(|q| !q.is_empty()) {
        for (start, matched) in text.match_indices(quote) {
            spans.push((start, start + matched.len()));
        }
    }
    spans.sort_unstable();
    let mut result = Vec::new();
    let mut cursor = 0;
    for (start, end) in spans
        .into_iter()
        .chain(std::iter::once((text.len(), text.len())))
    {
        if start > cursor
            && let Some(fragment) = text.get(cursor..start)
        {
            // Mantener conectores internos, pero quitar puntuación y conectores extremos.
            let words: Vec<_> = fragment
                .split_whitespace()
                .filter_map(|part| {
                    let word = part.trim_matches(|c: char| !c.is_alphanumeric());
                    (!word.is_empty()).then_some(word)
                })
                .collect();
            let first = words.iter().position(|w| !connector(w));
            let last = words.iter().rposition(|w| !connector(w));
            if let (Some(first), Some(last)) = (first, last)
                && let (Some(a), Some(b)) = (words.get(first), words.get(last))
                && let Some(a_pos) = fragment.find(*a)
                && let Some(b_pos) = fragment.rfind(*b)
                && let Some(value) = fragment.get(a_pos..b_pos + b.len())
            {
                result.push(value.to_owned());
            }
        }
        cursor = cursor.max(end);
    }
    result
}

fn filter_layer2(
    mut extra: Proposal,
    rules: &Proposal,
    unresolved: &[String],
    text: &str,
) -> Option<Proposal> {
    extra.records.retain(|record| {
        !record.quote.is_empty()
            && !rules
                .records
                .iter()
                .any(|r| r.indicator == record.indicator)
            && unresolved
                .iter()
                .chain(&rules.uninterpreted)
                .any(|fragment| fragment.contains(&record.quote))
    });
    // El texto pendiente procede de la capa 1; no presentar listas adicionales del modelo.
    extra.negated.clear();
    extra.uninterpreted.clear();
    if extra.records.is_empty() || !extra.check(text).is_empty() {
        None
    } else {
        Some(extra)
    }
}

fn worker<F: EngineFactory>(
    root: PathBuf,
    mut factory: F,
    rx: Receiver<Job>,
    current: Arc<Mutex<Option<CancelToken>>>,
    progress: Option<ProgressCallback>,
    interpreter: Option<ModelManifest>,
) {
    let mut slot = ModelSlot::<F::Engine>::new();
    let manifest = v5_manifest();
    while let Ok(job) = rx.recv() {
        let token = CancelToken::new();
        if let Ok(mut active) = current.lock() {
            *active = Some(token.clone());
        }
        let deadline = match &job {
            Job::Interpret { deadline, .. }
            | Job::Explain { deadline, .. }
            | Job::Status { deadline, .. } => deadline.clone(),
        };
        let callback = progress.clone();
        let mut ctl = Control::new(token, deadline, move |p| {
            if let Some(cb) = &callback {
                cb(p);
            }
        });
        match job {
            Job::Interpret { text, reply, .. } => {
                let proposal = interpret(&text);
                let unresolved = unresolved_fragments(&text, &proposal);
                // Un fragmento sin interpretar que solo tiene conectores o puntuación
                // ("y") no justifica cargar un modelo.
                let needed = !unresolved.is_empty()
                    || proposal.uninterpreted.iter().any(|u| {
                        u.split_whitespace().any(|part| {
                            let word = part.trim_matches(|c: char| !c.is_alphanumeric());
                            !word.is_empty() && !connector(word)
                        })
                    });
                let mut layer2 = if needed { "unavailable" } else { "not_needed" };
                let mut layer2_proposal = None;
                if needed
                    && factory.available()
                    && let Some(model) = interpreter
                        .as_ref()
                        .filter(|m| m.role == ModelRole::Interpreter)
                {
                    let loaded = slot
                        .current()
                        .is_some_and(|m| m.model_id() == model.model_id);
                    let load = if loaded {
                        Ok(())
                    } else {
                        ModelStore::new(root.join(&model.model_id))
                            .and_then(|store| factory.load(&store, model, &mut slot, &mut ctl))
                    };
                    match load {
                        Err(e) => layer2 = reason(&e),
                        Ok(()) => {
                            if let Some(engine) = slot.engine_mut() {
                                let mut l2 =
                                    SlmInterpreter::new(BorrowedEngine(engine), &model.version);
                                match l2.interpret(&text, &mut ctl) {
                                    Err(e) => layer2 = reason(&e),
                                    Ok(extra) => {
                                        let filtered =
                                            filter_layer2(extra, &proposal, &unresolved, &text);
                                        if let Some(filtered) = filtered {
                                            layer2_proposal = Some(filtered);
                                            layer2 = "used";
                                        } else {
                                            layer2 = "discarded";
                                        }
                                    }
                                }
                            } else {
                                layer2 = "slm_error";
                            }
                        }
                    }
                }
                let _ = reply.send(InterpretResponse {
                    proposal,
                    layer2_proposal,
                    unresolved,
                    layer2,
                });
            }
            Job::Explain {
                eval,
                fichas,
                reply,
                ..
            } => {
                let facts = render_facts(&eval);
                let mut result = ExplainResponse {
                    facts,
                    note: None,
                    note_status: "no_fichas",
                    model_id: None,
                };
                match offered_statements(&fichas, eval.status) {
                    Ok(offered) if offered.is_empty() => {}
                    Err(_) => result.note_status = "discarded",
                    Ok(_) if !factory.available() => result.note_status = "model_unavailable",
                    Ok(_) => {
                        let loaded = slot
                            .current()
                            .is_some_and(|m| m.model_id() == manifest.model_id);
                        let load = if loaded {
                            Ok(())
                        } else {
                            ModelStore::new(root.join(V5_ID)).and_then(|store| {
                                factory.load(&store, &manifest, &mut slot, &mut ctl)
                            })
                        };
                        match load {
                            Err(e) => result.note_status = reason(&e),
                            Ok(()) => {
                                if let Some(engine) = slot.engine_mut() {
                                    let mut explainer =
                                        SlmExplainer::new(BorrowedEngine(engine), V5_ID);
                                    match explainer.explain(&eval, &fichas, &mut ctl) {
                                        Ok(explanation) => {
                                            result.note_status = if explanation.note.is_some() {
                                                "available"
                                            } else {
                                                "discarded"
                                            };
                                            result.note = explanation.note;
                                            result.model_id = Some(V5_ID.into());
                                        }
                                        Err(e) => result.note_status = reason(&e),
                                    }
                                } else {
                                    result.note_status = "slm_error";
                                }
                            }
                        }
                    }
                }
                let _ = reply.send(result);
            }
            Job::Status { reply, .. } => {
                let dir = root.join(V5_ID);
                let installed = dir.join("space-health-base-Q4_K_M.gguf").is_file()
                    && dir.join("space-health-lora-f16.gguf").is_file();
                let verified = installed
                    && ModelStore::new(&dir)
                        .and_then(|s| s.verify(&manifest, &mut ctl))
                        .is_ok();
                let loaded = slot
                    .current()
                    .is_some_and(|m| m.model_id() == manifest.model_id);
                let interpreter_status = if let Some(m) = interpreter
                    .as_ref()
                    .filter(|m| m.role == ModelRole::Interpreter)
                {
                    let d = root.join(&m.model_id);
                    let installed = match &m.package {
                        ModelPackage::Single { file } => d.join(&file.name).is_file(),
                        ModelPackage::BaseWithAdapter { base, adapter, .. } => {
                            d.join(&base.name).is_file() && d.join(&adapter.name).is_file()
                        }
                    };
                    let verified = installed
                        && ModelStore::new(&d)
                            .and_then(|s| s.verify(m, &mut ctl))
                            .is_ok();
                    ModelStatus {
                        model_id: m.model_id.clone(),
                        installed,
                        verified,
                        loaded: slot.current().is_some_and(|v| v.model_id() == m.model_id),
                        available: factory.available(),
                    }
                } else {
                    ModelStatus {
                        model_id: "interpreter".into(),
                        installed: false,
                        verified: false,
                        loaded: false,
                        available: false,
                    }
                };
                let _ = reply.send(SlmStatus {
                    interpreter: interpreter_status,
                    explainer: ModelStatus {
                        model_id: V5_ID.into(),
                        installed,
                        verified,
                        loaded,
                        available: factory.available(),
                    },
                });
            }
        }
        if let Ok(mut active) = current.lock() {
            *active = None;
        }
    }
}
/// Standard job deadline, including time spent waiting in the queue.
pub fn default_deadline() -> Deadline {
    Deadline::after(Duration::from_secs(120))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use merken_core::{evaluate::evaluate_all, time::Timestamp};

    #[test]
    fn no_model_returns_rules_and_facts() {
        let (service, fichas) = SlmService::start(
            std::env::temp_dir().join("merken-slm-no-model"),
            NoEngineFactory,
            None,
        )
        .unwrap();
        assert!(fichas.is_empty());
        let interpretation = service
            .interpret("pulso 72 y algo desconocido".into(), default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        assert_eq!(interpretation.layer2, "unavailable");
        assert!(!interpretation.proposal.records.is_empty());
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let explanation = service
            .explain(eval, fichas, default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        assert!(!explanation.facts.is_empty());
        assert_eq!(explanation.note_status, "no_fichas");
        assert!(explanation.note.is_none());
        let status = service.status(default_deadline()).unwrap().recv().unwrap();
        assert!(!status.explainer.loaded);
        assert!(!status.explainer.available);
    }

    struct CountingFactory(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl EngineFactory for CountingFactory {
        type Engine = crate::MockEngine;
        fn available(&self) -> bool {
            true
        }
        fn load(
            &mut self,
            _: &ModelStore,
            _: &ModelManifest,
            _: &mut ModelSlot<Self::Engine>,
            _: &mut Control<'_>,
        ) -> Result<(), SlmError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(SlmError::ModelMissing)
        }
    }
    #[test]
    fn no_fichas_never_loads_engine() {
        let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (service, fichas) = SlmService::start(
            std::env::temp_dir().join("merken-slm-no-fichas"),
            CountingFactory(count.clone()),
            None,
        )
        .unwrap();
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let response = service
            .explain(eval, fichas, default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        assert_eq!(response.note_status, "no_fichas");
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn length_limit_is_reported_before_queueing() {
        let (service, _) = SlmService::start(
            std::env::temp_dir().join("merken-slm-limit"),
            NoEngineFactory,
            None,
        )
        .unwrap();
        assert!(matches!(
            service.interpret("a".repeat(MAX_INPUT_CHARS + 1), default_deadline()),
            Err(ServiceError::InvalidValue)
        ));
    }
    #[test]
    fn residual_examples() {
        let (service, _) = SlmService::start(
            std::env::temp_dir().join("merken-residual"),
            NoEngineFactory,
            None,
        )
        .unwrap();
        for (text, status, expected) in [
            ("pulso 72", "not_needed", vec![]),
            (
                "pulso 72 y no pegué ojo",
                "unavailable",
                vec!["no pegué ojo"],
            ),
            ("pulso 72, y", "not_needed", vec![]),
            // Negación de un indicador medible: cubierta, sin capa 2.
            ("pulso 72 y no tengo fiebre", "not_needed", vec![]),
        ] {
            let response = service
                .interpret(text.into(), default_deadline())
                .unwrap()
                .recv()
                .unwrap();
            assert_eq!(response.layer2, status, "{text}");
            assert_eq!(response.unresolved, expected, "{text}");
            assert_eq!(response.proposal, interpret(text));
            assert!(response.layer2_proposal.is_none());
        }
    }

    fn fixture_manifest(id: &str, role: ModelRole) -> ModelManifest {
        let file = |name: &str| ModelFile {
            name: name.into(),
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
        };
        let package = if role == ModelRole::Explainer {
            ModelPackage::BaseWithAdapter {
                base: file("model.gguf"),
                adapter: file("adapter.gguf"),
                lora_scale: 1.0,
                compatible_base_sha256: Some(file("model.gguf").sha256),
            }
        } else {
            ModelPackage::Single {
                file: file("model.gguf"),
            }
        };
        ModelManifest {
            model_id: id.into(),
            role,
            version: "test".into(),
            package,
        }
    }

    type Gate = (mpsc::Sender<()>, mpsc::Receiver<()>);
    type LoadCount = Arc<std::sync::atomic::AtomicUsize>;
    struct TestFactory {
        responses: Vec<String>,
        count: LoadCount,
        gate: Option<Gate>,
    }
    struct TestEngine {
        mock: crate::MockEngine,
        gate: Option<Gate>,
    }
    impl Engine for TestEngine {
        fn generate(
            &mut self,
            req: &crate::GenerateRequest,
            ctl: &mut Control<'_>,
        ) -> Result<String, SlmError> {
            if let Some((entered, release)) = self.gate.take() {
                let _ = entered.send(());
                let _ = release.recv();
            }
            self.mock.generate(req, ctl)
        }
        fn model_id(&self) -> &str {
            self.mock.model_id()
        }
    }
    impl EngineFactory for TestFactory {
        type Engine = TestEngine;
        fn available(&self) -> bool {
            true
        }
        fn load(
            &mut self,
            store: &ModelStore,
            manifest: &ModelManifest,
            slot: &mut ModelSlot<Self::Engine>,
            ctl: &mut Control<'_>,
        ) -> Result<(), SlmError> {
            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let fixture = fixture_manifest(&manifest.model_id, manifest.role);
            let responses = self.responses.clone();
            let mut gate = self.gate.take();
            store
                .verify_and_load(&fixture, slot, ctl, |model, _| {
                    Ok(TestEngine {
                        mock: crate::MockEngine::with_responses(responses.clone())
                            .with_model_id(model.model_id()),
                        gate: gate.take(),
                    })
                })
                .map(|_| ())
        }
    }
    fn test_service(responses: Vec<String>, gate: Option<Gate>) -> (SlmService, LoadCount) {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("merken-service-test-{}-{id}", std::process::id()));
        for model in ["test-interpreter", V5_ID] {
            let dir = root.join(model);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("model.gguf"), b"abc").unwrap();
            std::fs::write(dir.join("adapter.gguf"), b"abc").unwrap();
        }
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (service, _) = SlmService::start_with_interpreter(
            root,
            TestFactory {
                responses,
                count: count.clone(),
                gate,
            },
            None,
            Some(fixture_manifest("test-interpreter", ModelRole::Interpreter)),
        )
        .unwrap();
        (service, count)
    }

    #[test]
    fn mock_filter_and_invalid_output() {
        let input = "pulso 72 y radiación 0.42 uSv/h";
        let json = r#"{"schema_version":1,"records":[{"indicator":"space_radiation","fields":[{"key":"usv_h","value":0.42,"quote":"0.42 uSv/h"}],"time":{"kind":"unspecified"},"quote":"radiación 0.42 uSv/h"},{"indicator":"heart_rate","fields":[{"key":"bpm","value":72,"quote":"72"}],"time":{"kind":"unspecified"},"quote":"pulso 72"}],"negated":[],"uninterpreted":[]}"#;
        let (service, _) = test_service(vec![json.into(), "not json".into()], None);
        let first = service
            .interpret(input.into(), default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        assert_eq!(first.layer2, "used");
        let extra = first.layer2_proposal.unwrap();
        assert_eq!(extra.records.len(), 1);
        assert_eq!(
            extra.records[0].indicator,
            merken_core::catalog::Indicator::SpaceRadiation
        );
        assert!(extra.check(input).is_empty());
        assert_eq!(first.proposal, interpret(input));
        let second = service
            .interpret("pulso 72 y desconocido".into(), default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        assert_eq!(second.layer2, "discarded");
        assert!(second.layer2_proposal.is_none());
        // Una cita válida para la entrada, pero fuera de los fragmentos pendientes.
        let mut rules = interpret(input);
        rules.uninterpreted.clear();
        assert!(filter_layer2(extra, &rules, &["otro".into()], input).is_none());
    }

    #[test]
    fn blocked_generation_allows_cancel_and_reports_busy() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (service, _) = test_service(vec!["{}".into()], Some((entered_tx, release_rx)));
        let first = service
            .interpret("desconocido".into(), default_deadline())
            .unwrap();
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let _second = service
            .interpret("otro".into(), default_deadline())
            .unwrap();
        let _third = service.interpret("más".into(), default_deadline()).unwrap();
        assert!(matches!(
            service.interpret("llena".into(), default_deadline()),
            Err(ServiceError::Busy)
        ));
        let cancel_service = service.clone();
        assert!(
            std::thread::spawn(move || cancel_service.cancel())
                .join()
                .unwrap()
        );
        release_tx.send(()).unwrap();
        let result = first.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(result.layer2, "cancelled");
        assert_eq!(result.proposal, interpret("desconocido"));
        assert!(result.layer2_proposal.is_none());
    }

    #[test]
    fn timeout_and_engine_failure_fall_back() {
        let (service, _) = test_service(vec![], None);
        let timeout = service
            .interpret("desconocido".into(), Deadline::expired())
            .unwrap()
            .recv()
            .unwrap();
        assert_eq!(timeout.layer2, "timeout");
        let error = service
            .interpret("desconocido".into(), default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        assert_eq!(error.layer2, "slm_error");
        assert!(error.layer2_proposal.is_none());
    }

    #[test]
    fn explainer_engine_error_keeps_facts() {
        use crate::{FichaId, Statement, StatementId};
        let (service, _) = test_service(vec![], None);
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let ficha = Ficha::new(
            FichaId::new("test").unwrap(),
            vec![
                Statement::new(
                    StatementId::new("line").unwrap(),
                    "Review your recorded data.",
                    vec![eval.status],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let response = service
            .explain(eval, vec![ficha], default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        assert!(!response.facts.is_empty());
        assert!(response.note.is_none());
        assert_eq!(response.note_status, "slm_error");
    }

    #[test]
    fn switching_models_uses_one_slot() {
        use crate::{FichaId, Statement, StatementId};
        let (service, loads) = test_service(vec!["{}".into()], None);
        let eval = evaluate_all(&[], &[], Timestamp::from_millis(0))
            .into_iter()
            .next()
            .unwrap();
        let ficha = Ficha::new(
            FichaId::new("test").unwrap(),
            vec![
                Statement::new(
                    StatementId::new("line").unwrap(),
                    "Review your recorded data.",
                    vec![eval.status],
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let _ = service
            .interpret("desconocido".into(), default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        let status = service.status(default_deadline()).unwrap().recv().unwrap();
        assert!(status.interpreter.loaded && !status.explainer.loaded);
        let explanation = service
            .explain(eval, vec![ficha], default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        assert!(!explanation.facts.is_empty());
        let status = service.status(default_deadline()).unwrap().recv().unwrap();
        assert!(!status.interpreter.loaded && status.explainer.loaded);
        let _ = service
            .interpret("desconocido".into(), default_deadline())
            .unwrap()
            .recv()
            .unwrap();
        let status = service.status(default_deadline()).unwrap().recv().unwrap();
        assert!(status.interpreter.loaded && !status.explainer.loaded);
        assert_eq!(loads.load(std::sync::atomic::Ordering::SeqCst), 3);
    }
}
