//! Intérprete de texto libre con salida restringida por gramática GBNF (capa 2 de RF-14).

use crate::{
    control::{Control, Progress},
    engine::{DEFAULT_MAX_TOKENS, DEFAULT_SEED, DEFAULT_TEMPERATURE, Engine, GenerateRequest},
    error::{OutputIssue, SlmError},
};
use pulsys_core::{
    ErrorCode,
    catalog::Indicator,
    interpret::MAX_INPUT_CHARS,
    proposal::{
        InterpreterKind, PROPOSAL_SCHEMA_VERSION, Proposal, ProposedField, ProposedRecord,
        TimeHint, allowed_fields,
    },
};
use serde::Deserialize;
use serde_json::Value;

/// Límite máximo en bytes de la salida cruda del modelo antes de deserializar JSON.
pub const MAX_INTERPRETER_OUTPUT_BYTES: usize = 16 * 1024;
/// Número máximo de registros permitidos en una salida cruda del intérprete SLM.
pub const MAX_PROPOSAL_RECORDS: usize = 32;
/// Número máximo de campos permitidos por registro en la salida cruda del intérprete SLM.
pub const MAX_FIELDS_PER_RECORD: usize = 8;
/// Número máximo de citas admitidas en `negated` o `uninterpreted`.
pub const MAX_QUOTE_ITEMS: usize = 64;
/// Tamaño máximo en bytes por cita individual devuelta por el intérprete SLM.
pub const MAX_QUOTE_BYTES: usize = MAX_INPUT_CHARS * 4;

/// Gramática GBNF que restringe la salida del SLM al contrato cerrado [`Proposal`].
///
/// Impone claves exactas, versión de esquema `1`, intérprete `"slm"`, indicadores
/// cerrados de [`Indicator`], zonas corporales de `BodyZone`, calidad de sueño de
/// `SleepQuality`, tiempos `unspecified`/`ambiguous` y `null` para campos dudosos.
pub const PROPOSAL_GBNF: &str = r#"
root ::= ws "{" ws "\"schema_version\"" ws ":" ws "1" ws "," ws "\"interpreter\"" ws ":" ws "\"slm\"" ws "," ws "\"interpreter_version\"" ws ":" ws string ws "," ws "\"records\"" ws ":" ws record-array ws "," ws "\"negated\"" ws ":" ws string-array ws "," ws "\"uninterpreted\"" ws ":" ws string-array ws "}" ws

record-array ::= "[]" | "[" ws record (ws "," ws record)* ws "]"
string-array ::= "[]" | "[" ws string (ws "," ws string)* ws "]"

record ::= heart-rate-rec | blood-pressure-rec | body-temperature-rec | glucose-rec | sleep-rec | mood-rec | oxygen-in-blood-rec | space-radiation-rec | distance-from-earth-rec | nutrition-rec | symptom-rec

heart-rate-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"heart_rate\"" ws "," ws "\"fields\"" ws ":" ws "[" ws field-bpm ws "]" ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"

blood-pressure-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"blood_pressure\"" ws "," ws "\"fields\"" ws ":" ws bp-fields ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"
bp-fields ::= "[]" | "[" ws bp-field (ws "," ws bp-field)* ws "]"
bp-field ::= field-systolic | field-diastolic

body-temperature-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"body_temperature\"" ws "," ws "\"fields\"" ws ":" ws "[" ws field-celsius ws "]" ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"

glucose-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"glucose\"" ws "," ws "\"fields\"" ws ":" ws "[" ws field-mg-dl ws "]" ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"

sleep-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"sleep\"" ws "," ws "\"fields\"" ws ":" ws sleep-fields ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"
sleep-fields ::= "[]" | "[" ws sleep-field (ws "," ws sleep-field)* ws "]"
sleep-field ::= field-hours | field-quality | field-window-start | field-window-end

mood-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"mood\"" ws "," ws "\"fields\"" ws ":" ws "[" ws field-score ws "]" ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"

oxygen-in-blood-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"oxygen_in_blood\"" ws "," ws "\"fields\"" ws ":" ws "[" ws field-percentage ws "]" ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"

space-radiation-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"space_radiation\"" ws "," ws "\"fields\"" ws ":" ws "[" ws field-usv-h ws "]" ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"

distance-from-earth-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"distance_from_earth\"" ws "," ws "\"fields\"" ws ":" ws "[" ws field-km ws "]" ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"

nutrition-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"nutrition\"" ws "," ws "\"fields\"" ws ":" ws "[" ws field-kcal ws "]" ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"

symptom-rec ::= "{" ws "\"indicator\"" ws ":" ws "\"symptom\"" ws "," ws "\"fields\"" ws ":" ws symptom-fields ws "," ws "\"time\"" ws ":" ws time-hint ws "," ws "\"quote\"" ws ":" ws string ws "}"
symptom-fields ::= "[]" | "[" ws symptom-field (ws "," ws symptom-field)* ws "]"
symptom-field ::= field-body-zone | field-score | field-description

