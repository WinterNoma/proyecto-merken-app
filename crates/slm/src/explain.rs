//! Explicador de evaluaciones de `merken-core` con bloque determinista y nota SLM cerrada (RF-15).

pub mod facts;
pub mod note;
pub mod prompt;

#[cfg(test)]
mod tests;

pub use facts::{FactId, FactLine, render_facts};
pub use note::{
    AI_SUGGESTION_LABEL, AiNote, Document, ERR_EMPTY_NOTE, ERR_FORBIDDEN_NEGATION,
    ERR_FORBIDDEN_NUMBER, ERR_INCOMPATIBLE_STATUS, ERR_MALFORMED_CITATION, ERR_NON_ENGLISH,
    ERR_OUT_OF_VOCABULARY, ERR_SENTENCE_COUNT, ERR_SENTENCE_TOO_LONG, ERR_TOO_LONG,
    ERR_UNKNOWN_CITATION, ERR_UNVERIFIED_APPROVAL, MAX_NOTE_CHARS, MAX_NOTE_SENTENCES,
    MAX_WORDS_PER_SENTENCE, validate_note,
};
pub use prompt::{PlainEnglishPromptFormat, PromptFormat};

use crate::{
    control::{Control, Progress},
    engine::{DEFAULT_MAX_TOKENS, DEFAULT_SEED, DEFAULT_TEMPERATURE, Engine, GenerateRequest},
    error::SlmError,
};
use merken_core::evaluate::Evaluation;
use serde::Serialize;

/// Resultado del explicador: hechos deterministas siempre visibles y nota opcional validada del SLM.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Explanation {
    pub facts: Vec<FactLine>,
    pub note: Option<AiNote>,
    pub note_discarded: Option<&'static str>,
    pub model_id: String,
}

/// Trait para explicadores de evaluaciones deterministas (RF-15).
pub trait Explainer {
    /// Explica una evaluación produciendo hechos deterministas y, opcionalmente, una nota validada.
    fn explain(
        &mut self,
        eval: &Evaluation,
        docs: &[Document],
        ctl: &mut Control<'_>,
    ) -> Result<Explanation, SlmError>;
}

/// Explicador respaldado por un [`Engine`] con validación de vocabulario cerrado para la nota.
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
        docs: &[Document],
    ) -> GenerateRequest {
        GenerateRequest {
            system: self.prompt_format.system_prompt().to_owned(),
            prompt: self.prompt_format.format_prompt(eval, facts, docs),
            grammar: None,
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
        docs: &[Document],
        ctl: &mut Control<'_>,
    ) -> Result<Explanation, SlmError> {
        ctl.check()?;
        let facts = render_facts(eval);
        let req = self.build_request(eval, &facts, docs);
        let raw = self.engine.generate(&req, ctl)?;
        ctl.check()?;
        ctl.report_and_check(Progress::Validating)?;

        let (note, note_discarded) = match validate_note(&raw, &facts, docs) {
            Ok(valid) => (Some(valid), None),
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
