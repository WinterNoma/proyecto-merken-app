//! Capa 1 del intérprete de texto de Merken (RF-14): reglas deterministas.
//!
//! Convierte texto libre en español, inglés o mezclado en una [`Proposal`]
//! determinista, pura (sin E/S ni reloj) e instantánea. Nunca guarda nada y
//! nunca inventa valores ni horas: lo ambiguo queda con `value: None` o
//! [`TimeHint::Ambiguous`], las negaciones van a `negated` y lo que no se
//! entiende va a `uninterpreted`.

use crate::proposal::{InterpreterKind, PROPOSAL_SCHEMA_VERSION, Proposal};

mod extract;
mod lexicon;
mod numbers;
#[cfg(test)]
mod tests;
mod time;

/// Versión del intérprete determinista de reglas.
pub const RULES_INTERPRETER_VERSION: &str = "rules-interp-2026.11.14";

/// Longitud máxima en caracteres Unicode aceptada por [`interpret`].
/// Si la entrada supera este límite se devuelve una propuesta vacía para que
/// la interfaz abra el formulario directamente.
pub const MAX_INPUT_CHARS: usize = 1000;

pub(crate) use extract::{
    AnalyzedSegment, InputAnalysis, analyze_input_for_check, check_fallback_field_attribution,
    check_field_attribution_in_segment, is_single_clause_quote, occurrence_is_negated_in_input,
};
pub(crate) use lexicon::{
    BoundedOccurrence, check_numeric_unit_around_occurrence,
    find_token_bounded_occurrences_with_tokens, has_token_bounded_occurrence,
    quote_unit_compatible,
};
pub use lexicon::{mentions_indicator, quote_number, quote_sleep_quality, quote_zone};

/// Interpreta texto libre (español, inglés o mezclado) mediante reglas deterministas.
pub fn interpret(text: &str) -> Proposal {
    let trimmed = text.trim();
    if trimmed.is_empty() || text.chars().count() > MAX_INPUT_CHARS {
        return Proposal {
            schema_version: PROPOSAL_SCHEMA_VERSION,
            interpreter: InterpreterKind::Rules,
            interpreter_version: RULES_INTERPRETER_VERSION.to_owned(),
            records: Vec::new(),
            negated: Vec::new(),
            uninterpreted: Vec::new(),
        };
    }

    extract::interpret_within_limit(text)
}
