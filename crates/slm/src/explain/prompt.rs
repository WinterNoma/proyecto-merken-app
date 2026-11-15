//! Construcción del prompt para el explicador SLM (RF-15, RNF-14).
//!
//! Define el trait [`PromptFormat`] para desacoplar el formato de selección de Fase 0
//! en inglés del contrato `source`/`motor`/`documents` previsto para Fase 1.

use crate::explain::{facts::FactLine, selection::OfferedStatement};
use merken_core::evaluate::Evaluation;

const PLAIN_ENGLISH_SYSTEM_PROMPT: &str = "\
You assist an astronaut health evaluation by selecting up to 3 of the authorized statements below (RF-15, RNF-14).
Strict rules:
1. Return ONLY valid JSON matching {\"select\": [\"<ficha>/<statement>\", ...]} with 0 to 3 IDs.
2. Only choose IDs listed under Offered Statements, ordered from most useful to least useful.
3. Never invent IDs, repeat IDs, or write any free-form text or extra JSON fields.
4. If none of the offered statements are helpful for this evaluation, return {\"select\": []}.";

/// Estrategia intercambiable de formateo de prompt para [`crate::explain::SlmExplainer`].
///
/// Permite añadir en Fase 1 el formato `source`/`motor`/`documents` del SLM v5 sin
/// modificar el bucle de ejecución ni la validación de selección cerrada.
pub trait PromptFormat: Send + Sync {
    /// Prompt de sistema enviado al motor de inferencia.
    fn system_prompt(&self) -> &str;

    /// Construye el prompt de usuario en inglés con los hechos deterministas y las frases ofrecidas.
    fn format_prompt(
        &self,
        eval: &Evaluation,
        facts: &[FactLine],
        offered: &[OfferedStatement],
    ) -> String;
}

/// Formato por defecto en inglés llano que lista hechos y frases ofrecidas con su ID.
#[derive(Clone, Copy, Debug, Default)]
pub struct PlainEnglishPromptFormat;

impl PromptFormat for PlainEnglishPromptFormat {
    fn system_prompt(&self) -> &str {
        PLAIN_ENGLISH_SYSTEM_PROMPT
    }

    fn format_prompt(
        &self,
        _eval: &Evaluation,
        facts: &[FactLine],
        offered: &[OfferedStatement],
    ) -> String {
        let mut out = String::from("Facts:\n");
        for fact in facts {
            out.push_str("- ");
            out.push_str(&fact.text);
            out.push('\n');
        }

        if offered.is_empty() {
            out.push_str("Offered statements: none\n");
        } else {
            out.push_str("Offered statements:\n");
            for stmt in offered {
                out.push_str("- [");
                out.push_str(&stmt.selection_id);
                out.push_str("] ");
                out.push_str(&stmt.text);
                out.push('\n');
            }
        }

        out.push_str(
            "Task: Return ONLY JSON {\"select\": [...]} choosing the 0 to 3 most useful offered statement IDs for these facts, or [] if none apply.\n",
        );
        out
    }
}
