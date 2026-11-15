//! Propuesta de registro a partir de texto libre (RF-14).
//!
//! Contrato común de las dos capas del intérprete: la capa 1 determinista
//! (`interpret`, en este crate) y la capa 2 con SLM (`merken-slm`). Ninguna
//! guarda nada: devuelven una [`Proposal`] que el núcleo comprueba con
//! [`Proposal::check`] y que el usuario confirma campo a campo.
//!
//! Reglas del contrato:
//! - cada valor lleva la **cita literal** (`quote`) del texto que lo respalda;
//!   un valor sin cita no se acepta;
//! - un campo que no se entiende va con `value: null` (sin interpretar), nunca
//!   con un valor inventado;
//! - el intérprete **no inventa horas**: una expresión temporal se devuelve
//!   como [`TimeHint::Ambiguous`] y la resuelve el usuario;
//! - lo negado ("no me duele la cabeza") va en `negated`, no como registro.
use crate::{ErrorCode, ValidationError, catalog::Indicator, measurement::Measurement};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Versión del contrato de propuesta.
pub const PROPOSAL_SCHEMA_VERSION: u32 = 1;

/// Quién interpretó el texto.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpreterKind {
    /// Capa 1: reglas deterministas de `merken-core`.
    Rules,
    /// Capa 2: modelo pequeño con salida restringida (`merken-slm`).
    Slm,
}

/// Indicación temporal tal como aparece en el texto. Nunca una hora calculada.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TimeHint {
    /// El texto no dice cuándo: la interfaz propone "ahora" y el usuario confirma.
    Unspecified,
    /// El texto da una referencia temporal que el usuario debe concretar.
    Ambiguous { quote: String },
}

/// Un campo del `value` de `logRecord` (p. ej. `bpm`, `score`, `body_zone`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProposedField {
    /// Clave del campo en el `value` del indicador.
    pub key: String,
    /// Valor propuesto, o `None` si no se pudo interpretar.
    pub value: Option<Value>,
    /// Fragmento literal del texto de entrada que respalda el valor.
    pub quote: Option<String>,
}

/// Un registro propuesto. Puede estar incompleto: entonces la interfaz abre el
/// formulario con lo entendido ya rellenado.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProposedRecord {
    pub indicator: Indicator,
    pub fields: Vec<ProposedField>,
    pub time: TimeHint,
    /// Fragmento literal que menciona este indicador.
    pub quote: String,
}

/// Resultado de interpretar un texto. Nada de esto se ha guardado.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    pub schema_version: u32,
    pub interpreter: InterpreterKind,
    /// Versión de las reglas o del modelo que interpretó.
    pub interpreter_version: String,
    pub records: Vec<ProposedRecord>,
    /// Menciones negadas (p. ej. "no me duele la cabeza"): no se registran.
    pub negated: Vec<String>,
    /// Partes del texto que no se entendieron.
    pub uninterpreted: Vec<String>,
}

/// Problema encontrado al comprobar una propuesta.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProposalIssue {
    /// Índice del registro afectado, si aplica.
    pub record: Option<usize>,
    pub error: ValidationError,
}

/// Campos que admite el `value` de cada indicador (mismo contrato que `Measurement`).
pub const fn allowed_fields(indicator: Indicator) -> &'static [&'static str] {
    match indicator {
        Indicator::HeartRate => &["bpm"],
        Indicator::BloodPressure => &["systolic", "diastolic"],
        Indicator::BodyTemperature => &["celsius"],
        Indicator::Glucose => &["mg_dl"],
        Indicator::Sleep => &["hours", "quality", "window_start", "window_end"],
        Indicator::Mood => &["score"],
        Indicator::OxygenInBlood => &["percentage"],
        Indicator::SpaceRadiation => &["usv_h"],
        Indicator::DistanceFromEarth => &["km"],
        Indicator::Nutrition => &["kcal"],
        Indicator::Symptom => &["body_zone", "score", "description"],
    }
}

fn issue(record: Option<usize>, field: &str, code: ErrorCode, message: &str) -> ProposalIssue {
    ProposalIssue {
        record,
        error: ValidationError {
            code,
            field: field.to_owned(),
            message: message.to_owned(),
        },
    }
}

fn quoted(input: &str, quote: &str) -> bool {
    !quote.trim().is_empty() && input.contains(quote)
}

impl ProposedRecord {
    /// `value` para `logRecord` con los campos interpretados (omite los `None`).
    pub fn value(&self) -> Value {
        let mut obj = Map::new();
        for f in &self.fields {
            if let Some(v) = &f.value {
                obj.insert(f.key.clone(), v.clone());
            }
        }
        Value::Object(obj)
    }

