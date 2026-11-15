//! # merken-slm
//!
//! Asistente con modelos pequeños en el dispositivo, detrás de un trait para
//! poder cambiar de motor (llama.cpp, candle, ONNX Runtime):
//! - **interpretar** texto → propuesta de registro con salida restringida por
//!   gramática (capa 2 de RF-14); el usuario la confirma;
//! - **explicar** una evaluación ya decidida por `merken-core` (RF-15),
//!   seleccionando y ordenando únicamente frases curadas autorizadas por ID;
//! - **controlar** cancelación, progreso y plazo máximo (RNF-08), integridad y
//!   exclusión en almacén de modelos (RNF-13) y ranura única en memoria (RNF-09).
//!
//! El SLM ayuda, no decide: nunca asigna estados, ni recomienda tratamientos,
//! ni redacta texto libre en la nota clínica.

pub mod control;
pub mod engine;
pub mod error;
pub mod explain;
#[cfg(any(feature = "llama", test))]
mod gbnf;
#[cfg(any(feature = "llama", test))]
mod gguf;
pub mod interpreter;
#[cfg(feature = "llama")]
pub mod llama;
pub mod manifest;
pub mod slot;
pub mod store;

pub use control::{CancelToken, Control, Deadline, Progress};
pub use engine::{
    DEFAULT_MAX_TOKENS, DEFAULT_SEED, DEFAULT_TEMPERATURE, Engine, GenerateRequest, MockEngine,
};
pub use error::{EngineErrorCode, OutputIssue, SlmError};
pub use explain::{
    AI_SUGGESTION_LABEL, AiNote, Explainer, Explanation, FactId, FactLine, Ficha, FichaId,
    MAX_ID_LEN, MAX_SELECTED_STATEMENTS, MAX_SELECTION_OUTPUT_BYTES, MAX_STATEMENT_CHARS,
    OfferedStatement, PlainEnglishPromptFormat, PromptFormat, RenderedStatement, SlmExplainer,
    Statement, StatementId, build_selection_grammar, offered_statements, render_facts,
    validate_selection,
};
pub use interpreter::{
    Interpreter, MAX_FIELDS_PER_RECORD, MAX_INTERPRETER_OUTPUT_BYTES, MAX_PROPOSAL_RECORDS,
    MAX_QUOTE_BYTES, MAX_QUOTE_ITEMS, PROPOSAL_GBNF, SlmInterpreter,
};
#[cfg(feature = "llama")]
pub use llama::{DEFAULT_CONTEXT_SIZE, DEFAULT_THREADS, LlamaEngine, LlamaEngineConfig};
pub use manifest::{
    ModelFile, ModelManifest, ModelPackage, ModelRole, VerifiedFile, VerifiedModel,
};
pub use slot::ModelSlot;
pub use store::ModelStore;
