//! Compara el motor local con las predicciones guardadas del paquete v5.
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::time::{Duration, Instant};

use merken_slm::{
    CancelToken, Control, Deadline, Engine, GenerateRequest, LlamaEngine, ModelFile, ModelManifest,
    ModelPackage, ModelRole, ModelSlot, ModelStore, PROPOSAL_GBNF, Progress,
    build_selection_grammar,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct FileInfo {
    bytes: u64,
    sha256: String,
}

#[derive(Deserialize)]
struct Runtime {
    base_file: String,
    adapter_file: String,
    lora_scale: f32,
    context_size: u32,
    max_new_tokens: u32,
    temperature: f32,
    files: HashMap<String, FileInfo>,
}

#[derive(Deserialize)]
struct Case {
    model_input: String,
    prediction: String,
}

fn model_file(runtime: &Runtime, name: &str) -> Result<ModelFile, Box<dyn Error>> {
    let info = runtime
        .files
        .get(name)
        .ok_or("archivo ausente de runtime.json")?;
    Ok(ModelFile {
        name: name.to_owned(),
        bytes: info.bytes,
        sha256: info.sha256.clone(),
    })
}

fn limit_arg() -> Result<usize, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    match args.next() {
        None => Ok(40),
        Some(flag) if flag == "--limit" => {
            let limit: usize = args.next().ok_or("falta N en --limit")?.parse()?;
            if limit == 0 || args.next().is_some() {
                return Err("uso: v5_parity [--limit N], N > 0".into());
            }
            Ok(limit)
        }
        Some(_) => Err("uso: v5_parity [--limit N]".into()),
    }
}

fn normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn percentile(sorted: &[f64], numerator: usize) -> Option<f64> {
    let rank = sorted.len().checked_mul(numerator)?.div_ceil(100);
    sorted.get(rank.saturating_sub(1)).copied()
}

fn run() -> Result<(), Box<dyn Error>> {
    let limit = limit_arg()?;
    let dir =
        std::path::PathBuf::from(std::env::var_os("MERKEN_V5_DIR").ok_or("falta MERKEN_V5_DIR")?);
    let runtime: Runtime = serde_json::from_str(&fs::read_to_string(dir.join("runtime.json"))?)?;
    if runtime.temperature != 0.0 || runtime.context_size != 1024 {
        return Err("runtime v5 no coincide con la configuración esperada".into());
    }
    let base = model_file(&runtime, &runtime.base_file)?;
    let adapter = model_file(&runtime, &runtime.adapter_file)?;
    let manifest = ModelManifest {
        model_id: "space-health-v5".into(),
        role: ModelRole::Explainer,
        version: "v5".into(),
        package: ModelPackage::BaseWithAdapter {
            compatible_base_sha256: Some(base.sha256.clone()),
            base,
            adapter,
            lora_scale: runtime.lora_scale,
        },
    };
    let system = fs::read_to_string(dir.join("system.txt"))?;
    let data = fs::read_to_string(dir.join("evaluation/eval_gguf_final_adaptado.jsonl"))?;
    let cases: Vec<Case> = data
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    if cases.len() != 40 {
        return Err("la evaluación debe contener 40 casos".into());
    }
    let store = ModelStore::new(&dir)?;
    let mut slot = ModelSlot::new();
    let mut load_ctl = Control::without_progress(
        CancelToken::new(),
        Deadline::after(Duration::from_secs(600)),
    );
    let load_start = Instant::now();
    let engine = store.verify_and_load(
        &manifest,
        &mut slot,
        &mut load_ctl,
        LlamaEngine::from_verified,
    )?;
    println!(
        "Verificación y carga: {:.3} s",
        load_start.elapsed().as_secs_f64()
    );

    let mut exact = 0_usize;
    let mut normal = 0_usize;
    let mut latencies = Vec::new();
    let mut total_tokens = 0_u64;
    let mut total_seconds = 0.0;
    for (index, case) in cases.iter().take(limit).enumerate() {
        let mut tokens = 0_u32;
        let req = GenerateRequest {
            system: system.clone(),
            prompt: case.model_input.clone(),
            max_tokens: runtime.max_new_tokens,
            ..Default::default()
        };
        let start = Instant::now();
        let output = {
            let mut ctl = Control::new(
                CancelToken::new(),
                Deadline::after(Duration::from_secs(120)),
                |progress| {
                    if let Progress::Generating { tokens: count, .. } = progress {
                        tokens = count;
                    }
                },
            );
            engine.generate(&req, &mut ctl)?
        };
        let seconds = start.elapsed().as_secs_f64();
        let is_exact = output == case.prediction;
        let is_normal = normalized(&output) == normalized(&case.prediction);
        exact += usize::from(is_exact);
        normal += usize::from(is_normal);
        total_tokens += u64::from(tokens);
        total_seconds += seconds;
        latencies.push(seconds);
        println!(
            "Caso {:02}: {:.3} s, {} tokens, exacta={}, normalizada={}",
            index + 1,
            seconds,
            tokens,
            is_exact,
            is_normal
        );
    }
    latencies.sort_by(f64::total_cmp);
    let count = latencies.len();
    println!(
        "Coincidencias exactas: {exact}/{count}; normalizadas: {normal}/{count} (total del conjunto: 40)"
    );
    println!(
        "Latencia p50/p95: {:.3}/{:.3} s",
        percentile(&latencies, 50).ok_or("sin casos")?,
        percentile(&latencies, 95).ok_or("sin casos")?
    );
    println!(
        "Tokens generados/s: {:.2}",
        total_tokens as f64 / total_seconds
    );
    println!("Memoria pico del proceso: no disponible con esta implementación segura y portable");

    let selection = build_selection_grammar(&["example"]);
    for (name, grammar) in [
        ("proposal", PROPOSAL_GBNF),
        ("selection", selection.as_str()),
    ] {
        engine.validate_grammar(grammar)?;
        let request = GenerateRequest {
            system: "Return only a valid JSON object matching the requested schema.".into(),
            prompt: if name == "proposal" {
                "Return an empty proposal."
            } else {
                "Return an empty selection."
            }
            .into(),
            grammar: Some(grammar.to_owned()),
            max_tokens: runtime.max_new_tokens,
            ..Default::default()
        };
        let mut ctl = Control::without_progress(
            CancelToken::new(),
            Deadline::after(Duration::from_secs(120)),
        );
        let output = engine.generate(&request, &mut ctl)?;
        let parsed: serde_json::Value = serde_json::from_str(&output)?;
        if !parsed.is_object() {
            return Err("la salida restringida no es un objeto JSON".into());
        }
        println!("Gramática {name}: inicializada y JSON generado correctamente");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    run()
}
