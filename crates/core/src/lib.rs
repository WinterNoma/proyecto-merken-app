//! # merken-core
//!
//! Dominio y evaluación de Merken. **Puro y determinista** (RNF-03, RNF-04):
//! sin E/S, sin reloj del sistema y sin nada que pueda entrar en pánico (RNF-01).

pub mod telemetry;

pub use telemetry::{AstronautTelemetry, BloodPressure};

/// Versión del núcleo, para trazar qué reglas produjeron cada evaluación.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
