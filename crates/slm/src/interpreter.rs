//! Intérprete de texto libre con salida restringida por gramática GBNF (capa 2 de RF-14).

use crate::{
    control::{Control, Progress},
    engine::{DEFAULT_MAX_TOKENS, DEFAULT_SEED, DEFAULT_TEMPERATURE, Engine, GenerateRequest},
    error::{OutputIssue, SlmError},
};
use merken_core::{
    ErrorCode,
    catalog::Indicator,
    proposal::{
        InterpreterKind, PROPOSAL_SCHEMA_VERSION, Proposal, ProposedField, ProposedRecord,
        TimeHint, allowed_fields,
    },
};
use serde::Deserialize;
use serde_json::Value;

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

const INTERPRETER_SYSTEM_PROMPT: &str = "\
You extract structured astronaut health log proposals from free text (English or Spanish).
Return ONLY valid JSON matching the Proposal grammar.
Rules:
1. Every extracted record, field value, ambiguous time, negated phrase, and uninterpreted segment MUST carry a verbatim substring quote copied from the user input text.
2. Never invent numbers, timestamps, or quotes. If a value is uncertain or missing, set \"value\": null and \"quote\": null.
3. For relative or vague time expressions (such as \"this morning\" or \"last night\"), use {\"kind\":\"ambiguous\",\"quote\":\"...\"}. Otherwise use {\"kind\":\"unspecified\"}.
4. Put negated symptoms or readings in \"negated\" and unparsed fragments in \"uninterpreted\".
5. Never assign health statuses, diagnoses, treatments, or advice.";

/// Trait para intérpretes de texto libre hacia una [`Proposal`] validable por `merken-core`.
pub trait Interpreter {
    /// Interpreta `text` y devuelve una propuesta verificada con [`Proposal::check`].
    fn interpret(&mut self, text: &str, ctl: &Control<'_>) -> Result<Proposal, SlmError>;
}

/// Intérprete de respaldo (capa 2 de RF-14) respaldado por un [`Engine`] y gramática GBNF.
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

fn dedup_quoted(list: &mut Vec<String>, input: &str) {
    let mut cleaned = Vec::with_capacity(list.len());
    for item in list.drain(..) {
        push_unique_quoted(&mut cleaned, input, &item);
    }
    *list = cleaned;
}

fn sanitize_proposal(proposal: &mut Proposal, input: &str) -> Result<(), SlmError> {
    dedup_quoted(&mut proposal.negated, input);
    dedup_quoted(&mut proposal.uninterpreted, input);

    let max_iters = proposal.records.len().saturating_mul(16).saturating_add(16);
    for _ in 0..max_iters {
        let issues = proposal.check(input);
        let Some(first) = issues.into_iter().next() else {
            dedup_quoted(&mut proposal.negated, input);
            dedup_quoted(&mut proposal.uninterpreted, input);
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
                "negated" => dedup_quoted(&mut proposal.negated, input),
                "uninterpreted" => dedup_quoted(&mut proposal.uninterpreted, input),
                other => {
                    return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                        "invalid_proposal",
                        format!("Unrecoverable proposal issue on field '{other}'"),
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
    fn interpret(&mut self, text: &str, ctl: &Control<'_>) -> Result<Proposal, SlmError> {
        ctl.check()?;
        let req = self.build_request(text);
        let raw_output = self.engine.generate(&req, ctl)?;
        ctl.check()?;
        ctl.report(Progress::Validating);

        let raw: RawProposal = serde_json::from_str(raw_output.trim()).map_err(|e| {
            SlmError::InvalidOutput(vec![OutputIssue::new(
                "invalid_json",
                format!("Interpreter output is not valid Proposal JSON: {e}"),
            )])
        })?;

        if raw.schema_version != PROPOSAL_SCHEMA_VERSION {
            return Err(SlmError::InvalidOutput(vec![OutputIssue::new(
                "invalid_schema_version",
                "Proposal schema_version does not match the core contract",
            )]));
        }

        let mut records = Vec::with_capacity(raw.records.len());
        let mut uninterpreted = raw.uninterpreted;

        for rec in raw.records {
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

        sanitize_proposal(&mut proposal, text)?;
        ctl.report(Progress::Done);
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
    use merken_core::catalog::{BodyZone, SleepQuality};
    use serde_json::json;
    use std::time::Duration;

    fn run_interpret(input: &str, mock_json: &str) -> (Result<Proposal, SlmError>, MockEngine) {
        let engine = MockEngine::with_responses([mock_json]);
        let mut interp = SlmInterpreter::new(engine, "granite-4.0-350m");
        let ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));
        let res = interp.interpret(input, &ctl);
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
        // Registro con cita inventada descartado; registro con cita válida mantiene solo lo veraz.
        assert_eq!(proposal.records.len(), 1);
        assert_eq!(proposal.records[0].indicator, Indicator::Mood);
        assert_eq!(proposal.records[0].fields[0].value, None);
        assert_eq!(proposal.records[0].time, TimeHint::Unspecified);
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
}
