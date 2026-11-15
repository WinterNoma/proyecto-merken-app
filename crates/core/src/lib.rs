//! # merken-core
//!
//! Dominio y evaluación de Merken. **Puro y determinista** (RNF-03, RNF-04):
//! sin E/S, sin reloj del sistema y sin nada que pueda entrar en pánico (RNF-01).

pub mod telemetry;

pub use telemetry::{AstronautTelemetry, BloodPressure};

/// Versión del núcleo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Versión del conjunto de reglas.
pub const RULES_VERSION: &str = "rules-2026.11.14";

pub mod catalog;
pub mod units;

use serde::Serialize;

/// Categoría de un error de validación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidValue,
    InvalidSchema,
}

/// Error de validación expresado para el consumidor de la API.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValidationError {
    pub code: ErrorCode,
    pub field: String,
    pub message: String,
}

/// Comprueba el límite de caracteres Unicode de un texto.
pub fn validate_text(field: &str, text: &str, max_chars: usize) -> Result<(), ValidationError> {
    if text.chars().count() > max_chars {
        return Err(ValidationError {
            code: ErrorCode::InvalidValue,
            field: field.to_owned(),
            message: format!("Text must contain at most {max_chars} characters"),
        });
    }
    Ok(())
}

pub mod effective;
pub mod evaluate;
pub mod interpret;
pub mod measurement;
pub mod plausibility;
pub mod proposal;
pub mod record;
pub mod rules;
pub mod time;

#[cfg(test)]
mod tests;