field-bpm ::= "{" ws "\"key\"" ws ":" ws "\"bpm\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-systolic ::= "{" ws "\"key\"" ws ":" ws "\"systolic\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-diastolic ::= "{" ws "\"key\"" ws ":" ws "\"diastolic\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-celsius ::= "{" ws "\"key\"" ws ":" ws "\"celsius\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-mg-dl ::= "{" ws "\"key\"" ws ":" ws "\"mg_dl\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-hours ::= "{" ws "\"key\"" ws ":" ws "\"hours\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-quality ::= "{" ws "\"key\"" ws ":" ws "\"quality\"" ws "," ws "\"value\"" ws ":" ws nullable-sleep-quality ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-window-start ::= "{" ws "\"key\"" ws ":" ws "\"window_start\"" ws "," ws "\"value\"" ws ":" ws nullable-string ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-window-end ::= "{" ws "\"key\"" ws ":" ws "\"window_end\"" ws "," ws "\"value\"" ws ":" ws nullable-string ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-score ::= "{" ws "\"key\"" ws ":" ws "\"score\"" ws "," ws "\"value\"" ws ":" ws nullable-score-int ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-percentage ::= "{" ws "\"key\"" ws ":" ws "\"percentage\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-usv-h ::= "{" ws "\"key\"" ws ":" ws "\"usv_h\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-km ::= "{" ws "\"key\"" ws ":" ws "\"km\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-kcal ::= "{" ws "\"key\"" ws ":" ws "\"kcal\"" ws "," ws "\"value\"" ws ":" ws nullable-number ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-body-zone ::= "{" ws "\"key\"" ws ":" ws "\"body_zone\"" ws "," ws "\"value\"" ws ":" ws nullable-body-zone ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"
field-description ::= "{" ws "\"key\"" ws ":" ws "\"description\"" ws "," ws "\"value\"" ws ":" ws nullable-string ws "," ws "\"quote\"" ws ":" ws nullable-string ws "}"

time-hint ::= "{" ws "\"kind\"" ws ":" ws "\"unspecified\"" ws "}" | "{" ws "\"kind\"" ws ":" ws "\"ambiguous\"" ws "," ws "\"quote\"" ws ":" ws string ws "}"

body-zone ::= "\"head\"" | "\"neck\"" | "\"chest\"" | "\"abdomen\"" | "\"back\"" | "\"left_arm\"" | "\"right_arm\"" | "\"left_hand\"" | "\"right_hand\"" | "\"left_leg\"" | "\"right_leg\"" | "\"left_foot\"" | "\"right_foot\"" | "\"other\""
sleep-quality ::= "\"poor\"" | "\"fair\"" | "\"good\""

nullable-number ::= number | "null"
nullable-score-int ::= score-int | "null"
nullable-body-zone ::= body-zone | "null"
nullable-sleep-quality ::= sleep-quality | "null"
nullable-string ::= string | "null"

score-int ::= "10" | [0-9]
number ::= ("-"? ([0-9] | [1-9] [0-9]*)) ("." [0-9]+)? ([eE] [+-]? [0-9]+)?
string ::= "\"" ( [^"\\\x7F\x00-\x1F] | "\\" (["\\/bfnrt] | "u" [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F] [0-9a-fA-F]) )* "\""
ws ::= [ \t\n\r]*
"#;

const INTERPRETER_SYSTEM_PROMPT: &str = r#"Extract health records as Proposal JSON only. Do not diagnose or give advice.
Copy every quote exactly from the input; field quotes must be inside the record quote.
Never invent numbers or quotes. Use null for uncertain values. Put negations in negated and unknown text in uninterpreted. Use ambiguous time only with its literal quote.
Input: nutrition 1850 kcal
Output: {"schema_version":1,"interpreter":"slm","interpreter_version":"example","records":[{"indicator":"nutrition","fields":[{"key":"kcal","value":1850,"quote":"1850 kcal"}],"time":{"kind":"unspecified"},"quote":"nutrition 1850 kcal"}],"negated":[],"uninterpreted":[]}
Input: no back pain
Output: {"schema_version":1,"interpreter":"slm","interpreter_version":"example","records":[],"negated":["no back pain"],"uninterpreted":[]}"#;

/// Trait para intérpretes de texto libre hacia una [`Proposal`] validable por `pulsys-core`.
pub trait Interpreter {
    /// Interpreta `text` y devuelve una propuesta verificada con [`Proposal::check`].
    fn interpret(&mut self, text: &str, ctl: &mut Control<'_>) -> Result<Proposal, SlmError>;
}

/// Intérprete de respaldo (capa 2 de RF-14) respaldado por un [`Engine`] y gramática GBNF.
///
/// Comparte el límite [`MAX_INPUT_CHARS`] de la capa determinista: rechaza entradas que
/// superen ese límite antes de invocar al motor, acota la salida del modelo antes de
/// analizarla y realiza comprobaciones cooperativas de [`Control`] durante el saneado.
pub struct SlmInterpreter<E: Engine> {
    engine: E,
    model_version: String,
    max_tokens: u32,
}

impl<E: Engine> SlmInterpreter<E> {
    /// Crea un intérprete SLM con parámetros deterministas (temperatura 0 y semilla fija).
    pub fn new(engine: E, model_version: impl Into<String>) -> Self {
        Self {
            engine,
            model_version: model_version.into(),
            max_tokens: DEFAULT_MAX_TOKENS,
        }
    }

    /// Ajusta el límite máximo de tokens de salida.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = max_tokens;
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

    fn build_request(&self, text: &str) -> GenerateRequest {
        GenerateRequest {
            system: INTERPRETER_SYSTEM_PROMPT.to_owned(),
            prompt: format!(
                "Extract a Proposal JSON (schema_version: {PROPOSAL_SCHEMA_VERSION}, interpreter: \"slm\", interpreter_version: \"{}\") from the following user input:\n<<<INPUT>>>\n{text}\n<<<END_INPUT>>>",
                self.model_version
            ),
            grammar: Some(PROPOSAL_GBNF.to_owned()),
            max_tokens: self.max_tokens,
            temperature: DEFAULT_TEMPERATURE,
            seed: DEFAULT_SEED,
        }
    }
}

#[derive(Deserialize)]
struct RawField {
    key: String,
    value: Option<Value>,
    quote: Option<String>,
}

#[derive(Deserialize)]
struct RawRecord {
    indicator: String,
    fields: Vec<RawField>,
    time: TimeHint,
    quote: String,
}

#[derive(Deserialize)]
struct RawProposal {
    schema_version: u32,
    #[serde(default)]
    #[allow(dead_code)]
    interpreter: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    interpreter_version: Option<String>,
    records: Vec<RawRecord>,
    #[serde(default)]
    negated: Vec<String>,
    #[serde(default)]
    uninterpreted: Vec<String>,
}

fn is_quoted(input: &str, quote: &str) -> bool {
    !quote.trim().is_empty() && input.contains(quote)
}

fn push_unique_quoted(list: &mut Vec<String>, input: &str, quote: &str) {
    if is_quoted(input, quote) && !list.iter().any(|existing| existing == quote) {
        list.push(quote.to_owned());
    }
}

fn dedup_quoted(
    list: &mut Vec<String>,
    input: &str,
    ctl: &mut Control<'_>,
) -> Result<(), SlmError> {
    let mut cleaned = Vec::with_capacity(list.len());
    for item in list.drain(..) {
        ctl.check()?;
        push_unique_quoted(&mut cleaned, input, &item);
    }
    *list = cleaned;
    Ok(())
}

fn check_raw_proposal_limits(raw: &RawProposal, ctl: &mut Control<'_>) -> Result<(), SlmError> {
    ctl.check()?;
    if raw.records.len() > MAX_PROPOSAL_RECORDS {
        return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
            "too_many_records",
            "Interpreter output exceeds maximum allowed record count",
        )]));
    }
    if raw.negated.len() > MAX_QUOTE_ITEMS || raw.uninterpreted.len() > MAX_QUOTE_ITEMS {
        return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
            "too_many_quotes",
            "Interpreter output exceeds maximum allowed quote list size",
        )]));
    }

    for rec in &raw.records {
        ctl.check()?;
        if rec.fields.len() > MAX_FIELDS_PER_RECORD {
            return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                "too_many_fields",
                "Interpreter record exceeds maximum allowed field count",
            )]));
        }
        if rec.quote.len() > MAX_QUOTE_BYTES {
            return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                "quote_too_long",
                "Interpreter record quote exceeds maximum allowed byte length",
            )]));
        }
        if let TimeHint::Ambiguous { quote } = &rec.time
            && quote.len() > MAX_QUOTE_BYTES
        {
            return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                "quote_too_long",
                "Interpreter time hint quote exceeds maximum allowed byte length",
            )]));
        }
        for field in &rec.fields {
            if field
                .quote
                .as_ref()
                .is_some_and(|q| q.len() > MAX_QUOTE_BYTES)
            {
                return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                    "quote_too_long",
                    "Interpreter field quote exceeds maximum allowed byte length",
                )]));
            }
        }
    }

    for quote in raw.negated.iter().chain(raw.uninterpreted.iter()) {
        ctl.check()?;
        if quote.len() > MAX_QUOTE_BYTES {
            return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                "quote_too_long",
                "Interpreter quote entry exceeds maximum allowed byte length",
            )]));
        }
    }

    Ok(())
}

