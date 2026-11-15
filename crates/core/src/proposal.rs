//! Propuesta de registro a partir de texto libre (RF-14).
//!
//! Contrato común de las dos capas del intérprete: la capa 1 determinista
//! (`interpret`, en este crate) y la capa 2 con SLM (`merken-slm`). Ninguna
//! guarda nada: devuelven una [`Proposal`] que el núcleo comprueba con
//! [`Proposal::check`] y que el usuario confirma campo a campo.
//!
//! Reglas del contrato:
//! - cada valor lleva la **cita literal** (`quote`) del texto que lo respalda
//!   (subcadena de la cita del registro, y esta de la entrada); un valor sin
//!   respaldo comprobable no se acepta;
//! - un campo que no se entiende va con `value: null` (sin interpretar), nunca
//!   con un valor inventado;
//! - el intérprete **no inventa horas ni ventanas**: una expresión temporal se
//!   devuelve como [`TimeHint::Ambiguous`] y `window_start`/`window_end` solo
//!   los fija el usuario;
//! - lo negado ("no me duele la cabeza") va en `negated`, no como registro.
use crate::{
    ErrorCode, ValidationError,
    catalog::{BodyZone, Indicator, SleepQuality},
    interpret::{
        mentions_indicator, quote_number, quote_sleep_quality, quote_unit_compatible, quote_zone,
    },
    measurement::Measurement,
    validate_text,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Versión del contrato de propuesta.
pub const PROPOSAL_SCHEMA_VERSION: u32 = 1;

const QUOTE_TOLERANCE: f64 = 1e-9;

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

fn quoted(haystack: &str, quote: &str) -> bool {
    !quote.trim().is_empty() && haystack.contains(quote)
}

fn valid_numeric_value(key: &str, val: &Value) -> Option<f64> {
    let x = val.as_f64()?;
    if !x.is_finite() {
        return None;
    }
    let ok = match key {
        "bpm" | "systolic" | "diastolic" | "celsius" | "mg_dl" => x > 0.0,
        "hours" => x > 0.0 && x <= 24.0,
        "percentage" => x > 0.0 && x <= 100.0,
        "usv_h" | "km" | "kcal" => x >= 0.0,
        "score" => (0.0..=10.0).contains(&x) && x.fract() == 0.0,
        _ => false,
    };
    ok.then_some(x)
}

fn check_present_field(
    at: Option<usize>,
    input: &str,
    rec: &ProposedRecord,
    f: &ProposedField,
    val: &Value,
    out: &mut Vec<ProposalIssue>,
) {
    if matches!(f.key.as_str(), "window_start" | "window_end") {
        out.push(issue(
            at,
            &f.key,
            ErrorCode::InvalidValue,
            "Sleep window endpoints must be set by the user",
        ));
        return;
    }

    let valid_quote = f
        .quote
        .as_deref()
        .filter(|fq| quoted(input, fq) && quoted(&rec.quote, fq));
    if valid_quote.is_none() {
        out.push(issue(
            at,
            &f.key,
            ErrorCode::InvalidValue,
            "Every value needs a quote taken from the record quote and input text",
        ));
    }

    match f.key.as_str() {
        "bpm" | "systolic" | "diastolic" | "celsius" | "mg_dl" | "hours" | "percentage"
        | "usv_h" | "km" | "kcal" | "score" => {
            let Some(num) = valid_numeric_value(&f.key, val) else {
                out.push(issue(
                    at,
                    &f.key,
                    ErrorCode::InvalidValue,
                    "Field value is outside the allowed type or range",
                ));
                return;
            };
            if let Some(fq) = valid_quote {
                let backed = quote_number(fq).is_some_and(|q| (q - num).abs() <= QUOTE_TOLERANCE)
                    && quote_unit_compatible(rec.indicator, fq);
                if !backed {
                    out.push(issue(
                        at,
                        &f.key,
                        ErrorCode::InvalidValue,
                        "Numeric value is not backed by its quote",
                    ));
                }
            }
        }
        "quality" => {
            let Some(quality) = val.as_str().and_then(|s| SleepQuality::parse(s).ok()) else {
                out.push(issue(
                    at,
                    &f.key,
                    ErrorCode::InvalidValue,
                    "Invalid sleep quality",
                ));
                return;
            };
            if let Some(fq) = valid_quote
                && quote_sleep_quality(fq) != Some(quality)
            {
                out.push(issue(
                    at,
                    &f.key,
                    ErrorCode::InvalidValue,
                    "Sleep quality is not backed by its quote",
                ));
            }
        }
        "body_zone" => {
            let Some(zone) = val.as_str().and_then(|s| BodyZone::parse(s).ok()) else {
                out.push(issue(
                    at,
                    &f.key,
                    ErrorCode::InvalidValue,
                    "Invalid body zone",
                ));
                return;
            };
            if let Some(fq) = valid_quote
                && quote_zone(fq) != Some(zone)
            {
                out.push(issue(
                    at,
                    &f.key,
                    ErrorCode::InvalidValue,
                    "Body zone is not backed by its quote",
                ));
            }
        }
        "description" => {
            let Some(desc) = val.as_str() else {
                out.push(issue(
                    at,
                    &f.key,
                    ErrorCode::InvalidValue,
                    "Invalid description",
                ));
                return;
            };
            if desc.trim().is_empty() || validate_text("description", desc, 2000).is_err() {
                out.push(issue(
                    at,
                    &f.key,
                    ErrorCode::InvalidValue,
                    "Invalid description",
                ));
                return;
            }
            if !input.contains(desc) || !rec.quote.contains(desc) {
                out.push(issue(
                    at,
                    &f.key,
                    ErrorCode::InvalidValue,
                    "Description must be a literal substring of the record quote and input text",
                ));
            }
        }
        _ => {}
    }
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
    /// validación del núcleo.
    ///
    /// Importante: `is_complete()` es **únicamente una condición de completitud**,
    /// nunca un permiso para guardar automáticamente; todo registro requiere
    /// confirmación explícita del usuario y superar [`Proposal::check`].
    pub fn is_complete(&self) -> bool {
        self.fields.iter().all(|f| f.value.is_some())
            && Measurement::validate(self.indicator, &self.value(), None).is_ok()
    }
}

impl Proposal {
    /// Comprueba la propuesta contra el texto de entrada y el contrato del
    /// núcleo. Devuelve todos los problemas; una lista vacía significa que la
    /// estructura y el respaldo de cada campo son válidos (no que la
    /// interpretación sea correcta: eso lo confirma el usuario).
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
            } else if !mentions_indicator(rec.indicator, &rec.quote) {
                out.push(issue(
                    at,
                    "quote",
                    ErrorCode::InvalidValue,
                    "Record quote does not mention the indicator",
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
                let is_allowed = allowed.contains(&f.key.as_str());
                if !is_allowed {
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
                if is_allowed && let Some(val) = &f.value {
                    check_present_field(at, input, rec, f, val, &mut out);
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

    #[test]
    fn adversarial_pulse_citing_another_number_is_rejected() {
        // Revisión 1: "dormí 6 horas, pulso 72" con pulso 6 citando "6" dentro de rec.quote "pulso 72"
        let p = pulse("pulso 72", json!(6), Some("6"));
        assert!(!p.check("dormí 6 horas, pulso 72").is_empty());

        // Revisión 2: "I read chapter 6; my pulse is unknown." con bpm 180 citando "6"
        let p2: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "heart_rate",
                "fields": [{"key": "bpm", "value": 180, "quote": "6"}],
                "time": {"kind": "unspecified"},
                "quote": "pulse"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(
            !p2.check("I read chapter 6; my pulse is unknown.")
                .is_empty()
        );
    }

    #[test]
    fn adversarial_bpm_999_citing_72_is_rejected() {
        let p = pulse("pulso 72", json!(999), Some("72"));
        assert!(!p.check("pulso 72").is_empty());
    }

    #[test]
    fn adversarial_symptom_wrong_zone_score_and_invented_description_are_rejected() {
        // body_zone: "left_foot" citada por "cabeza", description inventada
        let p: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "symptom",
                "fields": [
                    {"key": "body_zone", "value": "left_foot", "quote": "cabeza"},
                    {"key": "score", "value": 6, "quote": "6"},
                    {"key": "description", "value": "infarto confirmado", "quote": "cabeza"}
                ],
                "time": {"kind": "unspecified"},
                "quote": "me duele la cabeza 6"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        let issues = p.check("me duele la cabeza 6");
        assert!(issues.iter().any(|i| i.error.field == "body_zone"));
        assert!(issues.iter().any(|i| i.error.field == "description"));

        // Revisión 2: "No headache; chapter 6." con chest/headache, score=10/"6", description inventada
        let p2: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "symptom",
                "fields": [
                    {"key": "body_zone", "value": "chest", "quote": "headache"},
                    {"key": "score", "value": 10, "quote": "6"},
                    {"key": "description", "value": "Confirmed thrombosis; take aspirin", "quote": "headache"}
                ],
                "time": {"kind": "unspecified"},
                "quote": "headache"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        let issues2 = p2.check("No headache; chapter 6.");
        assert!(issues2.iter().any(|i| i.error.field == "body_zone"));
        assert!(issues2.iter().any(|i| i.error.field == "score"));
        assert!(issues2.iter().any(|i| i.error.field == "description"));
    }

    #[test]
    fn adversarial_partial_sleep_validates_hours_range_when_other_fields_are_none() {
        // hours: -1 con otros campos None
        let p_neg: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "sleep",
                "fields": [
                    {"key": "hours", "value": -1, "quote": "-1"},
                    {"key": "quality", "value": null, "quote": null},
                    {"key": "window_start", "value": null, "quote": null},
                    {"key": "window_end", "value": null, "quote": null}
                ],
                "time": {"kind": "unspecified"},
                "quote": "dormí -1 horas"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        let issues_neg = p_neg.check("dormí -1 horas");
        assert!(issues_neg.iter().any(|i| i.error.field == "hours"));

        // hours: 999 con otros campos None
        let p_999: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "sleep",
                "fields": [
                    {"key": "hours", "value": 999, "quote": "6"},
                    {"key": "quality", "value": null, "quote": null},
                    {"key": "window_start", "value": null, "quote": null},
                    {"key": "window_end", "value": null, "quote": null}
                ],
                "time": {"kind": "unspecified"},
                "quote": "I slept 6 hours"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        let issues_999 = p_999.check("I slept 6 hours");
        assert!(issues_999.iter().any(|i| i.error.field == "hours"));

        // hours: 0 con quality: None (hours debe ser > 0)
        let p_zero: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "sleep",
                "fields": [
                    {"key": "hours", "value": 0, "quote": "0"},
                    {"key": "quality", "value": null, "quote": null},
                    {"key": "window_start", "value": null, "quote": null},
                    {"key": "window_end", "value": null, "quote": null}
                ],
                "time": {"kind": "unspecified"},
                "quote": "dormí 0 horas"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(
            p_zero
                .check("dormí 0 horas")
                .iter()
                .any(|i| i.error.field == "hours")
        );
    }

    #[test]
    fn adversarial_invented_sleep_windows_citing_anoche_are_rejected() {
        let p: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "sleep",
                "fields": [
                    {"key": "hours", "value": 6, "quote": "6"},
                    {"key": "quality", "value": null, "quote": null},
                    {"key": "window_start", "value": "2026-01-01T00:00:00Z", "quote": "anoche"},
                    {"key": "window_end", "value": "2026-01-01T06:00:00Z", "quote": "anoche"}
                ],
                "time": {"kind": "ambiguous", "quote": "anoche"},
                "quote": "dormí 6 horas anoche"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        let issues = p.check("dormí 6 horas anoche");
        assert!(issues.iter().any(|i| i.error.field == "window_start"));
        assert!(issues.iter().any(|i| i.error.field == "window_end"));
    }

    #[test]
    fn adversarial_partial_invalid_enums_types_and_record_quote_without_indicator() {
        // Symptom parcial con score decimal e indicador ausente en rec.quote
        let p: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "symptom",
                "fields": [
                    {"key": "body_zone", "value": null, "quote": null},
                    {"key": "score", "value": 4.5, "quote": "4.5"},
                    {"key": "description", "value": null, "quote": null}
                ],
                "time": {"kind": "unspecified"},
                "quote": "4.5"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        let issues = p.check("tengo 4.5");
        assert!(issues.iter().any(|i| i.error.field == "quote"));
        assert!(issues.iter().any(|i| i.error.field == "score"));

        // Sleep parcial con quality inválida y error sin filtrar valores del usuario
        let p_bad_q: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "sleep",
                "fields": [
                    {"key": "hours", "value": null, "quote": null},
                    {"key": "quality", "value": "EXCELENTE_SECRETO", "quote": "bien"},
                    {"key": "window_start", "value": null, "quote": null},
                    {"key": "window_end", "value": null, "quote": null}
                ],
                "time": {"kind": "unspecified"},
                "quote": "dormí bien"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        let issues_q = p_bad_q.check("dormí bien");
        assert!(issues_q.iter().any(|i| i.error.field == "quality"));
        assert!(
            issues_q
                .iter()
                .all(|i| !i.error.message.contains("EXCELENTE_SECRETO"))
        );

        // Unidad cruzada en la cita numérica: bpm citando "36.8 °C"
        let p_cross_unit = pulse(
            "pulso 72, temperatura 36.8 °C",
            json!(36.8),
            Some("36.8 °C"),
        );
        assert!(
            !p_cross_unit
                .check("pulso 72, temperatura 36.8 °C")
                .is_empty()
        );
    }
}
