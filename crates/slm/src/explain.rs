//! Explicador de evaluaciones de `merken-core` con hechos deterministas y selección curada (RF-15).

pub mod facts;
pub mod prompt;
pub mod selection;

#[cfg(test)]
mod tests;

pub use facts::{FactId, FactLine, render_facts};
pub use prompt::{PlainEnglishPromptFormat, PromptFormat};
pub use selection::{
    AI_SUGGESTION_LABEL, AiNote, ERR_DUPLICATE_SELECTION, ERR_EXTRA_FIELDS, ERR_INVALID_JSON,
    ERR_INVALID_SCHEMA, ERR_OUTPUT_TOO_LARGE, ERR_TOO_MANY_SELECTIONS, ERR_UNKNOWN_SELECTION,
    Ficha, FichaId, MAX_ID_LEN, MAX_SELECTED_STATEMENTS, MAX_SELECTION_IDS,
    MAX_SELECTION_OUTPUT_BYTES, MAX_STATEMENT_CHARS, OfferedStatement, RenderedStatement,
    Statement, StatementId, build_selection_grammar, offered_statements, validate_selection,
};

use crate::{
    control::{Control, Progress},
    engine::{DEFAULT_MAX_TOKENS, DEFAULT_SEED, DEFAULT_TEMPERATURE, Engine, GenerateRequest},
    error::SlmError,
};
use merken_core::evaluate::Evaluation;
use serde::Serialize;

/// Resultado del explicador: hechos deterministas siempre visibles y nota opcional curada.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Explanation {
    pub facts: Vec<FactLine>,
    pub note: Option<AiNote>,
    pub note_discarded: Option<&'static str>,
    pub model_id: String,
}

/// Trait para explicadores de evaluaciones deterministas (RF-15).
pub trait Explainer {
    /// Explica una evaluación produciendo hechos deterministas y, opcionalmente, frases curadas.
    fn explain(
        &mut self,
        eval: &Evaluation,
        fichas: &[Ficha],
        ctl: &mut Control<'_>,
    ) -> Result<Explanation, SlmError>;
}

/// Explicador respaldado por un [`Engine`] que selecciona y ordena frases autorizadas mediante GBNF.
pub struct SlmExplainer<E: Engine> {
    engine: E,
    model_id: String,
    max_tokens: u32,
    prompt_format: Box<dyn PromptFormat>,
}

impl<E: Engine> SlmExplainer<E> {
    /// Crea un explicador indicando explícitamente el `model_id` verificado.
    pub fn new(engine: E, model_id: impl Into<String>) -> Self {
        Self {
            engine,
            model_id: model_id.into(),
            max_tokens: DEFAULT_MAX_TOKENS,
            prompt_format: Box::new(PlainEnglishPromptFormat),
        }
    }

    /// Crea un explicador tomando el `model_id` reportado por el propio motor.
    pub fn with_engine(engine: E) -> Self {
        let model_id = engine.model_id().to_owned();
        Self::new(engine, model_id)
    }

    /// Ajusta el límite máximo de tokens de salida.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Sustituye el formato de construcción del prompt (p. ej. para el contrato v5 de Fase 1).
    pub fn with_prompt_format<F: PromptFormat + 'static>(mut self, format: F) -> Self {
        self.prompt_format = Box::new(format);
        self
    }

    /// Devuelve una referencia al motor subyacente.
    pub fn engine(&self) -> &E {
        &self.engine
    }

    /// Devuelve una referencia mutable al motor subyacente.
    pub fn engine_mut(&mut self) -> &mut E {
        &mut self.engine
    }

    /// Libera el envoltorio devolviendo el motor subyacente.
    pub fn into_engine(self) -> E {
        self.engine
    }

    fn build_request(
        &self,
        eval: &Evaluation,
        facts: &[FactLine],
        offered: &[OfferedStatement],
    ) -> GenerateRequest {
        let ids: Vec<&str> = offered.iter().map(|s| s.selection_id.as_str()).collect();
        GenerateRequest {
            system: self.prompt_format.system_prompt().to_owned(),
            prompt: self.prompt_format.format_prompt(eval, facts, offered),
            grammar: Some(build_selection_grammar(&ids)),
            max_tokens: self.max_tokens,
            temperature: DEFAULT_TEMPERATURE,
            seed: DEFAULT_SEED,
        }
    }
}

impl<E: Engine> Explainer for SlmExplainer<E> {
    fn explain(
        &mut self,
        eval: &Evaluation,
        fichas: &[Ficha],
        ctl: &mut Control<'_>,
    ) -> Result<Explanation, SlmError> {
        self.explain_with_facts(eval, fichas, render_facts(eval), ctl)
    }
}

impl<E: Engine> SlmExplainer<E> {
    /// Service entry point: facts were already rendered on the command thread.
    pub(crate) fn explain_with_facts(
        &mut self,
        eval: &Evaluation,
        fichas: &[Ficha],
        facts: Vec<FactLine>,
        ctl: &mut Control<'_>,
    ) -> Result<Explanation, SlmError> {
        ctl.check()?;
        let offered = offered_statements(fichas, eval.status)?;
        let req = self.build_request(eval, &facts, &offered);
        let raw = self.engine.generate(&req, ctl)?;
        ctl.check()?;
        ctl.report_and_check(Progress::Validating)?;

        let (note, note_discarded) = match validate_selection(&raw, &offered) {
            Ok(valid) => (valid, None),
            Err(code) => (None, Some(code)),
        };

        ctl.check()?;
        ctl.report_and_check(Progress::Done)?;
        Ok(Explanation {
            facts,
            note,
            note_discarded,
            model_id: self.model_id.clone(),
        })
    }
}
