//! Tipos de error y problemas de validación de salida para `merken-slm`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Problema concreto detectado al validar la salida de un modelo SLM.
///
/// Por privacidad, `code` y `detail` contienen únicamente códigos y mensajes
/// constantes: nunca fragmentos de la salida del modelo, texto del usuario,
/// cifras clínicas ni mensajes de Serde.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputIssue {
    /// Código corto y estable del problema (p. ej. `invalid_json`, `unknown_citation`).
    pub code: String,
    /// Descripción constante en inglés del problema detectado, sin exponer datos sensibles.
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

/// Código constante de error interno del motor de inferencia.
///
/// Nunca contiene texto libre del backend ni fragmentos clínicos o del usuario.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineErrorCode {
    /// Fallo al inicializar o cargar el modelo en el motor.
    LoadFailed,
    /// El adaptador LoRA no cumple los requisitos del paquete.
    AdapterInvalid,
    /// Fallo durante la generación de salida en el motor.
    GenerationFailed,
    /// La gramática GBNF solicitada no es válida para el vocabulario del modelo.
    InvalidGrammar,
    /// El prompt más `max_tokens` supera el tamaño máximo de contexto configurado.
    ContextExceeded,
    /// El motor no devolvió ninguna respuesta (p. ej. cola vacía en pruebas).
    EmptyResponse,
    /// Error de sincronización interna al adquirir el bloqueo del almacén de modelos.
    LockPoisoned,
    /// Adquisición reentrante incompatible del bloqueo del almacén desde el mismo hilo.
    LockReentrant,
    /// Fallo interno no clasificado del motor.
    Internal,
}

impl EngineErrorCode {
    /// Representación estable en `snake_case` del código de error del motor.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadFailed => "load_failed",
            Self::AdapterInvalid => "adapter_invalid",
            Self::GenerationFailed => "generation_failed",
            Self::InvalidGrammar => "invalid_grammar",
            Self::ContextExceeded => "context_exceeded",
            Self::EmptyResponse => "empty_response",
            Self::LockPoisoned => "lock_poisoned",
            Self::LockReentrant => "lock_reentrant",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for EngineErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
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
    /// Error interno del motor de inferencia identificado mediante código constante.
    Engine(EngineErrorCode),
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
            Self::Engine(code) => write!(f, "SLM engine error: {code}"),
            Self::Io(msg) => write!(f, "Model I/O error: {msg}"),
        }
    }
}

impl std::error::Error for SlmError {}

fn io_error_message(kind: std::io::ErrorKind) -> &'static str {
    match kind {
        std::io::ErrorKind::NotFound => "file or directory not found",
        std::io::ErrorKind::PermissionDenied => "permission denied",
        std::io::ErrorKind::AlreadyExists => "already exists",
        std::io::ErrorKind::InvalidInput => "invalid input",
        std::io::ErrorKind::UnexpectedEof => "unexpected end of file",
        _ => "i/o operation failed",
    }
}

impl From<std::io::Error> for SlmError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(io_error_message(err.kind()).to_owned())
    }
}