    /// `true` si todos los campos están interpretados y el valor pasa la
    /// validación del núcleo, de modo que se puede confirmar sin formulario.
    pub fn is_complete(&self) -> bool {
        self.fields.iter().all(|f| f.value.is_some())
            && Measurement::validate(self.indicator, &self.value(), None).is_ok()
    }
}

impl Proposal {
    /// Comprueba la propuesta contra el texto de entrada y el contrato del
    /// núcleo. Devuelve todos los problemas; una lista vacía significa que la
    /// estructura es válida (no que la interpretación sea correcta: eso lo
    /// confirma el usuario).
    pub fn check(&self, input: &str) -> Vec<ProposalIssue> {
        let mut out = Vec::new();
        if self.schema_version != PROPOSAL_SCHEMA_VERSION {
            out.push(issue(
                None,
                "schema_version",
                ErrorCode::InvalidSchema,
                "Unsupported proposal schema version",
            ));
        }
        for (field, list) in [
            ("negated", &self.negated),
            ("uninterpreted", &self.uninterpreted),
        ] {
            if list.iter().any(|q| !quoted(input, q)) {
                out.push(issue(
                    None,
                    field,
                    ErrorCode::InvalidValue,
                    "Quote is not part of the input text",
                ));
            }
        }
        for (i, rec) in self.records.iter().enumerate() {
            let at = Some(i);
            if !quoted(input, &rec.quote) {
                out.push(issue(
                    at,
                    "quote",
                    ErrorCode::InvalidValue,
                    "Quote is not part of the input text",
                ));
            }
            if let TimeHint::Ambiguous { quote } = &rec.time
                && !quoted(input, quote)
            {
                out.push(issue(
                    at,
                    "time",
                    ErrorCode::InvalidValue,
                    "Quote is not part of the input text",
                ));
            }
            let allowed = allowed_fields(rec.indicator);
            let mut seen: Vec<&str> = Vec::new();
            for f in &rec.fields {
                if !allowed.contains(&f.key.as_str()) {
                    out.push(issue(at, &f.key, ErrorCode::InvalidSchema, "Unknown field"));
                }
                if seen.contains(&f.key.as_str()) {
                    out.push(issue(
                        at,
                        &f.key,
                        ErrorCode::InvalidSchema,
                        "Duplicated field",
                    ));
                }
                seen.push(&f.key);
                if f.value.is_some() && !f.quote.as_deref().is_some_and(|q| quoted(input, q)) {
                    out.push(issue(
                        at,
                        &f.key,
                        ErrorCode::InvalidValue,
                        "Every value needs a quote taken from the input text",
                    ));
                }
            }
            if rec.fields.iter().all(|f| f.value.is_some())
                && let Err(e) = Measurement::validate(rec.indicator, &rec.value(), None)
            {
                out.push(ProposalIssue {
                    record: at,
                    error: e,
                });
            }
        }
        out
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pulse(quote: &str, bpm: Value, field_quote: Option<&str>) -> Proposal {
        Proposal {
            schema_version: PROPOSAL_SCHEMA_VERSION,
            interpreter: InterpreterKind::Rules,
            interpreter_version: "test".into(),
            records: vec![ProposedRecord {
                indicator: Indicator::HeartRate,
                fields: vec![ProposedField {
                    key: "bpm".into(),
                    value: Some(bpm),
                    quote: field_quote.map(Into::into),
                }],
                time: TimeHint::Unspecified,
                quote: quote.into(),
            }],
            negated: vec![],
            uninterpreted: vec![],
        }
    }

    #[test]
    fn valid_proposal_has_no_issues() {
        let p = pulse("pulse 72", json!(72), Some("72"));
        assert!(p.check("my pulse 72 now").is_empty());
        assert!(p.records[0].is_complete());
    }

    #[test]
    fn invented_quote_is_rejected() {
        let p = pulse("pulse 72", json!(72), Some("72"));
        assert!(!p.check("I feel fine").is_empty());
    }

    #[test]
    fn value_without_quote_is_rejected() {
        let p = pulse("pulse 72", json!(72), None);
        assert!(!p.check("pulse 72").is_empty());
    }

    #[test]
    fn invalid_value_is_rejected_by_core() {
        let p = pulse("pulse 0", json!(0), Some("0"));
        assert!(!p.check("pulse 0").is_empty());
    }

    #[test]
    fn json_round_trip() {
        let p = pulse("pulse 72", json!(72), Some("72"));
        let s = serde_json::to_string(&p).unwrap();
        let back: Proposal = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
