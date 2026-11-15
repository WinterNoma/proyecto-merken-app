//! In-process CPU inference. No HTTP client, server, remote fallback or telemetry.
use crate::contract::{self, Input, Output};
use llama_cpp_2::{
    context::params::LlamaContextParams,
    llama_batch::LlamaBatch,
    model::{LlamaModel, params::LlamaModelParams},
    sampling::LlamaSampler,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    num::NonZeroU32,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
pub const BASE: &str = "space-health-base-Q4_K_M.gguf";
pub const ADAPTER: &str = "wellbeing-v7-lora-f16.gguf";
const FILES: [(&str, u64, &str); 2] = [
    (
        BASE,
        1107409120,
        "4fce4f8510d16a058a9e6eef90d6ffd992135259e0fcadd42e69b06d1243dea1",
    ),
    (
        ADAPTER,
        34892576,
        "521290d5207de280a7fc1ebc7517138e2dd994405347cc451a16fa60d5ffc2ef",
    ),
];
pub fn verify(directory: &Path) -> Result<(), String> {
    for (name, size, sha) in FILES {
        let path = directory.join(name);
        if path
            .metadata()
            .map_err(|_| "Offline model files are missing")?
            .len()
            != size
        {
            return Err("Offline model has an incorrect size".into());
        }
        let mut file = File::open(path).map_err(|_| "Offline model cannot be read")?;
        let mut hash = Sha256::new();
        let mut buf = vec![0_u8; 512 * 1024];
        loop {
            let n = file
                .read(&mut buf)
                .map_err(|_| "Offline model cannot be read")?;
            if n == 0 {
                break;
            }
            hash.update(buf.get(..n).ok_or("Invalid read size")?);
        }
        if format!("{:x}", hash.finalize()) != sha {
            return Err("Offline model integrity check failed".into());
        }
    }
    Ok(())
}
pub fn generate(
    directory: &Path,
    input: &Input,
    cancel: &AtomicBool,
) -> Result<(Output, String, u128), String> {
    let start = Instant::now();
    let text = contract::prompt(input)?;
    // Mismo backend que el motor del servicio (v5): un segundo init fallaría.
    let backend = crate::llama::global_backend()
        .map_err(|_| "Offline runtime could not initialize".to_string())?;
    let mut model = LlamaModel::load_from_file(
        backend,
        directory.join(BASE),
        &LlamaModelParams::default().with_n_gpu_layers(0),
    )
    .map_err(|_| "Offline model could not load; check available memory")?;
    let mut adapter = model
        .lora_adapter_init(directory.join(ADAPTER))
        .map_err(|_| "Offline adapter could not load")?;
    let params = LlamaContextParams::default()
        .with_n_ctx(NonZeroU32::new(1536))
        .with_n_batch(512)
        .with_n_ubatch(256)
        .with_n_threads(4)
        .with_n_threads_batch(4);
    let mut context = model
        .new_context(backend, params)
        .map_err(|_| "Offline context could not allocate memory")?;
    context
        .lora_adapter_set(&mut adapter, 1.0)
        .map_err(|_| "Offline adapter could not be applied")?;
    let tokens = model.vocab().tokenize(text.as_bytes(), false, true);
    if tokens.is_empty() || tokens.len() + 160 > 1536 {
        return Err("Snapshot exceeds the evaluated context budget".into());
    }
    let mut batch = LlamaBatch::new(512, 1);
    let mut position = 0_i32;
    for chunk in tokens.chunks(256) {
        check(cancel, start)?;
        batch.clear();
        for token in chunk {
            batch
                .add(
                    *token,
                    position,
                    &[0],
                    usize::try_from(position).ok() == tokens.len().checked_sub(1),
                )
                .map_err(|_| "Offline input batch failed")?;
            position += 1;
        }
        context
            .decode(&mut batch)
            .map_err(|_| "Offline prompt evaluation failed")?;
    }
    let mut sampler = LlamaSampler::greedy();
    let mut bytes = Vec::new();
    let mut complete = false;
    for _ in 0..160 {
        check(cancel, start)?;
        let token = sampler.sample(&context, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.vocab().is_eog(token) {
            complete = true;
            break;
        }
        bytes.extend(model.vocab().token_to_piece(token, true, None));
        if bytes.len() > 8192 {
            return Err("Offline response exceeds its limit".into());
        }
        batch.clear();
        batch
            .add(token, position, &[0], true)
            .map_err(|_| "Offline generation batch failed")?;
        position += 1;
        context
            .decode(&mut batch)
            .map_err(|_| "Offline generation failed")?;
    }
    if !complete {
        return Err("Offline response was truncated; no recommendation accepted".into());
    }
    let raw = String::from_utf8(bytes).map_err(|_| "Offline response has invalid text encoding")?;
    let output = contract::accept(input, raw.trim())?;
    Ok((output, raw, start.elapsed().as_millis()))
}
fn check(cancel: &AtomicBool, start: Instant) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Recommendation cancelled".into());
    }
    if start.elapsed() > Duration::from_secs(180) {
        return Err("Offline generation timed out; core alerts remain available".into());
    }
    Ok(())
}