fn sanitize_proposal(
    proposal: &mut Proposal,
    input: &str,
    ctl: &mut Control<'_>,
) -> Result<(), SlmError> {
    ctl.check()?;
    dedup_quoted(&mut proposal.negated, input, ctl)?;
    dedup_quoted(&mut proposal.uninterpreted, input, ctl)?;

    let max_iters = proposal.records.len().saturating_mul(16).saturating_add(16);
    for _ in 0..max_iters {
        ctl.check()?;
        let issues = proposal.check(input);
        let Some(first) = issues.into_iter().next() else {
            dedup_quoted(&mut proposal.negated, input, ctl)?;
            dedup_quoted(&mut proposal.uninterpreted, input, ctl)?;
            return Ok(());
        };

        let Some(rec_idx) = first.record else {
            match first.error.field.as_str() {
                "schema_version" => {
                    return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                        "invalid_schema_version",
                        "Proposal schema_version does not match the core contract",
                    )]));
                }
                "negated" => dedup_quoted(&mut proposal.negated, input, ctl)?,
                "uninterpreted" => dedup_quoted(&mut proposal.uninterpreted, input, ctl)?,
                _ => {
                    return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                        "invalid_proposal",
                        "Unrecoverable proposal validation issue",
                    )]));
                }
            }
            continue;
        };

        let Some(rec) = proposal.records.get_mut(rec_idx) else {
            continue;
        };

        match first.error.field.as_str() {
            "quote" => {
                // Cita de registro inventada: se descarta el registro entero sin pasarlo a uninterpreted.
                proposal.records.remove(rec_idx);
            }
            "time" => {
                rec.time = TimeHint::Unspecified;
            }
            field_name if first.error.code == ErrorCode::InvalidSchema => {
                let allowed = allowed_fields(rec.indicator);
                let is_allowed = allowed.contains(&field_name);
                let mut kept_first = false;
                let mut i = 0;
                while i < rec.fields.len() {
                    let remove_this = match rec.fields.get(i) {
                        Some(f) if f.key == field_name => {
                            if !is_allowed || kept_first {
                                true
                            } else {
                                kept_first = true;
                                false
                            }
                        }
                        _ => false,
                    };
                    if remove_this {
                        let removed = rec.fields.remove(i);
                        if let Some(q) = removed.quote {
                            push_unique_quoted(&mut proposal.uninterpreted, input, &q);
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            field_name => {
                let mut touched_any = false;
                for f in &mut rec.fields {
                    if f.key == field_name {
                        let had_value = f.value.is_some();
                        let quote_valid = f.quote.as_deref().is_some_and(|q| is_quoted(input, q));
                        if had_value || !quote_valid {
                            f.value = None;
                            if let Some(q) = f.quote.take() {
                                push_unique_quoted(&mut proposal.uninterpreted, input, &q);
                            }
                            touched_any = true;
                        }
                    }
                }
                if !touched_any {
                    let allowed = allowed_fields(rec.indicator);
                    let already_present = rec.fields.iter().any(|f| f.key == field_name);
                    if allowed.contains(&field_name) && !already_present {
                        // Campo obligatorio ausente (p. ej. window_start en sueño): dejar en None
                        // para que el usuario complete el formulario sin perder el resto.
                        rec.fields.push(ProposedField {
                            key: field_name.to_owned(),
                            value: None,
                            quote: None,
                        });
                    } else {
                        let mut any_cleared = false;
                        for f in &mut rec.fields {
                            if f.value.take().is_some() {
                                any_cleared = true;
                            }
                            if let Some(q) = f.quote.take() {
                                push_unique_quoted(&mut proposal.uninterpreted, input, &q);
                            }
                        }
                        if !any_cleared {
                            let removed = proposal.records.remove(rec_idx);
                            push_unique_quoted(&mut proposal.uninterpreted, input, &removed.quote);
                        }
                    }
                }
            }
        }
    }

    if proposal.check(input).is_empty() {
        Ok(())
    } else {
        Err(SlmError::InvalidOutput(vec![OutputIssue::new(
            "sanitization_failed",
            "Proposal still contains invalid fields after sanitization",
        )]))
    }
}

impl<E: Engine> Interpreter for SlmInterpreter<E> {
    fn interpret(&mut self, text: &str, ctl: &mut Control<'_>) -> Result<Proposal, SlmError> {
        ctl.check()?;
        if text.chars().count() > MAX_INPUT_CHARS {
            return Err(SlmError::Unsupported(
                "Input exceeds maximum allowed character limit".to_owned(),
            ));
        }
        let req = self.build_request(text);
        let raw_output = self.engine.generate(&req, ctl)?;
        ctl.check()?;
        ctl.report_and_check(Progress::Validating)?;

        if raw_output.len() > MAX_INTERPRETER_OUTPUT_BYTES {
            return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                "output_too_large",
                "Interpreter output exceeds maximum allowed byte size",
            )]));
        }

        let raw: RawProposal = serde_json::from_str(raw_output.trim()).map_err(|_| {
            SlmError::InvalidOutput(vec![OutputIssue::new(
                "invalid_json",
                "Interpreter output is not valid Proposal JSON",
            )])
        })?;

        if raw.schema_version != PROPOSAL_SCHEMA_VERSION {
            return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                "invalid_schema_version",
                "Proposal schema_version does not match the core contract",
            )]));
        }

        check_raw_proposal_limits(&raw, ctl)?;

        let mut records = Vec::with_capacity(raw.records.len());
        let mut uninterpreted = raw.uninterpreted;

        for rec in raw.records {
            ctl.check()?;
            match Indicator::parse(&rec.indicator) {
                Ok(indicator) => records.push(ProposedRecord {
                    indicator,
                    fields: rec
                        .fields
                        .into_iter()
                        .map(|f| ProposedField {
                            key: f.key,
                            value: f.value,
                            quote: f.quote,
                        })
                        .collect(),
                    time: rec.time,
                    quote: rec.quote,
                }),
                Err(_) => {
                    // Indicador desconocido: se descarta el registro y, si la cita es literal,
                    // pasa a uninterpreted.
                    push_unique_quoted(&mut uninterpreted, text, &rec.quote);
                }
            }
        }

        let mut proposal = Proposal {
            schema_version: PROPOSAL_SCHEMA_VERSION,
            interpreter: InterpreterKind::Slm,
            interpreter_version: self.model_version.clone(),
            records,
            negated: raw.negated,
            uninterpreted,
        };

        sanitize_proposal(&mut proposal, text, ctl)?;
        ctl.check()?;
        ctl.report_and_check(Progress::Done)?;
        Ok(proposal)
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::{
        control::{CancelToken, Deadline},
        engine::MockEngine,
    };
    use pulsys_core::catalog::{BodyZone, SleepQuality};
    use serde_json::json;
    use std::time::Duration;

    fn run_interpret(input: &str, mock_json: &str) -> (Result<Proposal, SlmError>, MockEngine) {
        let engine = MockEngine::with_responses([mock_json]);
        let mut interp = SlmInterpreter::new(engine, "granite-4.0-350m");
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));
        let res = interp.interpret(input, &mut ctl);
        (res, interp.into_engine())
    }

    #[test]
    fn proposal_gbnf_contains_closed_catalog_values() {
        assert!(PROPOSAL_GBNF.contains("root ::="));
        for ind in Indicator::ALL {
            assert!(
                PROPOSAL_GBNF.contains(&format!("\\\"{}\\\"", ind.as_str())),
                "missing indicator {}",
                ind.as_str()
            );
            for key in allowed_fields(ind) {
                assert!(
                    PROPOSAL_GBNF.contains(&format!("\\\"{key}\\\"")),
                    "missing field key {key}"
                );
            }
        }
        for zone in BodyZone::ALL {
            assert!(
                PROPOSAL_GBNF.contains(&format!("\\\"{}\\\"", zone.as_str())),
                "missing body_zone {}",
                zone.as_str()
            );
        }
        for q in SleepQuality::ALL {
            assert!(
                PROPOSAL_GBNF.contains(&format!("\\\"{}\\\"", q.as_str())),
                "missing sleep quality {}",
                q.as_str()
            );
        }
        assert!(PROPOSAL_GBNF.contains("\\\"unspecified\\\""));
        assert!(PROPOSAL_GBNF.contains("\\\"ambiguous\\\""));
        assert!(PROPOSAL_GBNF.contains("\\\"slm\\\""));
        assert!(PROPOSAL_GBNF.contains("\"null\""));
    }

    #[test]
    fn valid_output_passes_check_and_forces_slm_interpreter() {
        let input = "my pulse is 72 this morning, no headache";
        let json_out = json!({
            "schema_version": 1,
            "interpreter": "rules",
            "interpreter_version": "ignored",
            "records": [{
                "indicator": "heart_rate",
                "fields": [{"key": "bpm", "value": 72, "quote": "72"}],
                "time": {"kind": "ambiguous", "quote": "this morning"},
                "quote": "pulse is 72"
            }],
            "negated": ["no headache"],
            "uninterpreted": []
        })
        .to_string();

        let (res, engine) = run_interpret(input, &json_out);
        let proposal = res.unwrap();
        assert_eq!(proposal.interpreter, InterpreterKind::Slm);
        assert_eq!(proposal.interpreter_version, "granite-4.0-350m");
        assert!(proposal.check(input).is_empty());
        assert!(proposal.records[0].is_complete());
        assert_eq!(engine.requests()[0].grammar.as_deref(), Some(PROPOSAL_GBNF));
        assert_eq!(engine.requests()[0].temperature, 0.0);
        assert_eq!(engine.requests()[0].seed, DEFAULT_SEED);
    }

    #[test]
    fn value_without_quote_is_discarded_to_none() {
        let input = "heart rate feels fast";
        let json_out = json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "heart_rate",
                "fields": [{"key": "bpm", "value": 110, "quote": null}],
                "time": {"kind": "unspecified"},
                "quote": "heart rate feels fast"
            }],
            "negated": [],
            "uninterpreted": []
        })
        .to_string();

        let (res, _) = run_interpret(input, &json_out);
        let proposal = res.unwrap();
        assert!(proposal.check(input).is_empty());
        assert_eq!(proposal.records.len(), 1);
        assert_eq!(proposal.records[0].fields[0].value, None);
        assert!(!proposal.records[0].is_complete());
    }

    #[test]
    fn invented_quote_is_discarded_without_entering_uninterpreted() {
        let input = "I feel dizzy today";
        let json_out = json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [
                {
                    "indicator": "heart_rate",
                    "fields": [{"key": "bpm", "value": 95, "quote": "95"}],
                    "time": {"kind": "unspecified"},
                    "quote": "pulse 95"
                },
                {
                    "indicator": "mood",
                    "fields": [{"key": "score", "value": 4, "quote": "4"}],
                    "time": {"kind": "ambiguous", "quote": "yesterday"},
                    "quote": "feel dizzy"
                }
            ],
            "negated": ["invented negation"],
            "uninterpreted": ["invented uninterpreted"]
        })
        .to_string();

        let (res, _) = run_interpret(input, &json_out);
        let proposal = res.unwrap();
        assert!(proposal.check(input).is_empty());
        // Registro con cita inventada descartado; el de ánimo también, porque
        // "feel dizzy" no nombra el ánimo (`mentions_indicator` en `check`).
        assert!(proposal.records.is_empty());
        assert!(proposal.negated.is_empty());
        assert!(proposal.uninterpreted.is_empty());
    }

    #[test]
    fn out_of_range_number_is_discarded() {
        let input = "my pulse 0 and oxygen 150";
        let json_out = json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [
                {
                    "indicator": "heart_rate",
                    "fields": [{"key": "bpm", "value": 0, "quote": "0"}],
                    "time": {"kind": "unspecified"},
                    "quote": "pulse 0"
                },
                {
                    "indicator": "oxygen_in_blood",
                    "fields": [{"key": "percentage", "value": 150, "quote": "150"}],
                    "time": {"kind": "unspecified"},
                    "quote": "oxygen 150"
                }
            ],
            "negated": [],
            "uninterpreted": []
        })
        .to_string();

        let (res, _) = run_interpret(input, &json_out);
        let proposal = res.unwrap();
        assert!(proposal.check(input).is_empty());
        assert_eq!(proposal.records[0].fields[0].value, None);
        assert_eq!(proposal.records[1].fields[0].value, None);
        assert!(proposal.uninterpreted.contains(&"0".to_owned()));
        assert!(proposal.uninterpreted.contains(&"150".to_owned()));
    }

    #[test]
    fn unknown_indicator_is_discarded_into_uninterpreted() {
        let input = "my pulse is 72 and cholesterol 200";
        let json_out = json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [
                {
                    "indicator": "heart_rate",
                    "fields": [{"key": "bpm", "value": 72, "quote": "72"}],
                    "time": {"kind": "unspecified"},
                    "quote": "pulse is 72"
                },
                {
                    "indicator": "cholesterol",
                    "fields": [{"key": "total", "value": 200, "quote": "200"}],
                    "time": {"kind": "unspecified"},
                    "quote": "cholesterol 200"
                }
            ],
            "negated": [],
            "uninterpreted": []
        })
        .to_string();

        let (res, _) = run_interpret(input, &json_out);
        let proposal = res.unwrap();
        assert!(proposal.check(input).is_empty());
        assert_eq!(proposal.records.len(), 1);
        assert_eq!(proposal.records[0].indicator, Indicator::HeartRate);
        assert_eq!(proposal.uninterpreted, vec!["cholesterol 200".to_owned()]);
    }

    #[test]
    fn broken_json_returns_invalid_output() {
        let (res, _) = run_interpret("pulse 72", "{\"records\": [broken");
        assert!(matches!(res, Err(SlmError::InvalidOutput(issues)) if !issues.is_empty()));
    }

    #[test]
    fn cancel_on_validating_and_done_callbacks_returns_cancelled() {
        let input = "my pulse is 72";
        let valid_json = json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "heart_rate",
                "fields": [{"key": "bpm", "value": 72, "quote": "72"}],
                "time": {"kind": "unspecified"},
                "quote": "pulse is 72"
            }],
            "negated": [],
            "uninterpreted": []
        })
        .to_string();

        for target_stage in [Progress::Validating, Progress::Done] {
            let engine = MockEngine::with_responses([valid_json.clone()]);
            let mut interp = SlmInterpreter::new(engine, "granite-4.0-350m");
            let cancel = CancelToken::new();
            let cancel_cb = cancel.clone();
            let mut ctl = Control::new(
                cancel,
                Deadline::after(Duration::from_secs(5)),
                move |progress| {
                    if progress == target_stage {
                        cancel_cb.cancel();
                    }
                },
            );
            assert_eq!(interp.interpret(input, &mut ctl), Err(SlmError::Cancelled));
        }
    }

    #[test]
    fn deadline_expiring_during_validating_returns_timeout() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        use std::time::Instant;

        let input = "my pulse is 72";
        let valid_json = json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "heart_rate",
                "fields": [{"key": "bpm", "value": 72, "quote": "72"}],
                "time": {"kind": "unspecified"},
                "quote": "pulse is 72"
            }],
            "negated": [],
            "uninterpreted": []
        })
        .to_string();

        let start = Instant::now();
        let expired = Arc::new(AtomicBool::new(false));
        let expired_clock = Arc::clone(&expired);
        let deadline = Deadline::with_clock(
            start,
            Duration::from_secs(5),
            Arc::new(move || {
                if expired_clock.load(Ordering::SeqCst) {
                    start + Duration::from_secs(10)
                } else {
                    start
                }
            }),
        );
        let expired_cb = Arc::clone(&expired);
        let mut ctl = Control::new(CancelToken::new(), deadline, move |progress| {
            if progress == Progress::Validating {
                expired_cb.store(true, Ordering::SeqCst);
            }
        });

        let engine = MockEngine::with_responses([valid_json]);
        let mut interp = SlmInterpreter::new(engine, "granite-4.0-350m");
        assert_eq!(interp.interpret(input, &mut ctl), Err(SlmError::Timeout));
    }

    #[test]
    fn input_exceeding_max_chars_is_rejected_without_calling_engine() {
        let over_limit = "a".repeat(MAX_INPUT_CHARS + 1);
        let engine = MockEngine::new();
        let mut interp = SlmInterpreter::new(engine, "granite-4.0-350m");
        let mut ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));
        let res = interp.interpret(&over_limit, &mut ctl);
        assert!(matches!(res, Err(SlmError::Unsupported(_))));
        assert!(interp.engine().requests().is_empty());

        let at_limit = format!("{}pulse 72", " ".repeat(MAX_INPUT_CHARS - 8));
        assert_eq!(at_limit.chars().count(), MAX_INPUT_CHARS);
        let valid_json = json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "heart_rate",
                "fields": [{"key": "bpm", "value": 72, "quote": "72"}],
                "time": {"kind": "unspecified"},
                "quote": "pulse 72"
            }],
            "negated": [],
            "uninterpreted": []
        })
        .to_string();
        let (ok_res, _) = run_interpret(&at_limit, &valid_json);
        assert!(ok_res.is_ok());
    }

    #[test]
    fn output_and_structural_limits_reject_excessive_payloads() {
        let oversized = "x".repeat(MAX_INTERPRETER_OUTPUT_BYTES + 1);
        let (res, _) = run_interpret("pulse 72", &oversized);
        assert!(
            matches!(res, Err(SlmError::InvalidOutput(ref issues)) if issues.iter().any(|i| i.code == "output_too_large"))
        );

        let too_many_records = json!({
            "schema_version": 1,
            "records": vec![json!({
                "indicator": "heart_rate",
                "fields": [],
                "time": {"kind": "unspecified"},
                "quote": "pulse"
            }); MAX_PROPOSAL_RECORDS + 1],
            "negated": [],
            "uninterpreted": []
        })
        .to_string();
        let (res_recs, _) = run_interpret("pulse 72", &too_many_records);
        assert!(
            matches!(res_recs, Err(SlmError::InvalidOutput(ref issues)) if issues.iter().any(|i| i.code == "too_many_records"))
        );

        let too_many_fields = json!({
            "schema_version": 1,
            "records": [{
                "indicator": "heart_rate",
                "fields": vec![json!({"key": "bpm", "value": 72, "quote": "72"}); MAX_FIELDS_PER_RECORD + 1],
                "time": {"kind": "unspecified"},
                "quote": "pulse 72"
            }],
            "negated": [],
            "uninterpreted": []
        })
        .to_string();
        let (res_fields, _) = run_interpret("pulse 72", &too_many_fields);
        assert!(
            matches!(res_fields, Err(SlmError::InvalidOutput(ref issues)) if issues.iter().any(|i| i.code == "too_many_fields"))
        );

        let long_quote = "a".repeat(MAX_QUOTE_BYTES + 1);
        let long_quote_json = json!({
            "schema_version": 1,
            "records": [],
            "negated": [long_quote],
            "uninterpreted": []
        })
        .to_string();
        let (res_quote, _) = run_interpret("pulse 72", &long_quote_json);
        assert!(
            matches!(res_quote, Err(SlmError::InvalidOutput(ref issues)) if issues.iter().any(|i| i.code == "quote_too_long"))
        );
    }

    #[test]
    fn deadline_expiring_inside_sanitization_loop_returns_timeout() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
        };
        use std::time::Instant;

        let input = "pulse 72";

        // 1. Prueba directa sobre `sanitize_proposal`: verifica que ocurre al menos una
        // reparación antes de que venza el plazo en la siguiente iteración del bucle.
        let mut proposal = Proposal {
            schema_version: PROPOSAL_SCHEMA_VERSION,
            interpreter: InterpreterKind::Slm,
            interpreter_version: "granite-4.0-350m".to_owned(),
            records: vec![ProposedRecord {
                indicator: Indicator::HeartRate,
                fields: vec![
                    ProposedField {
                        key: "bpm".to_owned(),
                        value: Some(json!(0)),
                        quote: Some("72".to_owned()),
                    },
                    ProposedField {
                        key: "extra_a".to_owned(),
                        value: Some(json!(1)),
                        quote: Some("72".to_owned()),
                    },
                    ProposedField {
                        key: "extra_b".to_owned(),
                        value: Some(json!(2)),
                        quote: Some("72".to_owned()),
                    },
                ],
                time: TimeHint::Ambiguous {
                    quote: "invented".to_owned(),
                },
                quote: "pulse 72".to_owned(),
            }],
            negated: vec!["invented 1".to_owned(), "invented 2".to_owned()],
            uninterpreted: Vec::new(),
        };
        let initial_snapshot = proposal.clone();
        assert!(!initial_snapshot.check(input).is_empty());

        let direct_start = Instant::now();
        let direct_checks = Arc::new(AtomicU64::new(0));
        let direct_checks_clock = Arc::clone(&direct_checks);
        let direct_deadline = Deadline::with_clock(
            direct_start,
            Duration::from_secs(5),
            Arc::new(move || {
                // Check 0: entrada de sanitize_proposal; Checks 1..=2: dedup_quoted(negated);
                // Check 3: iteración 0 del bucle (primera reparación); Check 4: iteración 1 -> vence.
                let n = direct_checks_clock.fetch_add(1, Ordering::SeqCst);
                if n >= 4 {
                    direct_start + Duration::from_secs(10)
                } else {
                    direct_start
                }
            }),
        );
        let mut direct_ctl = Control::without_progress(CancelToken::new(), direct_deadline);
        assert_eq!(
            sanitize_proposal(&mut proposal, input, &mut direct_ctl),
            Err(SlmError::Timeout)
        );
        assert_eq!(direct_checks.load(Ordering::SeqCst), 5);
        assert!(proposal.negated.is_empty());
        let repaired_record = proposal.records != initial_snapshot.records;
        assert!(
            repaired_record,
            "sanitize_proposal must perform at least one record repair before timing out"
        );
        assert!(
            !proposal.check(input).is_empty(),
            "timeout must interrupt sanitization before all issues are repaired"
        );

        // 2. Ruta completa `SlmInterpreter::interpret`: el reloj se arma al entrar en
        // `Progress::Validating` y vence dentro de la iteración 1 de `sanitize_proposal`.
        let json_out = json!({
            "schema_version": 1,
            "records": [{
                "indicator": "heart_rate",
                "fields": [
                    {"key": "bpm", "value": 0, "quote": "72"},
                    {"key": "extra_a", "value": 1, "quote": "72"},
                    {"key": "extra_b", "value": 2, "quote": "72"}
                ],
                "time": {"kind": "ambiguous", "quote": "invented"},
                "quote": "pulse 72"
            }],
            "negated": [],
            "uninterpreted": []
        })
        .to_string();

        let start = Instant::now();
        let in_validating = Arc::new(AtomicBool::new(false));
        let in_validating_clock = Arc::clone(&in_validating);
        let validating_checks = Arc::new(AtomicU64::new(0));
        let validating_checks_clock = Arc::clone(&validating_checks);
        let deadline = Deadline::with_clock(
            start,
            Duration::from_secs(5),
            Arc::new(move || {
                if !in_validating_clock.load(Ordering::SeqCst) {
                    return start;
                }
                // Tras Validating: 0=Validating, 1..=2=limits, 3=conversión, 4=entrada saneado,
                // 5=iteración 0 de saneado (repara 1 fallo), 6=iteración 1 -> vence.
                let n = validating_checks_clock.fetch_add(1, Ordering::SeqCst);
                if n >= 6 {
                    start + Duration::from_secs(10)
                } else {
                    start
                }
            }),
        );
        let in_validating_cb = Arc::clone(&in_validating);
        let mut ctl = Control::new(CancelToken::new(), deadline, move |progress| {
            if progress == Progress::Validating {
                in_validating_cb.store(true, Ordering::SeqCst);
            }
        });
        let engine = MockEngine::with_responses([json_out]);
        let mut interp = SlmInterpreter::new(engine, "granite-4.0-350m");
        assert_eq!(interp.interpret(input, &mut ctl), Err(SlmError::Timeout));
        assert_eq!(validating_checks.load(Ordering::SeqCst), 7);
    }

    #[test]
    fn interpreter_errors_never_leak_sensitive_text_or_serde_fragments() {
        let secret = "[I have HIV] 999";
        let (res, _) = run_interpret(secret, secret);
        let err = res.unwrap_err();
        let display = err.to_string();
        assert!(!display.contains("HIV"));
        assert!(!display.contains("999"));
        let SlmError::InvalidOutput(issues) = err else {
            panic!("expected InvalidOutput");
        };
        for issue in &issues {
            assert!(!issue.code.contains("HIV"));
            assert!(!issue.detail.contains("HIV"));
            assert!(!issue.detail.contains("999"));
        }

        let wrong_version = json!({
            "schema_version": 999,
            "records": [],
            "negated": ["[I have HIV]"],
            "uninterpreted": []
        })
        .to_string();
        let (res_ver, _) = run_interpret(secret, &wrong_version);
        let SlmError::InvalidOutput(ver_issues) = res_ver.unwrap_err() else {
            panic!("expected InvalidOutput");
        };
        for issue in &ver_issues {
            assert!(!issue.detail.contains("999"));
            assert!(!issue.detail.contains("HIV"));
        }
    }
}
