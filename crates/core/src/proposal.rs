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
        AnalyzedSegment, BoundedOccurrence, InputAnalysis, analyze_input_for_check,
        check_field_attribution_in_segment, check_numeric_unit_around_occurrence,
        find_token_bounded_occurrences_with_tokens, has_token_bounded_occurrence,
        is_single_clause_quote, mentions_indicator, occurrence_is_negated_in_input, quote_number,
        quote_sleep_quality, quote_unit_compatible, quote_zone,
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
        "hours" => (0.0..=24.0).contains(&x),
        "percentage" => x > 0.0 && x <= 100.0,
        "usv_h" | "km" | "kcal" => x >= 0.0,
        "score" => (0.0..=10.0).contains(&x) && x.fract() == 0.0,
        _ => false,
    };
    ok.then_some(x)
}

fn is_numeric_field_key(key: &str) -> bool {
    matches!(
        key,
        "bpm"
            | "systolic"
            | "diastolic"
            | "celsius"
            | "mg_dl"
            | "hours"
            | "percentage"
            | "usv_h"
            | "km"
            | "kcal"
            | "score"
    )
}

fn json_values_compatible(key: &str, proposed: &Value, l1: &Value) -> bool {
    if is_numeric_field_key(key)
        && let (Some(a), Some(b)) = (valid_numeric_value(key, proposed), l1.as_f64())
    {
        return (a - b).abs() <= QUOTE_TOLERANCE;
    }
    if let (Some(a), Some(b)) = (proposed.as_str(), l1.as_str()) {
        return a.eq_ignore_ascii_case(b);
    }
    proposed == l1
}

#[allow(clippy::too_many_arguments)]
fn check_field_occurrence_against_analysis(
    input: &str,
    analysis: &InputAnalysis,
    rec: &ProposedRecord,
    valid_rec_occs: &[BoundedOccurrence],
    f: &ProposedField,
    val: &Value,
    fq: &str,
    fq_occ: &BoundedOccurrence,
    is_numeric: bool,
) -> bool {
    if !valid_rec_occs.is_empty()
        && !valid_rec_occs
            .iter()
            .any(|ro| fq_occ.first_tok >= ro.first_tok && fq_occ.last_tok <= ro.last_tok)
    {
        return false;
    }

    if is_numeric
        && !check_numeric_unit_around_occurrence(
            input,
            &analysis.tokens,
            rec.indicator,
            fq,
            fq_occ.first_tok,
            fq_occ.last_tok,
        )
    {
        return false;
    }

    let seg_opt: Option<&AnalyzedSegment> = analysis
        .segments
        .iter()
        .find(|seg| seg.contains_occ(fq_occ));

    if let Some(seg) = seg_opt {
        if seg.indicator != rec.indicator || seg.negated || seg.field_is_ambiguous(&f.key) {
            return false;
        }
        let l1_val = seg
            .record_idx
            .and_then(|idx| analysis.proposal.records.get(idx))
            .and_then(|lr| lr.fields.iter().find(|lf| lf.key == f.key))
            .and_then(|lf| lf.value.as_ref());
        if let Some(l1_v) = l1_val {
            return json_values_compatible(&f.key, val, l1_v);
        }
        if is_numeric {
            return check_field_attribution_in_segment(
                input,
                analysis,
                seg,
                rec.indicator,
                &f.key,
                fq,
                fq_occ,
            );
        }
        return true;
    }

    // El reconocimiento cubre Indicator::ALL: un campo sin segmento local no tiene respaldo.
    // No se vuelve a analizar una cita recortada que pueda ocultar correcciones de la entrada.
    false
}

