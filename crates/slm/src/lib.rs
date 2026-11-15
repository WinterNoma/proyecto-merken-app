//! # merken-slm
//!
//! Asistente con modelos pequeños en el dispositivo, detrás de un trait para
//! poder cambiar de motor (llama.cpp, candle, ONNX Runtime):
//! - **interpretar** texto → propuesta de registro con salida restringida por
//!   gramática (capa 2 de RF-14); el usuario la confirma;
//! - **explicar** una evaluación ya decidida por `merken-core` (RF-15);
//! - **controlar** cancelación, progreso y plazo máximo (RNF-08), integridad de
//!   manifiestos (RNF-13) y ranura única en memoria (RNF-09).
//!
//! El SLM ayuda, no decide: nunca asigna estados ni recomienda tratamientos.
//! Si su salida no pasa la validación, se descarta.

pub mod control;
pub mod engine;
pub mod error;
pub mod explain;
pub mod interpreter;
pub mod manifest;
pub mod slot;

pub use control::{CancelToken, Control, Deadline, Progress};
pub use engine::{
    DEFAULT_MAX_TOKENS, DEFAULT_SEED, DEFAULT_TEMPERATURE, Engine, GenerateRequest, MockEngine,
};
pub use error::{OutputIssue, SlmError};
pub use explain::{
    AI_SUGGESTION_LABEL, AiNote, Document, Explainer, Explanation, FactId, FactLine,
    MAX_NOTE_CHARS, MAX_NOTE_SENTENCES, MAX_WORDS_PER_SENTENCE, PlainEnglishPromptFormat,
    PromptFormat, SlmExplainer, render_facts, validate_note,
};
pub use interpreter::{Interpreter, PROPOSAL_GBNF, SlmInterpreter};
pub use manifest::{
    ModelFile, ModelManifest, ModelPackage, ModelRole, VerifiedFile, VerifiedModel,
};
pub use slot::ModelSlot;
