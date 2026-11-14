//! # merken-core
//!
//! Dominio y evaluación de Merken. **Puro y determinista** (RNF-03, RNF-04):
//! sin E/S, sin reloj del sistema y sin nada que pueda entrar en pánico (RNF-01).
//!
//! Aquí irán:
//! - tipos de unidades y conceptos (RNF-02),
//! - referencia personal (RF-08),
//! - estados `SinEvaluacion` / `Estable` / `CambioReferencia` / `Alarma` (RF-09),
//! - reglas versionadas con su fuente (RF-11),
//! - intérprete determinista de texto, capa 1 de RF-14.
//!
//! Requisitos: `docs/requisitos-app-movil.md`.

/// Versión del núcleo, para trazar qué reglas produjeron cada evaluación.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