#[allow(clippy::too_many_arguments)]
fn check_present_field(
    at: Option<usize>,
    input: &str,
    analysis: &InputAnalysis,
    rec: &ProposedRecord,
    valid_rec_occs: &[BoundedOccurrence],
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

    let allow_bp_slash = rec.indicator == Indicator::BloodPressure
        && matches!(f.key.as_str(), "systolic" | "diastolic");
    let valid_quote = f.quote.as_deref().filter(|fq| {
        quoted(input, fq)
            && quoted(&rec.quote, fq)
            && has_token_bounded_occurrence(&rec.quote, fq, allow_bp_slash)
    });
    if valid_quote.is_none() {
        out.push(issue(
            at,
            &f.key,
            ErrorCode::InvalidValue,
            "Every value needs a quote taken on token boundaries from the record quote and input text",
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
                let fq_occs = find_token_bounded_occurrences_with_tokens(
                    input,
                    &analysis.tokens,
                    fq,
                    allow_bp_slash,
                );
                let matching_occ_and_unit = fq_occs.iter().any(|fq_occ| {
                    check_field_occurrence_against_analysis(
                        input,
                        analysis,
                        rec,
                        valid_rec_occs,
                        f,
                        val,
                        fq,
                        fq_occ,
                        true,
                    )
                });
                let backed = quote_number(fq).is_some_and(|q| (q - num).abs() <= QUOTE_TOLERANCE)
                    && quote_unit_compatible(rec.indicator, fq)
                    && matching_occ_and_unit;
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
            if let Some(fq) = valid_quote {
                let fq_occs =
                    find_token_bounded_occurrences_with_tokens(input, &analysis.tokens, fq, false);
                let matching_occ = fq_occs.iter().any(|fq_occ| {
                    check_field_occurrence_against_analysis(
                        input,
                        analysis,
                        rec,
                        valid_rec_occs,
                        f,
                        val,
                        fq,
                        fq_occ,
                        false,
                    )
                });
                if quote_sleep_quality(fq) != Some(quality) || !matching_occ {
                    out.push(issue(
                        at,
                        &f.key,
                        ErrorCode::InvalidValue,
                        "Sleep quality is not backed by its quote",
                    ));
                }
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
            if let Some(fq) = valid_quote {
                let fq_occs =
                    find_token_bounded_occurrences_with_tokens(input, &analysis.tokens, fq, false);
                let matching_occ = fq_occs.iter().any(|fq_occ| {
                    check_field_occurrence_against_analysis(
                        input,
                        analysis,
                        rec,
                        valid_rec_occs,
                        f,
                        val,
                        fq,
                        fq_occ,
                        false,
                    )
                });
                if quote_zone(fq) != Some(zone) || !matching_occ {
                    out.push(issue(
                        at,
                        &f.key,
                        ErrorCode::InvalidValue,
                        "Body zone is not backed by its quote",
                    ));
                }
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
        let analysis = analyze_input_for_check(input);
        for (i, rec) in self.records.iter().enumerate() {
            let at = Some(i);
            let rec_occs = find_token_bounded_occurrences_with_tokens(
                input,
                &analysis.tokens,
                &rec.quote,
                false,
            );
            let valid_rec_occs: Vec<BoundedOccurrence> = rec_occs
                .iter()
                .copied()
                .filter(|occ| !occurrence_is_negated_in_input(&analysis, input, occ, rec.indicator))
                .collect();

            if !quoted(input, &rec.quote) || rec_occs.is_empty() {
                out.push(issue(
                    at,
                    "quote",
                    ErrorCode::InvalidValue,
                    "Quote is not a token-bounded part of the input text",
                ));
            } else if valid_rec_occs.is_empty() {
                out.push(issue(
                    at,
                    "quote",
                    ErrorCode::InvalidValue,
                    "Record quote is negated in the input text",
                ));
            } else if !is_single_clause_quote(&rec.quote) {
                out.push(issue(
                    at,
                    "quote",
                    ErrorCode::InvalidValue,
                    "Record quote must cover a single clause",
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
                && (!quoted(input, quote) || !has_token_bounded_occurrence(input, quote, false))
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
                    check_present_field(
                        at,
                        input,
                        &analysis,
                        rec,
                        &valid_rec_occs,
                        f,
                        val,
                        &mut out,
                    );
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

        // hours: 0 es válido según units::Hours (0.0..=24.0)
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
        assert!(p_zero.check("dormí 0 horas").is_empty());
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

    #[test]
    fn adversarial_check_token_boundaries_reject_substrings() {
        // 172 -> 72 en rec.quote o en field.quote
        let p_sub_rec = pulse("72", json!(72), Some("72"));
        assert!(
            p_sub_rec
                .check("pulso 172")
                .iter()
                .any(|i| i.error.field == "quote")
        );

        let p_sub_field = pulse("pulso 172", json!(72), Some("72"));
        assert!(
            p_sub_field
                .check("pulso 172")
                .iter()
                .any(|i| i.error.field == "bpm")
        );

        // "6" dentro de "6/7" no vale para symptom.score ni heart_rate
        let p_slash: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "symptom",
                "fields": [
                    {"key": "body_zone", "value": "head", "quote": "cabeza"},
                    {"key": "score", "value": 6, "quote": "6"},
                    {"key": "description", "value": "dolor de cabeza 6/7", "quote": "dolor de cabeza 6/7"}
                ],
                "time": {"kind": "unspecified"},
                "quote": "dolor de cabeza 6/7"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(
            p_slash
                .check("dolor de cabeza 6/7")
                .iter()
                .any(|i| i.error.field == "score")
        );

        // Pero presión arterial "120/80" con citas "120" y "80" sigue siendo válida
        let p_bp: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "blood_pressure",
                "fields": [
                    {"key": "systolic", "value": 120, "quote": "120"},
                    {"key": "diastolic", "value": 80, "quote": "80"}
                ],
                "time": {"kind": "unspecified"},
                "quote": "presión 120/80"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(p_bp.check("presión 120/80").is_empty());
    }

    #[test]
    fn adversarial_check_single_clause_scope_blocks_cross_clause_borrowing() {
        let p_cross = pulse("dormí 6 horas, pulso 72", json!(6), Some("6"));
        let issues = p_cross.check("dormí 6 horas, pulso 72");
        assert!(issues.iter().any(|i| i.error.field == "quote"));

        let p_semi = pulse("dormí 6 horas; pulso", json!(6), Some("6"));
        assert!(
            p_semi
                .check("dormí 6 horas; pulso")
                .iter()
                .any(|i| i.error.field == "quote")
        );

        let p_pero = pulse("dormí 6 horas pero pulso", json!(6), Some("6"));
        assert!(
            p_pero
                .check("dormí 6 horas pero pulso")
                .iter()
                .any(|i| i.error.field == "quote")
        );

        let p_y_luego = pulse("dormí 6 horas y luego pulso", json!(6), Some("6"));
        assert!(
            p_y_luego
                .check("dormí 6 horas y luego pulso")
                .iter()
                .any(|i| i.error.field == "quote")
        );
    }

    #[test]
    fn adversarial_check_rejects_incompatible_and_unknown_units_around_occurrence() {
        // "98.6 F" con rec.quote = "temperatura 98.6" y fq = "98.6"
        let p_temp_f: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "body_temperature",
                "fields": [
                    {"key": "celsius", "value": 98.6, "quote": "98.6"}
                ],
                "time": {"kind": "unspecified"},
                "quote": "temperatura 98.6"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(
            p_temp_f
                .check("temperatura 98.6 F")
                .iter()
                .any(|i| i.error.field == "celsius")
        );
        assert!(
            p_temp_f
                .check("temperatura 98.6 degF")
                .iter()
                .any(|i| i.error.field == "celsius")
        );
        assert!(
            p_temp_f
                .check("temperatura 98.6 ℉")
                .iter()
                .any(|i| i.error.field == "celsius")
        );
        assert!(
            p_temp_f
                .check("temperatura 98.6x")
                .iter()
                .any(|i| i.error.field == "celsius")
        );

        // /min u otra unidad desconocida tras el número
        let p_pulse_slash = pulse("pulso 72", json!(72), Some("72"));
        assert!(
            p_pulse_slash
                .check("pulso 72/min")
                .iter()
                .any(|i| i.error.field == "bpm")
        );
    }

    #[test]
    fn adversarial_check_rejects_negated_mentions_and_layer1_conflicts() {
        // "no medí pulso 72" con negated = []
        let p_neg_es = pulse("pulso 72", json!(72), Some("72"));
        assert!(
            p_neg_es
                .check("no medí pulso 72")
                .iter()
                .any(|i| i.error.field == "quote")
        );

        // "I can't measure pulse 72" con negated = []
        let p_neg_en = pulse("pulse 72", json!(72), Some("72"));
        assert!(
            p_neg_en
                .check("I can't measure pulse 72")
                .iter()
                .any(|i| i.error.field == "quote")
        );

        // "No headache; chapter 6."
        let p_no_head: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "symptom",
                "fields": [
                    {"key": "body_zone", "value": "head", "quote": "headache"},
                    {"key": "score", "value": 6, "quote": "6"},
                    {"key": "description", "value": "headache", "quote": "headache"}
                ],
                "time": {"kind": "unspecified"},
                "quote": "headache"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        let issues_head = p_no_head.check("No headache; chapter 6.");
        assert!(issues_head.iter().any(|i| i.error.field == "quote"));
        assert!(issues_head.iter().any(|i| i.error.field == "score"));

        // Contraste con capa 1: mismo indicador con valor distinto ("tras 6 ejercicios pulso 72" -> bpm=6)
        let p_conflict = pulse("tras 6 ejercicios pulso 72", json!(6), Some("6"));
        let issues_conflict = p_conflict.check("tras 6 ejercicios pulso 72");
        assert!(issues_conflict.iter().any(|i| i.error.field == "bpm"));
        assert!(
            issues_conflict
                .iter()
                .all(|i| !i.error.message.contains("ejercicios") && !i.error.message.contains("72"))
        );

        // Contraste con capa 1 en Sleep: "no dormí bien" con quality="good"
        let p_sleep_bad: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "sleep",
                "fields": [
                    {"key": "hours", "value": null, "quote": null},
                    {"key": "quality", "value": "good", "quote": "bien"},
                    {"key": "window_start", "value": null, "quote": null},
                    {"key": "window_end", "value": null, "quote": null}
                ],
                "time": {"kind": "unspecified"},
                "quote": "no dormí bien"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(
            p_sleep_bad
                .check("no dormí bien")
                .iter()
                .any(|i| i.error.field == "quality")
        );
    }

    #[test]
    fn n1_check_validates_attribution_correction_and_full_unit_in_input() {
        // 1. Atribución que cruza predicado ("trabajé 5 horas antes de dormir mal" con hours=5)
        let p_sleep_attr: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "sleep",
                "fields": [
                    {"key": "hours", "value": 5, "quote": "5 horas"},
                    {"key": "quality", "value": "poor", "quote": "mal"},
                    {"key": "window_start", "value": null, "quote": null},
                    {"key": "window_end", "value": null, "quote": null}
                ],
                "time": {"kind": "unspecified"},
                "quote": "trabajé 5 horas antes de dormir mal"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(
            p_sleep_attr
                .check("trabajé 5 horas antes de dormir mal")
                .iter()
                .any(|i| i.error.field == "hours")
        );

        // 2. Atribución tras predicado ajeno ("pulso desconocido tras 6" con bpm=6; también in-range 60)
        let p_pulse_6 = pulse("pulso desconocido tras 6", json!(6), Some("6"));
        assert!(
            p_pulse_6
                .check("pulso desconocido tras 6")
                .iter()
                .any(|i| i.error.field == "bpm")
        );
        let p_pulse_60 = pulse("pulso desconocido tras 60", json!(60), Some("60"));
        assert!(
            p_pulse_60
                .check("pulso desconocido tras 60")
                .iter()
                .any(|i| i.error.field == "bpm")
        );

        // 3. Corrección contigua entre cláusulas ("pulso 72, perdón pulso 82" citando solo "pulso 72")
        let p_corr_72 = pulse("pulso 72", json!(72), Some("72"));
        assert!(
            p_corr_72
                .check("pulso 72, perdón pulso 82")
                .iter()
                .any(|i| i.error.field == "bpm")
        );
        let p_corr_82 = pulse("pulso 82", json!(82), Some("82"));
        assert!(
            p_corr_82
                .check("pulso 72, perdón pulso 82")
                .iter()
                .any(|i| i.error.field == "bpm")
        );

        // 4. Unidad dudosa con cola ("temperatura 37 °C/kg" citando "37")
        let p_temp_tail: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "body_temperature",
                "fields": [
                    {"key": "celsius", "value": 37, "quote": "37"}
                ],
                "time": {"kind": "unspecified"},
                "quote": "temperatura 37"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(
            p_temp_tail
                .check("temperatura 37 °C/kg")
                .iter()
                .any(|i| i.error.field == "celsius")
        );

        // 5. Contraste válido: relación local inequívoca sin valor en capa 1 (p. ej. radiación espacial)
        let p_rad: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "space_radiation",
                "fields": [
                    {"key": "usv_h", "value": 0.42, "quote": "0.42 uSv/h"}
                ],
                "time": {"kind": "unspecified"},
                "quote": "radiación 0.42 uSv/h"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(p_rad.check("radiación 0.42 uSv/h").is_empty());
    }

    #[test]
    fn n2_check_respects_next_indicator_negation_boundaries() {
        let p_valid = pulse("pulso 72", json!(72), Some("72"));
        assert!(p_valid.check("pulso 72 sin medir glucosa").is_empty());

        let p_gluc: Proposal = serde_json::from_value(json!({
            "schema_version": 1,
            "interpreter": "slm",
            "interpreter_version": "v1",
            "records": [{
                "indicator": "glucose",
                "fields": [
                    {"key": "mg_dl", "value": null, "quote": null}
                ],
                "time": {"kind": "unspecified"},
                "quote": "glucosa"
            }],
            "negated": [],
            "uninterpreted": []
        }))
        .unwrap();
        assert!(
            p_gluc
                .check("pulso 72 sin medir glucosa")
                .iter()
                .any(|i| i.error.field == "quote")
        );
    }

    #[test]
    fn n4_check_rejects_separated_unknown_units_and_keeps_trailing_prose() {
        for (txt, quote_rec, fq) in [
            ("temperatura 37 fahrenheits", "temperatura 37", "37"),
            ("temperatura 37 farenheit", "temperatura 37", "37"),
            ("temperatura 37 grados f", "temperatura 37", "37"),
            (
                "temperatura 37 grados f",
                "temperatura 37 grados",
                "37 grados",
            ),
        ] {
            let p_bad: Proposal = serde_json::from_value(json!({
                "schema_version": 1,
                "interpreter": "slm",
                "interpreter_version": "v1",
                "records": [{
                    "indicator": "body_temperature",
                    "fields": [{"key": "celsius", "value": 37, "quote": fq}],
                    "time": {"kind": "unspecified"},
                    "quote": quote_rec
                }],
                "negated": [],
                "uninterpreted": []
            }))
            .unwrap();
            assert!(
                p_bad.check(txt).iter().any(|i| i.error.field == "celsius"),
                "expected rejection for {txt:?} with quote {fq:?}"
            );
        }

        let p_ok = pulse("pulso 72 ahora", json!(72), Some("72"));
        assert!(p_ok.check("pulso 72 ahora").is_empty());
    }

    fn slm_numeric(
        indicator: Indicator,
        key: &str,
        value: Value,
        record_quote: &str,
        field_quote: &str,
    ) -> Proposal {
        Proposal {
            schema_version: PROPOSAL_SCHEMA_VERSION,
            interpreter: InterpreterKind::Slm,
            interpreter_version: "revision4-test".into(),
            records: vec![ProposedRecord {
                indicator,
                fields: allowed_fields(indicator)
                    .iter()
                    .map(|&field_key| ProposedField {
                        key: field_key.into(),
                        value: (field_key == key).then(|| value.clone()),
                        quote: (field_key == key).then(|| field_quote.into()),
                    })
                    .collect(),
                time: TimeHint::Unspecified,
                quote: record_quote.into(),
            }],
            negated: vec![],
            uninterpreted: vec![],
        }
    }

    #[test]
    fn revision4_corrections_use_original_mentions_for_every_indicator() {
        // Citas recortadas de las DOS ocurrencias: ninguna habilita un valor corregido.
        // Casos escritos a mano; la tabla cubre exactamente Indicator::ALL.
        let cases = [
            (
                Indicator::HeartRate,
                "bpm",
                "pulso 72",
                "72",
                json!(72),
                "pulso 82",
                "82",
                json!(82),
                "pulso 72, perdón pulso 82",
            ),
            (
                Indicator::BloodPressure,
                "systolic",
                "presión 120/80",
                "120",
                json!(120),
                "presión 130/85",
                "130",
                json!(130),
                "presión 120/80, perdón presión 130/85",
            ),
            (
                Indicator::BodyTemperature,
                "celsius",
                "temperatura 37 °C",
                "37 °C",
                json!(37),
                "temperatura 38 °C",
                "38 °C",
                json!(38),
                "temperatura 37 °C, perdón temperatura 38 °C",
            ),
            (
                Indicator::Glucose,
                "mg_dl",
                "glucosa 95 mg/dL",
                "95 mg/dL",
                json!(95),
                "glucosa 105 mg/dL",
                "105 mg/dL",
                json!(105),
                "glucosa 95 mg/dL, perdón glucosa 105 mg/dL",
            ),
            (
                Indicator::Sleep,
                "hours",
                "dormí 5 horas",
                "5 horas",
                json!(5),
                "dormí 7 horas",
                "7 horas",
                json!(7),
                "dormí 5 horas, perdón dormí 7 horas",
            ),
            (
                Indicator::Mood,
                "score",
                "ánimo 5/10",
                "5/10",
                json!(5),
                "ánimo 7/10",
                "7/10",
                json!(7),
                "ánimo 5/10, perdón ánimo 7/10",
            ),
            (
                Indicator::OxygenInBlood,
                "percentage",
                "saturación 98%",
                "98%",
                json!(98),
                "saturación 97%",
                "97%",
                json!(97),
                "saturación 98%, perdón saturación 97%",
            ),
            (
                Indicator::SpaceRadiation,
                "usv_h",
                "radiación 0.42 uSv/h",
                "0.42 uSv/h",
                json!(0.42),
                "radiación 0.84 uSv/h",
                "0.84 uSv/h",
                json!(0.84),
                "radiación 0.42 uSv/h, perdón radiación 0.84 uSv/h",
            ),
            (
                Indicator::DistanceFromEarth,
                "km",
                "distancia 420 km",
                "420 km",
                json!(420),
                "distancia 840 km",
                "840 km",
                json!(840),
                "distancia 420 km, perdón distancia 840 km",
            ),
            (
                Indicator::Nutrition,
                "kcal",
                "nutrición 2000 kcal",
                "2000 kcal",
                json!(2000),
                "nutrición 2200 kcal",
                "2200 kcal",
                json!(2200),
                "nutrición 2000 kcal, perdón nutrición 2200 kcal",
            ),
            (
                Indicator::Symptom,
                "score",
                "dolor de cabeza 5/10",
                "5/10",
                json!(5),
                "dolor de cabeza 7/10",
                "7/10",
                json!(7),
                "dolor de cabeza 5/10, perdón dolor de cabeza 7/10",
            ),
        ];
        assert_eq!(cases.len(), Indicator::ALL.len());
        for indicator in Indicator::ALL {
            assert_eq!(cases.iter().filter(|case| case.0 == indicator).count(), 1);
        }
        for (indicator, key, first, fq1, v1, second, fq2, v2, input) in cases {
            let first_proposal = slm_numeric(indicator, key, v1, first, fq1);
            let second_proposal = slm_numeric(indicator, key, v2, second, fq2);
            for (proposal, isolated) in [(&first_proposal, first), (&second_proposal, second)] {
                assert!(proposal.check(isolated).is_empty(), "{isolated:?}");
                assert!(
                    proposal
                        .check(input)
                        .iter()
                        .any(|issue| issue.error.field == key),
                    "trimmed quote accepted for {input:?}: {proposal:?}"
                );
            }
            assert!(crate::interpret::interpret(input).check(input).is_empty());

            // Tres indicadores intermedios, distintos del corregido; ambos órdenes de valores.
            let intermediate = if matches!(
                indicator,
                Indicator::HeartRate | Indicator::Glucose | Indicator::Mood
            ) {
                "temperatura 36 °C y distancia 420 km y nutrición 2000 kcal"
            } else {
                "pulso 72 y glucosa 95 mg/dL y ánimo 6/10"
            };
            for (old, new) in [(first, second), (second, first)] {
                for marker in [
                    ", perdón ",
                    " sorry ",
                    ", mejor dicho ",
                    ", I mean ",
                    ", quiero decir ",
                    ", perdón, ",
                ] {
                    let interleaved = format!("{old} y {intermediate}{marker}{new}");
                    for proposal in [&first_proposal, &second_proposal] {
                        assert!(
                            proposal
                                .check(&interleaved)
                                .iter()
                                .any(|issue| issue.error.field == key),
                            "corrected value accepted: {interleaved:?}: {proposal:?}"
                        );
                    }
                    let rules = crate::interpret::interpret(&interleaved);
                    assert!(rules.check(&interleaved).is_empty(), "{interleaved:?}");
                    assert!(
                        rules
                            .records
                            .iter()
                            .filter(|r| r.indicator == indicator)
                            .all(|r| r
                                .fields
                                .iter()
                                .filter(|f| f.key != "description")
                                .all(|f| f.value.is_none()))
                    );
                    // Valores intermedios escritos a mano, independientes del resultado de interpret.
                    let preserved = if matches!(
                        indicator,
                        Indicator::HeartRate | Indicator::Glucose | Indicator::Mood
                    ) {
                        [
                            slm_numeric(
                                Indicator::BodyTemperature,
                                "celsius",
                                json!(36),
                                "temperatura 36 °C",
                                "36 °C",
                            ),
                            slm_numeric(
                                Indicator::DistanceFromEarth,
                                "km",
                                json!(420),
                                "distancia 420 km",
                                "420 km",
                            ),
                            slm_numeric(
                                Indicator::Nutrition,
                                "kcal",
                                json!(2000),
                                "nutrición 2000 kcal",
                                "2000 kcal",
                            ),
                        ]
                    } else {
                        [
                            slm_numeric(Indicator::HeartRate, "bpm", json!(72), "pulso 72", "72"),
                            slm_numeric(
                                Indicator::Glucose,
                                "mg_dl",
                                json!(95),
                                "glucosa 95 mg/dL",
                                "95 mg/dL",
                            ),
                            slm_numeric(Indicator::Mood, "score", json!(6), "ánimo 6/10", "6/10"),
                        ]
                    };
                    for expected in preserved {
                        assert!(
                            expected.check(&interleaved).is_empty(),
                            "{interleaved:?}: {expected:?}"
                        );
                        let expected_rec = &expected.records[0];
                        if !matches!(
                            expected_rec.indicator,
                            Indicator::DistanceFromEarth | Indicator::Nutrition
                        ) {
                            let actual = rules
                                .records
                                .iter()
                                .find(|r| r.indicator == expected_rec.indicator)
                                .unwrap();
                            assert_eq!(
                                actual
                                    .fields
                                    .iter()
                                    .find(|f| f.key == expected_rec.fields[0].key)
                                    .unwrap()
                                    .value,
                                expected_rec.fields[0].value,
                                "{interleaved:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn revision4_context_corrections_in_both_orders_and_clause_forms() {
        let cases = [
            (
                Indicator::SpaceRadiation,
                "usv_h",
                json!(0.42),
                "radiación 0.42 uSv/h",
                "0.42 uSv/h",
                [
                    "radiación 0.42 uSv/h, perdón radiación 0.84 uSv/h",
                    "radiación 0.84 uSv/h, perdón radiación 0.42 uSv/h",
                    "radiación 0.42 uSv/h perdón radiación 0.84 uSv/h",
                    "radiación 0.84 uSv/h perdón radiación 0.42 uSv/h",
                    "radiación 0.42 uSv/h, perdón, radiación 0.84 uSv/h",
                    "radiación 0.84 uSv/h, perdón, radiación 0.42 uSv/h",
                ],
            ),
            (
                Indicator::DistanceFromEarth,
                "km",
                json!(420),
                "distancia 420 km",
                "420 km",
                [
                    "distancia 420 km, perdón distancia 840 km",
                    "distancia 840 km, perdón distancia 420 km",
                    "distancia 420 km perdón distancia 840 km",
                    "distancia 840 km perdón distancia 420 km",
                    "distancia 420 km, perdón, distancia 840 km",
                    "distancia 840 km, perdón, distancia 420 km",
                ],
            ),
            (
                Indicator::Nutrition,
                "kcal",
                json!(2000),
                "nutrición 2000 kcal",
                "2000 kcal",
                [
                    "nutrición 2000 kcal, perdón nutrición 2200 kcal",
                    "nutrición 2200 kcal, perdón nutrición 2000 kcal",
                    "nutrición 2000 kcal perdón nutrición 2200 kcal",
                    "nutrición 2200 kcal perdón nutrición 2000 kcal",
                    "nutrición 2000 kcal, perdón, nutrición 2200 kcal",
                    "nutrición 2200 kcal, perdón, nutrición 2000 kcal",
                ],
            ),
        ];
        for (indicator, key, value, record_quote, field_quote, inputs) in cases {
            let proposal = slm_numeric(indicator, key, value, record_quote, field_quote);
            assert!(proposal.check(record_quote).is_empty());
            for input in inputs {
                assert!(
                    proposal
                        .check(input)
                        .iter()
                        .any(|issue| issue.error.field == key),
                    "{input:?}"
                );
                assert!(crate::interpret::interpret(input).check(input).is_empty());
            }
        }
    }

    #[test]
    fn revision5_radiation_correction_orders_keep_pulse_72() {
        let old = slm_numeric(
            Indicator::SpaceRadiation,
            "usv_h",
            json!(0.42),
            "radiación 0.42 uSv/h",
            "0.42 uSv/h",
        );
        let new = slm_numeric(
            Indicator::SpaceRadiation,
            "usv_h",
            json!(0.84),
            "radiación 0.84 uSv/h",
            "0.84 uSv/h",
        );
        let pulse = slm_numeric(Indicator::HeartRate, "bpm", json!(72), "pulso 72", "72");
        // Valores y órdenes literales; ningún esperado procede del intérprete.
        for input in [
            "radiación 0.42 uSv/h y pulso 72, perdón radiación 0.84 uSv/h",
            "radiación 0.84 uSv/h y pulso 72, perdón radiación 0.42 uSv/h",
            "pulso 72 y radiación 0.42 uSv/h, perdón radiación 0.84 uSv/h",
            "pulso 72 y radiación 0.84 uSv/h, perdón radiación 0.42 uSv/h",
            "radiación 0.42 uSv/h y pulso 72 y glucosa 95 mg/dL y ánimo 6/10, perdón radiación 0.84 uSv/h",
            "radiación 0.84 uSv/h y ánimo 6/10 y glucosa 95 mg/dL y pulso 72, perdón radiación 0.42 uSv/h",
            "radiación 0.42 uSv/h pulso 72 glucosa 95 mg/dL ánimo 6/10 perdón radiación 0.84 uSv/h",
            "radiación 0.84 uSv/h ánimo 6/10 glucosa 95 mg/dL pulso 72 sorry radiación 0.42 uSv/h",
            "radiación 0.42 uSv/h y pulso 72, perdón, radiación 0.84 uSv/h",
            "radiación 0.84 uSv/h y pulso 72, I mean, radiación 0.42 uSv/h",
        ] {
            for proposal in [&old, &new] {
                assert!(
                    proposal
                        .check(input)
                        .iter()
                        .any(|issue| issue.error.field == "usv_h"),
                    "{input:?}"
                );
            }
            assert!(pulse.check(input).is_empty(), "{input:?}");
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}");
            let rec = rules
                .records
                .iter()
                .find(|r| r.indicator == Indicator::HeartRate)
                .unwrap();
            assert_eq!(rec.value(), json!({"bpm": 72}), "{input:?}");
            assert!(
                rules
                    .records
                    .iter()
                    .all(|r| r.indicator != Indicator::SpaceRadiation)
            );
        }
    }

    #[test]
    fn revision5_corrections_use_latest_matching_segment_and_keep_ambiguity() {
        let rad21 = slm_numeric(
            Indicator::SpaceRadiation,
            "usv_h",
            json!(0.21),
            "radiación 0.21 uSv/h",
            "0.21 uSv/h",
        );
        let rad42 = slm_numeric(
            Indicator::SpaceRadiation,
            "usv_h",
            json!(0.42),
            "radiación 0.42 uSv/h",
            "0.42 uSv/h",
        );
        let rad84 = slm_numeric(
            Indicator::SpaceRadiation,
            "usv_h",
            json!(0.84),
            "radiación 0.84 uSv/h",
            "0.84 uSv/h",
        );
        let pulse72 = slm_numeric(Indicator::HeartRate, "bpm", json!(72), "pulso 72", "72");
        let glucose95 = slm_numeric(
            Indicator::Glucose,
            "mg_dl",
            json!(95),
            "glucosa 95 mg/dL",
            "95 mg/dL",
        );
        let input =
            "radiación 0.21 uSv/h y radiación 0.42 uSv/h y pulso 72, perdón radiación 0.84 uSv/h";
        assert!(rad21.check(input).is_empty());
        assert!(pulse72.check(input).is_empty());
        for p in [&rad42, &rad84] {
            assert!(p.check(input).iter().any(|i| i.error.field == "usv_h"));
        }

        // Sin etiqueta: solo se invalida la última mención, incluso si ya era ambigua.
        for input in [
            "radiación 0.42 uSv/h y pulso 72, perdón 82",
            "radiación 0.42 uSv/h y pulso 72, perdón, 82",
            "radiación 0.42 uSv/h y pulso 72, perdón",
            "radiación 0.42 uSv/h y pulso 72 o 73, sorry 82 o 83",
        ] {
            assert!(rad42.check(input).is_empty(), "{input:?}");
            assert!(
                pulse72.check(input).iter().any(|i| i.error.field == "bpm"),
                "{input:?}"
            );
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}");
            assert_eq!(rules.records.len(), 1, "{input:?}");
            assert_eq!(rules.records[0].indicator, Indicator::HeartRate);
            assert_eq!(rules.records[0].value(), json!({}), "{input:?}");
        }

        // Una corrección explícita no recurre al último indicador si no existe su destino.
        let input = "pulso 72, perdón radiación 0.84 uSv/h";
        assert!(pulse72.check(input).is_empty());
        assert!(rad84.check(input).iter().any(|i| i.error.field == "usv_h"));

        for input in [
            "radiación 0.42 uSv/h y glucosa 95 mg/dL y pulso 72, perdón radiación 0.84 uSv/h o glucosa 105 mg/dL",
            "radiación 0.42 uSv/h y glucosa 95 mg/dL y pulso 72, I mean glucosa 105 mg/dL or radiación 0.84 uSv/h",
        ] {
            let glucose105 = slm_numeric(
                Indicator::Glucose,
                "mg_dl",
                json!(105),
                "glucosa 105 mg/dL",
                "105 mg/dL",
            );
            for p in [&rad42, &rad84, &glucose95, &glucose105] {
                let key = &p.records[0].fields[0].key;
                assert!(
                    p.check(input).iter().any(|i| i.error.field == *key),
                    "{input:?}: {p:?}"
                );
            }
            assert!(pulse72.check(input).is_empty(), "{input:?}");
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}");
            assert_eq!(
                rules
                    .records
                    .iter()
                    .find(|r| r.indicator == Indicator::HeartRate)
                    .unwrap()
                    .value(),
                json!({"bpm": 72})
            );
            assert!(
                rules
                    .records
                    .iter()
                    .filter(|r| r.indicator == Indicator::Glucose)
                    .all(|r| r.value() == json!({}))
            );
        }

        // Corrección sin etiqueta: se aplica a la última mención (glucosa), que
        // queda pendiente junto con la alternativa explícita; el pulso anterior
        // no se corrige y conserva 72.
        let input = "pulso 72 y glucosa 95 mg/dL, perdón 82 o glucosa 105 mg/dL";
        assert!(pulse72.check(input).is_empty());
        for p in [
            glucose95.clone(),
            slm_numeric(Indicator::HeartRate, "bpm", json!(82), "perdón 82", "82"),
            slm_numeric(
                Indicator::Glucose,
                "mg_dl",
                json!(105),
                "glucosa 105 mg/dL",
                "105 mg/dL",
            ),
        ] {
            let key = &p.records[0].fields[0].key;
            assert!(
                p.check(input).iter().any(|i| i.error.field == *key),
                "{input:?}"
            );
        }
        let rules = crate::interpret::interpret(input);
        assert!(rules.check(input).is_empty());
        assert_eq!(rules.records.len(), 2);
        for r in &rules.records {
            let expected = match r.indicator {
                Indicator::HeartRate => json!({"bpm": 72}),
                _ => json!({}),
            };
            assert_eq!(r.value(), expected);
        }

        // Alternativas de indicadores previas a una corrección implícita.
        let input = "pulso 72 o glucosa 95 mg/dL, perdón 82";
        for p in [&pulse72, &glucose95] {
            let key = &p.records[0].fields[0].key;
            assert!(p.check(input).iter().any(|i| i.error.field == *key));
        }
        let rules = crate::interpret::interpret(input);
        assert!(rules.check(input).is_empty());
        assert_eq!(rules.records.len(), 2);
        assert!(rules.records.iter().all(|r| r.value() == json!({})));

        // Las alternativas previas tampoco se resuelven por el último número encontrado.
        let input = "radiación 0.42 o 0.50 uSv/h y pulso 72, perdón radiación 0.84 uSv/h";
        let ambiguous42 = slm_numeric(
            Indicator::SpaceRadiation,
            "usv_h",
            json!(0.42),
            "radiación 0.42",
            "0.42",
        );
        let ambiguous50 = slm_numeric(
            Indicator::SpaceRadiation,
            "usv_h",
            json!(0.50),
            "radiación 0.42 o 0.50 uSv/h",
            "0.50 uSv/h",
        );
        for p in [&ambiguous42, &ambiguous50, &rad84] {
            assert!(p.check(input).iter().any(|i| i.error.field == "usv_h"));
        }
        assert!(pulse72.check(input).is_empty());
    }

    fn revision6_symptom(quote: &str, zone: &str, zone_quote: &str, score: u8) -> Proposal {
        let score_quote = score.to_string();
        let mut proposal = slm_numeric(
            Indicator::Symptom,
            "score",
            json!(score),
            quote,
            &score_quote,
        );
        for field in &mut proposal.records[0].fields {
            match field.key.as_str() {
                "body_zone" => {
                    field.value = Some(json!(zone));
                    field.quote = Some(zone_quote.into());
                }
                "description" => {
                    field.value = Some(json!(quote));
                    field.quote = Some(quote.into());
                }
                _ => {}
            }
        }
        proposal
    }

    #[test]
    fn revision6_symptom_corrections_follow_zone_in_both_languages_and_orders() {
        // Cada fila declara literalmente la mención corregida y la que conserva su valor.
        for (
            input,
            old,
            new,
            kept,
            zone,
            zone_quote,
            old_score,
            kept_zone,
            kept_zone_quote,
            kept_score,
        ) in [
            (
                "dolor de cabeza 6 y dolor de espalda 4, perdón dolor de cabeza 7",
                "dolor de cabeza 6",
                "dolor de cabeza 7",
                "dolor de espalda 4",
                "head",
                "cabeza",
                6,
                "back",
                "espalda",
                4,
            ),
            (
                "dolor de espalda 4 y dolor de cabeza 6, perdón dolor de cabeza 7",
                "dolor de cabeza 6",
                "dolor de cabeza 7",
                "dolor de espalda 4",
                "head",
                "cabeza",
                6,
                "back",
                "espalda",
                4,
            ),
            (
                "dolor de espalda 4 y dolor de cabeza 6, perdón dolor de espalda 7",
                "dolor de espalda 4",
                "dolor de espalda 7",
                "dolor de cabeza 6",
                "back",
                "espalda",
                4,
                "head",
                "cabeza",
                6,
            ),
            (
                "dolor de cabeza 6 y dolor de espalda 4, perdón dolor de espalda 7",
                "dolor de espalda 4",
                "dolor de espalda 7",
                "dolor de cabeza 6",
                "back",
                "espalda",
                4,
                "head",
                "cabeza",
                6,
            ),
            (
                "dolor de pie izquierdo 6 y dolor de pie derecho 4, perdón dolor de pie izquierdo 7",
                "dolor de pie izquierdo 6",
                "dolor de pie izquierdo 7",
                "dolor de pie derecho 4",
                "left_foot",
                "pie izquierdo",
                6,
                "right_foot",
                "pie derecho",
                4,
            ),
            (
                "right foot pain 4 and left foot pain 6, sorry left foot pain 7",
                "left foot pain 6",
                "left foot pain 7",
                "right foot pain 4",
                "left_foot",
                "left foot",
                6,
                "right_foot",
                "right foot",
                4,
            ),
            (
                "headache 6 and back pain 4, sorry headache 7",
                "headache 6",
                "headache 7",
                "back pain 4",
                "head",
                "headache",
                6,
                "back",
                "back",
                4,
            ),
            (
                "back pain 4 and headache 6, sorry headache 7",
                "headache 6",
                "headache 7",
                "back pain 4",
                "head",
                "headache",
                6,
                "back",
                "back",
                4,
            ),
            (
                "back pain 4 and headache 6, I mean back pain 7",
                "back pain 4",
                "back pain 7",
                "headache 6",
                "back",
                "back",
                4,
                "head",
                "headache",
                6,
            ),
            (
                "headache 6 and back pain 4, I mean back pain 7",
                "back pain 4",
                "back pain 7",
                "headache 6",
                "back",
                "back",
                4,
                "head",
                "headache",
                6,
            ),
        ] {
            for (quote, score) in [(old, old_score), (new, 7)] {
                let proposal = revision6_symptom(quote, zone, zone_quote, score);
                assert!(proposal.records[0].is_complete());
                assert!(proposal.check(quote).is_empty(), "{quote:?}");
                assert!(
                    proposal
                        .check(input)
                        .iter()
                        .any(|i| i.error.field == "score"),
                    "{input:?}: {quote:?}"
                );
            }
            let unaffected = revision6_symptom(kept, kept_zone, kept_zone_quote, kept_score);
            assert!(unaffected.check(input).is_empty(), "{input:?}");
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}");
            assert_eq!(rules.records.len(), 2, "{input:?}");
            for record in &rules.records {
                if record.quote == kept {
                    assert_eq!(
                        record.value(),
                        json!({
                            "body_zone": kept_zone, "score": kept_score, "description": kept
                        })
                    );
                    assert!(record.is_complete());
                } else {
                    assert_eq!(record.quote, old);
                    assert_eq!(record.value(), json!({"description": old}));
                    assert!(!record.is_complete());
                }
            }
            assert!(rules.records.iter().any(|r| r.quote == kept));
            assert!(rules.uninterpreted.iter().any(|q| q.contains(new)));
        }
    }

    #[test]
    fn revision6_symptom_zone_aliases_and_enumerated_symptoms() {
        for (input, old_quote, new_quote, old_zone_quote, new_zone_quote, kept) in [
            (
                "cefalea 6 y dolor de espalda 4, mejor dicho migraña 7",
                "cefalea 6",
                "migraña 7",
                "cefalea",
                "migraña",
                "dolor de espalda 4",
            ),
            (
                "back pain 4 and headache 6, sorry head pain 7",
                "headache 6",
                "head pain 7",
                "headache",
                "head",
                "back pain 4",
            ),
        ] {
            for (quote, zone_quote, score) in [
                (old_quote, old_zone_quote, 6),
                (new_quote, new_zone_quote, 7),
            ] {
                let proposal = revision6_symptom(quote, "head", zone_quote, score);
                assert!(proposal.check(quote).is_empty());
                assert!(
                    proposal
                        .check(input)
                        .iter()
                        .any(|i| i.error.field == "score")
                );
            }
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty());
            assert_eq!(rules.records.len(), 2);
            assert_eq!(
                rules
                    .records
                    .iter()
                    .find(|r| r.quote == old_quote)
                    .unwrap()
                    .value(),
                json!({"description": old_quote})
            );
            assert_eq!(
                rules
                    .records
                    .iter()
                    .find(|r| r.quote == kept)
                    .unwrap()
                    .value(),
                json!({"body_zone": "back", "score": 4, "description": kept})
            );
        }
        // Integración de ambos P1: cada identidad participa en la rectificación enumerada.
        for input in [
            "dolor de cabeza 6 y dolor de espalda 4 y pulso 72, corrijo dolor de cabeza 7 y dolor de espalda 5",
            "dolor de espalda 4 y dolor de cabeza 6 y pulso 72, corrijo dolor de espalda 5, dolor de cabeza 7",
            "headache 6 and back pain 4 and pulse 72, sorry headache 7 and back pain 5",
            "back pain 4 and headache 6 and pulse 72, I mean back pain 5, headache 7",
        ] {
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}");
            assert_eq!(rules.records.len(), 3);
            for record in &rules.records {
                if record.indicator == Indicator::HeartRate {
                    assert_eq!(record.value(), json!({"bpm": 72}));
                } else {
                    assert_eq!(record.indicator, Indicator::Symptom);
                    assert_eq!(record.value(), json!({"description": record.quote}));
                    assert!(!record.is_complete());
                }
            }
        }
    }

    #[test]
    fn revision6_unlocalized_symptom_corrections_use_description() {
        // "other" es una categoría compartida, no la identidad de náuseas y fatiga.
        for (input, old, new, kept) in [
            (
                "náuseas 6 y fatiga 4, perdón náuseas 7",
                "náuseas 6",
                "náuseas 7",
                "fatiga 4",
            ),
            (
                "fatiga 4 y náuseas 6, perdón náuseas 7",
                "náuseas 6",
                "náuseas 7",
                "fatiga 4",
            ),
            (
                "nausea 6 and fatigue 4, sorry nausea 7",
                "nausea 6",
                "nausea 7",
                "fatigue 4",
            ),
            (
                "fatigue 4 and nausea 6, sorry nausea 7",
                "nausea 6",
                "nausea 7",
                "fatigue 4",
            ),
            (
                "fatiga 6 y náuseas 4, mejor dicho fatiga 7",
                "fatiga 6",
                "fatiga 7",
                "náuseas 4",
            ),
            (
                "náuseas 4 y fatiga 6, mejor dicho fatiga 7",
                "fatiga 6",
                "fatiga 7",
                "náuseas 4",
            ),
            (
                "fatigue 6 and nausea 4, I mean fatigue 7",
                "fatigue 6",
                "fatigue 7",
                "nausea 4",
            ),
            (
                "nausea 4 and fatigue 6, I mean fatigue 7",
                "fatigue 6",
                "fatigue 7",
                "nausea 4",
            ),
        ] {
            for (quote, score) in [(old, 6), (new, 7)] {
                let proposal = slm_numeric(
                    Indicator::Symptom,
                    "score",
                    json!(score),
                    quote,
                    &score.to_string(),
                );
                assert!(proposal.check(quote).is_empty());
                assert!(
                    proposal
                        .check(input)
                        .iter()
                        .any(|i| i.error.field == "score"),
                    "{input:?}"
                );
            }
            let unaffected = slm_numeric(Indicator::Symptom, "score", json!(4), kept, "4");
            assert!(unaffected.check(input).is_empty(), "{input:?}");
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}");
            assert_eq!(rules.records.len(), 2);
            let corrected = rules.records.iter().find(|r| r.quote == old).unwrap();
            assert_eq!(corrected.value(), json!({"description": old}));
            let retained = rules.records.iter().find(|r| r.quote == kept).unwrap();
            assert_eq!(
                retained.value(),
                json!({"body_zone": "other", "score": 4, "description": kept})
            );
        }
    }

    #[test]
    fn revision6_unresolved_symptom_identity_keeps_all_candidates_pending() {
        for (input, candidates, correction) in [
            (
                "dolor de cabeza 6 y dolor de espalda 4, perdón dolor 7",
                ["dolor de cabeza 6", "dolor de espalda 4"],
                "dolor 7",
            ),
            (
                "dolor de espalda 4 y dolor de cabeza 6, perdón síntoma 7",
                ["dolor de cabeza 6", "dolor de espalda 4"],
                "síntoma 7",
            ),
            (
                "headache 6 and back pain 4, sorry pain 7",
                ["headache 6", "back pain 4"],
                "pain 7",
            ),
            (
                "back pain 4 and headache 6, I mean symptom 7",
                ["headache 6", "back pain 4"],
                "symptom 7",
            ),
            (
                "náuseas 6 y fatiga 4, perdón síntoma 7",
                ["náuseas 6", "fatiga 4"],
                "síntoma 7",
            ),
            (
                "fatigue 4 and nausea 6, sorry symptom 7",
                ["nausea 6", "fatigue 4"],
                "symptom 7",
            ),
        ] {
            for (quote, score) in [(candidates[0], 6), (candidates[1], 4), (correction, 7)] {
                let proposal = slm_numeric(
                    Indicator::Symptom,
                    "score",
                    json!(score),
                    quote,
                    &score.to_string(),
                );
                assert!(
                    proposal
                        .check(input)
                        .iter()
                        .any(|i| i.error.field == "score"),
                    "{input:?}"
                );
            }
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}");
            assert_eq!(rules.records.len(), 2);
            for record in &rules.records {
                assert!(candidates.contains(&record.quote.as_str()));
                assert_eq!(record.value(), json!({"description": record.quote}));
                assert!(!record.is_complete());
            }
        }
    }

    #[test]
    fn revision6_enumerated_corrections_keep_every_named_indicator_pending() {
        // Los esperados son constantes: pulso y glucosa pendientes; temperatura previa = 37.
        // Ambos órdenes iniciales y rectificados se combinan independientemente.
        for (pulse, glucose, temperature, new_pulse, new_glucose) in [
            (
                "pulso 72",
                "glucosa 95 mg/dL",
                "temperatura 37",
                "pulso 82",
                "glucosa 105 mg/dL",
            ),
            (
                "pulse 72",
                "glucose 95 mg/dL",
                "body temperature 37",
                "pulse 82",
                "glucose 105 mg/dL",
            ),
        ] {
            for separator in [" y ", " and ", ", "] {
                for marker in [
                    "corrijo",
                    "perdón",
                    "sorry",
                    "I mean",
                    "mejor dicho",
                    "rectifico",
                    "perdón,",
                ] {
                    for (first, second) in [(pulse, glucose), (glucose, pulse)] {
                        for (new_first, new_second) in
                            [(new_pulse, new_glucose), (new_glucose, new_pulse)]
                        {
                            for with_temperature in [false, true] {
                                let initial = format!("{first}{separator}{second}");
                                let initial = if with_temperature {
                                    format!("{initial}{separator}{temperature}")
                                } else {
                                    initial
                                };
                                let input = format!(
                                    "{initial}, {marker} {new_first}{separator}{new_second}"
                                );
                                for (indicator, key, value, quote, field_quote) in [
                                    (Indicator::HeartRate, "bpm", 72, pulse, "72"),
                                    (Indicator::HeartRate, "bpm", 82, new_pulse, "82"),
                                    (Indicator::Glucose, "mg_dl", 95, glucose, "95 mg/dL"),
                                    (Indicator::Glucose, "mg_dl", 105, new_glucose, "105 mg/dL"),
                                ] {
                                    let proposal = slm_numeric(
                                        indicator,
                                        key,
                                        json!(value),
                                        quote,
                                        field_quote,
                                    );
                                    assert!(proposal.check(quote).is_empty(), "{quote:?}");
                                    assert!(
                                        proposal.check(&input).iter().any(|i| i.error.field == key),
                                        "{input:?}: {quote:?}"
                                    );
                                }
                                let rules = crate::interpret::interpret(&input);
                                assert!(rules.check(&input).is_empty(), "{input:?}");
                                assert_eq!(
                                    rules.records.len(),
                                    if with_temperature { 3 } else { 2 }
                                );
                                for indicator in [Indicator::HeartRate, Indicator::Glucose] {
                                    let records: Vec<_> = rules
                                        .records
                                        .iter()
                                        .filter(|r| r.indicator == indicator)
                                        .collect();
                                    assert_eq!(records.len(), 1, "{input:?}");
                                    assert_eq!(records[0].value(), json!({}), "{input:?}");
                                    assert!(!records[0].is_complete());
                                }
                                if with_temperature {
                                    let unaffected = slm_numeric(
                                        Indicator::BodyTemperature,
                                        "celsius",
                                        json!(37),
                                        temperature,
                                        "37",
                                    );
                                    assert!(unaffected.check(&input).is_empty(), "{input:?}");
                                    let record = rules
                                        .records
                                        .iter()
                                        .find(|r| r.indicator == Indicator::BodyTemperature)
                                        .unwrap();
                                    assert_eq!(record.value(), json!({"celsius": 37}));
                                    assert!(record.is_complete());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn revision6_new_measurements_do_not_end_the_correction_scope() {
        for input in [
            "pulso 72 y glucosa 95 mg/dL, corrijo pulso 82 y temperatura 37 y glucosa 105 mg/dL",
            "glucosa 95 mg/dL y pulso 72, perdón glucosa 105 mg/dL, temperatura 37, pulso 82",
            "pulse 72 and glucose 95 mg/dL, sorry pulse 82 and temperatura 37 and glucose 105 mg/dL",
            "glucose 95 mg/dL and pulse 72, I mean glucose 105 mg/dL, temperatura 37, pulse 82",
        ] {
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}");
            assert_eq!(rules.records.len(), 3);
            for indicator in [Indicator::HeartRate, Indicator::Glucose] {
                let record = rules
                    .records
                    .iter()
                    .find(|r| r.indicator == indicator)
                    .unwrap();
                assert_eq!(record.value(), json!({}), "{input:?}");
            }
            let temperature = rules
                .records
                .iter()
                .find(|r| r.indicator == Indicator::BodyTemperature)
                .unwrap();
            assert_eq!(temperature.value(), json!({"celsius": 37}));
            let proposal = slm_numeric(
                Indicator::BodyTemperature,
                "celsius",
                json!(37),
                "temperatura 37",
                "37",
            );
            assert!(proposal.check(input).is_empty());
        }
    }

    #[test]
    fn revision6_correction_enumeration_stops_at_clause_changes() {
        for boundary in [". ", "; ", " pero ", " but ", " luego ", " then "] {
            let input = format!(
                "pulso 72 y glucosa 95 mg/dL, corrijo pulso 82 y glucosa 105 mg/dL{boundary}temperatura 37"
            );
            let rules = crate::interpret::interpret(&input);
            assert!(rules.check(&input).is_empty(), "{input:?}");
            assert_eq!(rules.records.len(), 3);
            for record in &rules.records {
                let expected = if record.indicator == Indicator::BodyTemperature {
                    json!({"celsius": 37})
                } else {
                    json!({})
                };
                assert_eq!(record.value(), expected, "{input:?}");
            }
            let temperature = slm_numeric(
                Indicator::BodyTemperature,
                "celsius",
                json!(37),
                "temperatura 37",
                "37",
            );
            assert!(temperature.check(&input).is_empty());
        }
    }

    #[test]
    fn revision5_score_aliases_do_not_authorize_trimmed_temperature_or_pressure() {
        for (input, temperature_quote) in [
            ("dolor de cabeza grado 6", "grado 6"),
            ("dolor de cabeza grados 6", "grados 6"),
            ("headache degree 6", "degree 6"),
            ("headache degrees 6", "degrees 6"),
            ("ánimo grado 6", "grado 6"),
        ] {
            let p = slm_numeric(
                Indicator::BodyTemperature,
                "celsius",
                json!(6),
                temperature_quote,
                "6",
            );
            assert!(!p.check(input).is_empty(), "{input:?}");
        }
        for input in [
            "ánimo bajo presión nivel 6",
            "mood under pressure level 6",
            "dolor de cabeza presión 6",
        ] {
            let p = slm_numeric(Indicator::BloodPressure, "systolic", json!(6), input, "6");
            assert!(
                p.check(input).iter().any(|i| i.error.field == "systolic"),
                "{input:?}"
            );
        }
    }

    #[test]
    fn revision4_slm_values_survive_all_context_combinations_in_both_orders() {
        let proposals = [
            slm_numeric(Indicator::HeartRate, "bpm", json!(72), "pulso 72", "72"),
            slm_numeric(
                Indicator::SpaceRadiation,
                "usv_h",
                json!(0.42),
                "radiación 0.42 uSv/h",
                "0.42 uSv/h",
            ),
            slm_numeric(
                Indicator::DistanceFromEarth,
                "km",
                json!(420),
                "distancia 420 km",
                "420 km",
            ),
            slm_numeric(
                Indicator::Nutrition,
                "kcal",
                json!(2000),
                "nutrición 2000 kcal",
                "2000 kcal",
            ),
        ];
        let inputs = [
            "pulso 72 y radiación 0.42 uSv/h",
            "radiación 0.42 uSv/h y pulso 72",
            "pulso 72 y distancia 420 km",
            "distancia 420 km y pulso 72",
            "pulso 72 y nutrición 2000 kcal",
            "nutrición 2000 kcal y pulso 72",
            "radiación 0.42 uSv/h y distancia 420 km",
            "distancia 420 km y radiación 0.42 uSv/h",
            "radiación 0.42 uSv/h y nutrición 2000 kcal",
            "nutrición 2000 kcal y radiación 0.42 uSv/h",
            "distancia 420 km y nutrición 2000 kcal",
            "nutrición 2000 kcal y distancia 420 km",
            "radiación 0.42 uSv/h y distancia 420 km y nutrición 2000 kcal",
            "radiación 0.42 uSv/h y nutrición 2000 kcal y distancia 420 km",
            "distancia 420 km y radiación 0.42 uSv/h y nutrición 2000 kcal",
            "distancia 420 km y nutrición 2000 kcal y radiación 0.42 uSv/h",
            "nutrición 2000 kcal y radiación 0.42 uSv/h y distancia 420 km",
            "nutrición 2000 kcal y distancia 420 km y radiación 0.42 uSv/h",
            "pulso 72 y radiación 0.42 uSv/h y distancia 420 km y nutrición 2000 kcal",
            "nutrición 2000 kcal y distancia 420 km y radiación 0.42 uSv/h y pulso 72",
            // También se necesitan límites locales sin una conjunción separadora.
            "pulso 72 radiación 0.42 uSv/h",
            "radiación 0.42 uSv/h pulso 72",
            "pulso 72 distancia 420 km",
            "distancia 420 km pulso 72",
            "pulso 72 nutrición 2000 kcal",
            "nutrición 2000 kcal pulso 72",
            "radiación 0.42 uSv/h distancia 420 km nutrición 2000 kcal",
            "nutrición 2000 kcal distancia 420 km radiación 0.42 uSv/h",
        ];
        for input in inputs {
            let mut combined = proposals[0].clone();
            combined.records.clear();
            for proposal in &proposals {
                let record = &proposal.records[0];
                if input.contains(&record.quote) {
                    let issues = proposal.check(input);
                    assert!(issues.is_empty(), "{input:?}: {issues:?}");
                    combined.records.push(record.clone());
                }
            }
            assert!(combined.records.len() >= 2);
            assert!(combined.check(input).is_empty(), "{input:?}");
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}: {rules:?}");
            assert!(
                rules
                    .records
                    .iter()
                    .all(|r| r.indicator == Indicator::HeartRate)
            );
            if input.contains("pulso 72") {
                assert_eq!(rules.records.len(), 1, "{input:?}");
                assert_eq!(rules.records[0].value(), json!({"bpm": 72}), "{input:?}");
            } else {
                assert!(rules.records.is_empty(), "{input:?}");
            }
        }
    }

    #[test]
    fn revision4_context_mentions_keep_attribution_negation_units_and_times() {
        let cases = [
            (
                Indicator::SpaceRadiation,
                "usv_h",
                json!(0.42),
                "radiación 0.42",
                "0.42",
                [
                    "radiación 0.42 uSv/h/kg",
                    "no medí radiación 0.42 uSv/h y pulso 72",
                    "pulso 72 y no medí radiación 0.42 uSv/h",
                    "radiación desconocida tras 0.42 uSv/h",
                    "radiación 0.42 o 0.84 uSv/h",
                ],
            ),
            (
                Indicator::DistanceFromEarth,
                "km",
                json!(420),
                "distancia 420",
                "420",
                [
                    "distancia 420 km/h",
                    "no medí distancia 420 km y pulso 72",
                    "pulso 72 y no medí distancia 420 km",
                    "distancia desconocida tras 420 km",
                    "distancia 420 o 840 km",
                ],
            ),
            (
                Indicator::Nutrition,
                "kcal",
                json!(2000),
                "nutrición 2000",
                "2000",
                [
                    "nutrición 2000 kcal/kg",
                    "no medí nutrición 2000 kcal y pulso 72",
                    "pulso 72 y no medí nutrición 2000 kcal",
                    "nutrición desconocida tras 2000 kcal",
                    "nutrición 2000 o 2200 kcal",
                ],
            ),
        ];
        for (indicator, key, value, short_record_quote, short_field_quote, inputs) in cases {
            for input in inputs {
                // Para los predicados ajenos la cita debe incluirlos para llegar al número.
                let record_quote = if input.contains(short_record_quote) {
                    short_record_quote
                } else {
                    input
                };
                let proposal = slm_numeric(
                    indicator,
                    key,
                    value.clone(),
                    record_quote,
                    short_field_quote,
                );
                assert!(
                    proposal
                        .check(input)
                        .iter()
                        .any(|issue| issue.error.field == key),
                    "{input:?}"
                );
                assert!(
                    crate::interpret::interpret(input).check(input).is_empty(),
                    "{input:?}"
                );
                if input.contains("pulso 72") {
                    assert!(
                        pulse("pulso 72", json!(72), Some("72"))
                            .check(input)
                            .is_empty()
                    );
                }
            }
        }
        for (indicator, key, record_quote) in [
            (Indicator::SpaceRadiation, "usv_h", "radiación a las 08:"),
            (Indicator::DistanceFromEarth, "km", "distancia a las 08:"),
            (Indicator::Nutrition, "kcal", "nutrición a las 08:"),
        ] {
            let proposal = slm_numeric(indicator, key, json!(8), record_quote, "08");
            assert!(
                proposal
                    .check(record_quote)
                    .iter()
                    .any(|issue| issue.error.field == key)
            );
        }
    }

    #[test]
    fn revision4_context_corrections_do_not_clear_neighboring_pulse() {
        let corrected_proposals = [
            slm_numeric(
                Indicator::SpaceRadiation,
                "usv_h",
                json!(0.42),
                "radiación 0.42 uSv/h",
                "0.42 uSv/h",
            ),
            slm_numeric(
                Indicator::SpaceRadiation,
                "usv_h",
                json!(0.84),
                "radiación 0.84 uSv/h",
                "0.84 uSv/h",
            ),
            slm_numeric(
                Indicator::DistanceFromEarth,
                "km",
                json!(420),
                "distancia 420 km",
                "420 km",
            ),
            slm_numeric(
                Indicator::DistanceFromEarth,
                "km",
                json!(840),
                "distancia 840 km",
                "840 km",
            ),
            slm_numeric(
                Indicator::Nutrition,
                "kcal",
                json!(2000),
                "nutrición 2000 kcal",
                "2000 kcal",
            ),
            slm_numeric(
                Indicator::Nutrition,
                "kcal",
                json!(2200),
                "nutrición 2200 kcal",
                "2200 kcal",
            ),
        ];
        for input in [
            "pulso 72 y radiación 0.42 uSv/h, perdón radiación 0.84 uSv/h",
            "radiación 0.42 uSv/h, perdón radiación 0.84 uSv/h y pulso 72",
            "pulso 72 y distancia 420 km, perdón distancia 840 km",
            "distancia 420 km, perdón distancia 840 km y pulso 72",
            "pulso 72 y nutrición 2000 kcal, perdón nutrición 2200 kcal",
            "nutrición 2000 kcal, perdón nutrición 2200 kcal y pulso 72",
            "pulso 72 radiación 0.42 uSv/h, perdón radiación 0.84 uSv/h",
            "radiación 0.42 uSv/h, perdón radiación 0.84 uSv/h pulso 72",
            "pulso 72 distancia 420 km, perdón distancia 840 km",
            "distancia 420 km, perdón distancia 840 km pulso 72",
            "pulso 72 nutrición 2000 kcal, perdón nutrición 2200 kcal",
            "nutrición 2000 kcal, perdón nutrición 2200 kcal pulso 72",
        ] {
            assert!(
                pulse("pulso 72", json!(72), Some("72"))
                    .check(input)
                    .is_empty(),
                "{input:?}"
            );
            for proposal in &corrected_proposals {
                let record = &proposal.records[0];
                if input.contains(&record.quote) {
                    let key = &record.fields[0].key;
                    assert!(
                        proposal
                            .check(input)
                            .iter()
                            .any(|issue| &issue.error.field == key),
                        "{input:?}: {proposal:?}"
                    );
                }
            }
            let rules = crate::interpret::interpret(input);
            assert!(rules.check(input).is_empty(), "{input:?}: {rules:?}");
            assert_eq!(rules.records.len(), 1, "{input:?}");
            assert_eq!(rules.records[0].value(), json!({"bpm": 72}), "{input:?}");
        }
    }

    #[test]
    fn revision4_numbers_remain_attributed_to_their_own_indicator() {
        for input in [
            "pulso 72 y radiación 0.42 uSv/h",
            "radiación 0.42 uSv/h y pulso 72",
            "pulso 72 y distancia 420 km",
            "distancia 420 km y pulso 72",
            "pulso 72 y nutrición 2000 kcal",
            "nutrición 2000 kcal y pulso 72",
            "radiación 0.42 uSv/h y distancia 420 km y nutrición 2000 kcal",
            "nutrición 2000 kcal y distancia 420 km y radiación 0.42 uSv/h",
        ] {
            for (indicator, key, value, field_quote) in [
                (Indicator::SpaceRadiation, "usv_h", json!(72), "72"),
                (Indicator::DistanceFromEarth, "km", json!(72), "72"),
                (Indicator::Nutrition, "kcal", json!(72), "72"),
                (Indicator::SpaceRadiation, "usv_h", json!(420), "420"),
                (Indicator::Nutrition, "kcal", json!(420), "420"),
                (Indicator::DistanceFromEarth, "km", json!(2000), "2000"),
            ] {
                if input.contains(field_quote) && mentions_indicator(indicator, input) {
                    let proposal = slm_numeric(indicator, key, value, input, field_quote);
                    assert!(
                        proposal
                            .check(input)
                            .iter()
                            .any(|issue| issue.error.field == key),
                        "{input:?}: {proposal:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn revision4_context_aliases_and_unit_only_mentions_have_local_segments() {
        for (indicator, key, value, record_quote, field_quote, inputs) in [
            (
                Indicator::SpaceRadiation,
                "usv_h",
                json!(0.42),
                "radiación espacial 0.42 uSv/h",
                "0.42 uSv/h",
                [
                    "pulso 72 y radiación espacial 0.42 uSv/h",
                    "radiación espacial 0.42 uSv/h y pulso 72",
                ],
            ),
            (
                Indicator::DistanceFromEarth,
                "km",
                json!(420),
                "distancia a la Tierra 420 km",
                "420 km",
                [
                    "pulso 72 y distancia a la Tierra 420 km",
                    "distancia a la Tierra 420 km y pulso 72",
                ],
            ),
            (
                Indicator::Nutrition,
                "kcal",
                json!(2000),
                "comida 2000 kcal",
                "2000 kcal",
                ["pulso 72 y comida 2000 kcal", "comida 2000 kcal y pulso 72"],
            ),
            (
                Indicator::SpaceRadiation,
                "usv_h",
                json!(0.42),
                "0.42 uSv/h",
                "0.42 uSv/h",
                ["pulso 72 0.42 uSv/h", "0.42 uSv/h pulso 72"],
            ),
            (
                Indicator::DistanceFromEarth,
                "km",
                json!(420),
                "420 km",
                "420 km",
                ["pulso 72 420 km", "420 km pulso 72"],
            ),
            (
                Indicator::Nutrition,
                "kcal",
                json!(2000),
                "2000 kcal",
                "2000 kcal",
                ["pulso 72 2000 kcal", "2000 kcal pulso 72"],
            ),
        ] {
            let proposal = slm_numeric(indicator, key, value, record_quote, field_quote);
            assert!(proposal.check(record_quote).is_empty(), "{record_quote:?}");
            for input in inputs {
                assert!(proposal.check(input).is_empty(), "{input:?}");
                assert!(
                    pulse("pulso 72", json!(72), Some("72"))
                        .check(input)
                        .is_empty(),
                    "{input:?}"
                );
                let rules = crate::interpret::interpret(input);
                assert!(rules.check(input).is_empty(), "{input:?}: {rules:?}");
                assert_eq!(rules.records.len(), 1, "{input:?}");
                assert_eq!(rules.records[0].value(), json!({"bpm": 72}), "{input:?}");
            }
        }
    }
}
