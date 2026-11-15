//! Construcción del prompt para el explicador SLM (RF-15, RNF-14).
//!
//! Define el trait [`PromptFormat`] para desacoplar el formato de Fase 0 en inglés llano
//! del contrato `source`/`motor`/`documents` previsto para el modelo v5 en Fase 1.

use crate::explain::{facts::FactLine, note::Document};
use merken_core::evaluate::Evaluation;

const PLAIN_ENGLISH_SYSTEM_PROMPT: &str = "\
You write an optional plain-English summary note for a pre-computed astronaut health evaluation (RNF-14).
Strict rules:
1. Write 1 to 3 short sentences in English only (at most 350 characters total and 30 words per sentence).
2. Do not write any numbers, digits, number words, ordinals, or percentages.
3. Use only words present in the Facts below, the cited Documents below, and basic English function words.
4. Do not use negations unless copying a literal sentence from the Facts or a cited Document.
5. Cite any referenced document using [doc_id]. Never invent citations.
6. Never contradict the evaluation status or escalation, never give diagnoses or medication/dosage advice, and never claim an approved protocol unless stated in Facts.";

/// Estrategia intercambiable de formateo de prompt para [`crate::explain::SlmExplainer`].
///
/// Permite añadir en Fase 1 el formato `source`/`motor`/`documents` del SLM v5 sin
/// modificar el bucle de ejecución ni la validación cerrada.
pub trait PromptFormat: Send + Sync {
    /// Prompt de sistema enviado al motor de inferencia.
    fn system_prompt(&self) -> &str;

    /// Construye el prompt de usuario en inglés con los hechos deterministas y las fichas curadas.
    fn format_prompt(&self, eval: &Evaluation, facts: &[FactLine], docs: &[Document]) -> String;
}

/// Formato por defecto en inglés llano basado en [`FactLine`] y [`Document`].
#[derive(Clone, Copy, Debug, Default)]
pub struct PlainEnglishPromptFormat;

impl PromptFormat for PlainEnglishPromptFormat {
    fn system_prompt(&self) -> &str {
        PLAIN_ENGLISH_SYSTEM_PROMPT
    }

    fn format_prompt(&self, _eval: &Evaluation, facts: &[FactLine], docs: &[Document]) -> String {
        let mut out = String::from("Facts:\n");
        for fact in facts {
            out.push_str("- ");
            out.push_str(&fact.text);
            out.push('\n');
        }

        if docs.is_empty() {
            out.push_str("Documents: none\n");
        } else {
            out.push_str("Documents:\n");
            for doc in docs {
                out.push_str("- [");
                out.push_str(&doc.id);
                out.push_str("] ");
                out.push_str(&doc.text);
                out.push('\n');
            }
        }

        out.push_str(
            "Task: Write 1 to 3 short English sentences with no numbers, using only the words above and citing any used document as [id].\n",
        );
        out
    }
}
