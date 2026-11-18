//! # pulsys-sync
//!
//! Cliente de la API de sincronización (RF-20 a RF-22): subida idempotente
//! por lotes y descarga de análisis por cursor. El móvil **nunca** habla
//! directamente con MySQL; las credenciales solo las tiene la API.
//!
//! Por ahora solo tiene el cliente de la **API de simulación** (datos
//! inventados, aparte del backend real): ver [`simulation`].

pub mod simulation;

use serde::Serialize;

/// Error de red o de la API, con un `code` estable para la interfaz.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SyncError {
    /// `network`, `unauthorized`, `not_found`, `http_error`, `bad_response`,
    /// `invalid_value` o `missing_token`.
    pub code: String,
    pub message: String,
}

impl SyncError {
    pub(crate) fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for SyncError {}
