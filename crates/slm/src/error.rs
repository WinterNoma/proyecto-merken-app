//! Tipos de error y problemas de validación de salida para `merken-slm`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Problema concreto detectado al validar la salida de un modelo SLM.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputIssue {
    /// Código corto y estable del problema (p. ej. `invalid_json`, `unknown_citation`).
    pub code: String,
    /// Descripción en inglés del problema detectado, sin exponer datos clínicos sensibles.
    pub detail: String,
}

impl OutputIssue {
    /// Construye un nuevo problema de salida.
    pub fn new(code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
        }
    }
}

impl fmt::Display for OutputIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

/// Errores del subsistema SLM local.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlmError {
    /// No hay archivos de modelo disponibles en el directorio indicado.
    ModelMissing,
    /// El tamaño o el resumen SHA-256 de un archivo no coincide con el manifiesto.
    HashMismatch { file: String },
    /// Falta uno de los archivos de un par obligatorio (p. ej. base + LoRA del v5).
    IncompletePair,
    /// La ejecución fue cancelada mediante [`crate::CancelToken`].
    Cancelled,
    /// Se superó el plazo máximo definido en [`crate::Deadline`].
    Timeout,
    /// La salida del modelo incumplió el contrato o la validación posterior.
    InvalidOutput(Vec<OutputIssue>),
    /// Operación, formato o ruta no soportados.
    Unsupported(String),
    /// Error interno del motor de inferencia.
    Engine(String),
    /// Error de entrada/salida al acceder a los archivos del modelo.
    Io(String),
}

impl fmt::Display for SlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelMissing => write!(f, "Model files are missing"),
            Self::HashMismatch { file } => write!(f, "Model file integrity check failed: {file}"),
            Self::IncompletePair => write!(f, "Incomplete model pair: required files are missing"),
            Self::Cancelled => write!(f, "Model execution was cancelled"),
            Self::Timeout => write!(f, "Model execution timed out"),
            Self::InvalidOutput(issues) => {
                write!(
                    f,
                    "Model output failed validation ({} issue(s))",
                    issues.len()
                )
            }
            Self::Unsupported(msg) => write!(f, "Unsupported SLM operation: {msg}"),
            Self::Engine(msg) => write!(f, "SLM engine error: {msg}"),
            Self::Io(msg) => write!(f, "Model I/O error: {msg}"),
        }
    }
}

impl std::error::Error for SlmError {}

impl From<std::io::Error> for SlmError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}
