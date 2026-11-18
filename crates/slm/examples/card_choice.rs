//! Prueba: ¿el v5 elige con sentido una ficha de ocio a partir de una frase?
//! La salida está restringida por gramática a los IDs permitidos. Cada frase se
//! prueba con las fichas en dos órdenes para ver si elige por contenido o por posición.
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::time::{Duration, Instant};

use pulsys_slm::{
    CancelToken, Control, Deadline, Engine, GenerateRequest, LlamaEngine, LlamaEngineConfig,
    ModelFile, ModelManifest, ModelPackage, ModelRole, ModelSlot, ModelStore,
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
    files: HashMap<String, FileInfo>,
}

const CARDS: [(&str, &str); 4] = [
    ("READING", "read a book or a story to get absorbed in it"),
    ("MUSIC", "listen to music or calm sounds to relax"),
    (
        "PUZZLE",
        "solve a puzzle to keep the mind active and challenged",
    ),
    (
        "JOURNAL",
        "write down thoughts or feelings to sort them out",
    ),
];

const PHRASES: [(&str, &str); 12] = [
    ("No quiero leer nada, prefiero algo para escuchar", "MUSIC"),
    (
        "No me apetece pensar mucho, solo quiero oír algo bonito",
        "MUSIC",
    ),
    ("Quiero entretenerme resolviendo algo lógico", "PUZZLE"),
    ("Necesito sacar lo que siento antes de dormir", "JOURNAL"),
    ("Hoy fue un día raro y quiero dejar constancia", "JOURNAL"),
    ("Me gustaría perderme en otro mundo un rato", "READING"),
    ("Quiero algo tranquilo pero que no sea música", "READING"),
    ("Necesito despejarme con algo que me haga pensar", "PUZZLE"),
    (
        "I don't want music, I want to put my thoughts on paper",
        "JOURNAL",
    ),
    ("Something to keep my brain busy please", "PUZZLE"),
    ("Can I just relax with some tunes", "MUSIC"),
    ("I want to continue the book I started", "READING"),
];

const SYSTEM: &str = "You choose one leisure activity card for an astronaut. The input lists the allowed cards and what the astronaut said. Select exactly one card ID, the one that best fits what the astronaut said.";

fn file(runtime: &Runtime, name: &str) -> Result<ModelFile, Box<dyn Error>> {
    let info = runtime.files.get(name).ok_or("archivo ausente")?;
    Ok(ModelFile {
        name: name.to_owned(),
        bytes: info.bytes,
        sha256: info.sha256.clone(),
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let dir =
        std::path::PathBuf::from(std::env::var_os("PULSYS_V5_DIR").ok_or("falta PULSYS_V5_DIR")?);
    let runtime: Runtime = serde_json::from_str(&fs::read_to_string(dir.join("runtime.json"))?)?;
    let base = file(&runtime, &runtime.base_file)?;
    let adapter = file(&runtime, &runtime.adapter_file)?;
    let use_lora = std::env::var_os("NO_LORA").is_none();
    let package = if use_lora {
        ModelPackage::BaseWithAdapter {
            compatible_base_sha256: Some(base.sha256.clone()),
            base,
            adapter,
            lora_scale: runtime.lora_scale,
        }
    } else {
        ModelPackage::Single { file: base }
    };
    let manifest = ModelManifest {
        model_id: "space-health-v5".into(),
        role: ModelRole::Explainer,
        version: "v5".into(),
        package,
    };
    let store = ModelStore::new(&dir)?;
    let mut slot = ModelSlot::new();
    let mut load = Control::without_progress(
        CancelToken::new(),
        Deadline::after(Duration::from_secs(600)),
    );
    let engine = LlamaEngine::load_into(
        &store,
        &manifest,
        &mut slot,
        LlamaEngineConfig::default(),
        &mut load,
    )?;
    println!(
        "Modelo: v5 {}",
        if use_lora { "con LoRA" } else { "solo base" }
    );
    let mut hits = 0;
    let mut stable = 0;
    for (phrase, expected) in PHRASES {
        let mut picks = Vec::new();
        let mut seconds = 0.0;
        for reversed in [false, true] {
            let mut cards: Vec<_> = CARDS.to_vec();
            if reversed {
                cards.reverse();
            }
            let list: Vec<String> = cards
                .iter()
                .map(|(id, what)| format!("- {id}: {what}"))
                .collect();
            let ids: Vec<&str> = cards.iter().map(|(id, _)| *id).collect();
            let grammar = build_selection_grammar(&ids);
            let request = GenerateRequest {
                system: SYSTEM.into(),
                prompt: format!(
                    "Allowed cards:\n{}\n\nAstronaut said: \"{phrase}\"\n\nReturn {{\"select\": [ID]}} with exactly one card ID.",
                    list.join("\n")
                ),
                grammar: Some(grammar),
                max_tokens: 32,
                temperature: 0.0,
                ..Default::default()
            };
            let mut ctl = Control::without_progress(
                CancelToken::new(),
                Deadline::after(Duration::from_secs(60)),
            );
            let start = Instant::now();
            let raw = engine.generate(&request, &mut ctl)?;
            let chosen: serde_json::Value = serde_json::from_str(&raw)?;
            let ids: Vec<String> = chosen["select"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            picks.push(if ids.is_empty() {
                "(none)".to_owned()
            } else {
                ids.join("+")
            });
            seconds += start.elapsed().as_secs_f64();
        }
        let same = picks[0] == picks[1];
        let ok = same && picks[0] == expected;
        hits += usize::from(ok);
        stable += usize::from(same);
        println!(
            "{} {:<62} esperado {:<8} eligió {:<8} / al revés {:<8} ({:.2} s c/u)",
            if ok { "OK " } else { "-- " },
            phrase,
            expected,
            picks[0],
            picks[1],
            seconds / 2.0
        );
    }
    println!("Aciertos en ambos órdenes: {hits}/12 · misma elección en ambos órdenes: {stable}/12");
    Ok(())
}
