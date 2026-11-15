//! Explicador de evaluaciones de `merken-core` con validación de fidelidad y seguridad (RF-15).

use crate::{
    control::{Control, Progress},
    engine::{DEFAULT_MAX_TOKENS, DEFAULT_SEED, DEFAULT_TEMPERATURE, Engine, GenerateRequest},
    error::{OutputIssue, SlmError},
};
use merken_core::evaluate::{
    Direction, Escalation, Evaluation, NotEvaluatedReason, SourceKind, Status,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Etiqueta obligatoria que identifica toda salida explicativa del SLM (RF-15).
pub const AI_SUGGESTION_LABEL: &str = "AI-generated suggestion";

/// Límite máximo de caracteres permitido para una explicación.
pub const MAX_EXPLANATION_CHARS: usize = 900;

/// Ficha de contramedida o protocolo curada proporcionada al explicador (RF-31).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub text: String,
}

/// Explicación validada generada a partir de una [`Evaluation`] ya decidida por `merken-core`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Explanation {
    pub text: String,
    pub label: String,
    pub model_id: String,
    pub cited_docs: Vec<String>,
}

/// Trait para explicadores de evaluaciones deterministas (RF-15).
pub trait Explainer {
    /// Explica una evaluación usando únicamente los hechos de `eval` y las fichas `docs`.
    fn explain(
        &mut self,
        eval: &Evaluation,
        docs: &[Document],
        ctl: &Control<'_>,
    ) -> Result<Explanation, SlmError>;
}

const EXPLAINER_SYSTEM_PROMPT: &str = "\
You explain a pre-computed astronaut health evaluation in plain English (RNF-14).
Rules:
1. Write 1 to 5 concise sentences in English only.
2. State only facts present in the evaluation and the provided documents. Never invent numbers, percentages, or thresholds.
3. Cite provided documents using [doc_id]. Never invent citations.
4. Never contradict the evaluation status or escalation.
5. Never diagnose, recommend drugs or dosages, mention a \"normal range\", or claim an \"approved protocol\" unless source_kind is \"approved_protocol\".";

/// Explicador respaldado por un [`Engine`] con validación posterior estricta.
pub struct SlmExplainer<E: Engine> {
    engine: E,
    model_id: String,
    max_tokens: u32,
}

impl<E: Engine> SlmExplainer<E> {
    /// Crea un explicador indicando explícitamente el `model_id` verificado.
    pub fn new(engine: E, model_id: impl Into<String>) -> Self {
        Self {
            engine,
            model_id: model_id.into(),
            max_tokens: DEFAULT_MAX_TOKENS,
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

    fn build_request(&self, eval: &Evaluation, docs: &[Document]) -> GenerateRequest {
        let prompt = format_explainer_prompt(eval, docs);
        GenerateRequest {
            system: EXPLAINER_SYSTEM_PROMPT.to_owned(),
            prompt,
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
        ctl: &Control<'_>,
    ) -> Result<Explanation, SlmError> {
        ctl.check()?;
        let req = self.build_request(eval, docs);
        let raw = self.engine.generate(&req, ctl)?;
        ctl.check()?;
        ctl.report(Progress::Validating);

        let text = raw.trim().to_owned();
        let issues = validate_explanation(&text, eval, docs);
        if !issues.is_empty() {
            return Err(SlmError::InvalidOutput(issues));
        }

        let cited_docs = extract_citations(&text)
            .into_iter()
            .filter(|id| docs.iter().any(|d| d.id == *id))
            .fold(Vec::new(), |mut acc, id| {
                if !acc.contains(&id) {
                    acc.push(id);
                }
                acc
            });

        ctl.report(Progress::Done);
        Ok(Explanation {
            text,
            label: AI_SUGGESTION_LABEL.to_owned(),
            model_id: self.model_id.clone(),
            cited_docs,
        })
    }
}

fn format_number(n: f64) -> String {
    if !n.is_finite() {
        return "none".to_owned();
    }
    if (n - n.round()).abs() < 1e-9 {
        return format!("{:.0}", n.round());
    }
    let s = format!("{n:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn status_str(status: Status) -> &'static str {
    match status {
        Status::NotEvaluated => "not_evaluated",
        Status::Stable => "stable",
        Status::Change => "change",
        Status::Alarm => "alarm",
    }
}

fn reason_str(reason: NotEvaluatedReason) -> &'static str {
    match reason {
        NotEvaluatedReason::NoData => "no_data",
        NotEvaluatedReason::TooFewData => "too_few_data",
        NotEvaluatedReason::Stale => "stale",
        NotEvaluatedReason::NotApplicable => "not_applicable",
    }
}

fn direction_str(direction: Direction) -> &'static str {
    match direction {
        Direction::Up => "up",
        Direction::Down => "down",
    }
}

fn escalation_str(escalation: Escalation) -> &'static str {
    match escalation {
        Escalation::Continue => "continue",
        Escalation::SelfCare => "self_care",
        Escalation::FlagForGround => "flag_for_ground",
        Escalation::Urgent => "urgent",
    }
}

fn source_kind_str(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Published => "published",
        SourceKind::ApprovedProtocol => "approved_protocol",
        SourceKind::DemoHeuristic => "demo_heuristic",
    }
}

fn format_explainer_prompt(eval: &Evaluation, docs: &[Document]) -> String {
    let mut out = String::from("Core evaluation facts:\n");
    out.push_str(&format!("- indicator: {}\n", eval.indicator));
    if let Some(zone) = eval.body_zone {
        out.push_str(&format!("- body_zone: {}\n", zone.as_str()));
    }
    out.push_str(&format!("- status: {}\n", status_str(eval.status)));
    if let Some(reason) = eval.reason {
        out.push_str(&format!("- reason: {}\n", reason_str(reason)));
    }
    if let Some(direction) = eval.direction {
        out.push_str(&format!("- direction: {}\n", direction_str(direction)));
    }
    out.push_str(&format!("- persistent: {}\n", eval.persistent));
    out.push_str(&format!(
        "- escalation: {}\n",
        escalation_str(eval.escalation)
    ));
    if let Some(val) = &eval.latest_value {
        out.push_str(&format!("- latest_value: {val}\n"));
    }
    if let Some(baseline) = eval.baseline {
        out.push_str(&format!("- baseline: {}\n", format_number(baseline)));
    }
    out.push_str(&format!("- history_count: {}\n", eval.history_count));
    out.push_str(&format!("- history_needed: {}\n", eval.history_needed));
    out.push_str(&format!(
        "- source_kind: {}\n",
        source_kind_str(eval.source_kind)
    ));
    if let Some(source_ref) = &eval.source_ref {
        out.push_str(&format!("- source_ref: {source_ref}\n"));
    }
    out.push_str(&format!("- summary: {}\n", eval.summary));

    if docs.is_empty() {
        out.push_str("Documents: none\n");
    } else {
        out.push_str("Documents:\n");
        for doc in docs {
            out.push_str(&format!("- [{}] {}\n", doc.id, doc.text));
        }
    }
    out
}

/// Valida de forma pura que la explicación cumpla fidelidad, idioma y reglas de seguridad.
pub fn validate_explanation(text: &str, eval: &Evaluation, docs: &[Document]) -> Vec<OutputIssue> {
    let mut issues = Vec::new();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        issues.push(OutputIssue::new(
            "empty_output",
            "Explanation text must not be empty",
        ));
        return issues;
    }

    if trimmed.chars().count() > MAX_EXPLANATION_CHARS {
        issues.push(OutputIssue::new(
            "too_long",
            format!("Explanation exceeds maximum length of {MAX_EXPLANATION_CHARS} characters"),
        ));
    }

    let sentences = count_sentences(trimmed);
    if !(1..=5).contains(&sentences) {
        issues.push(OutputIssue::new(
            "sentence_count",
            format!("Explanation must have 1 to 5 sentences (found {sentences})"),
        ));
    }

    validate_citations(trimmed, docs, &mut issues);
    validate_numbers_and_percentages(trimmed, eval, docs, &mut issues);
    validate_status_consistency(trimmed, eval, &mut issues);
    validate_forbidden_terms(trimmed, eval, &mut issues);
    validate_english_language(trimmed, &mut issues);

    issues
}

fn count_sentences(text: &str) -> usize {
    let stripped = strip_citations(text);
    let chars: Vec<char> = stripped.chars().collect();
    let mut count = 0usize;
    let mut has_word_content = false;

    for (i, &ch) in chars.iter().enumerate() {
        let is_decimal_dot = ch == '.'
            && i > 0
            && chars.get(i - 1).is_some_and(char::is_ascii_digit)
            && chars.get(i + 1).is_some_and(char::is_ascii_digit);

        if (ch == '.' && !is_decimal_dot) || ch == '!' || ch == '?' {
            if has_word_content {
                count += 1;
                has_word_content = false;
            }
        } else if ch.is_alphanumeric() {
            has_word_content = true;
        }
    }

    if has_word_content {
        count += 1;
    }
    count
}

fn extract_citations(text: &str) -> Vec<String> {
    let mut cited = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after_open = rest.get(open + 1..).unwrap_or("");
        if let Some(close) = after_open.find(']') {
            let inner = after_open.get(..close).unwrap_or("").trim();
            cited.push(inner.to_owned());
            rest = after_open.get(close + 1..).unwrap_or("");
        } else {
            break;
        }
    }
    cited
}

fn strip_citations(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_bracket = false;
    for ch in text.chars() {
        match ch {
            '[' => {
                in_bracket = true;
                out.push(' ');
            }
            ']' if in_bracket => {
                in_bracket = false;
                out.push(' ');
            }
            _ if !in_bracket => out.push(ch),
            _ => {}
        }
    }
    out
}

fn validate_citations(text: &str, docs: &[Document], issues: &mut Vec<OutputIssue>) {
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let after_open = rest.get(open + 1..).unwrap_or("");
        let Some(close) = after_open.find(']') else {
            issues.push(OutputIssue::new(
                "malformed_citation",
                "Unclosed '[' citation bracket",
            ));
            break;
        };
        let inner = after_open.get(..close).unwrap_or("").trim();
        if inner.is_empty() || inner.contains('[') {
            issues.push(OutputIssue::new(
                "malformed_citation",
                "Citation bracket must contain a single document id",
            ));
        } else if !docs.iter().any(|d| d.id == inner) {
            issues.push(OutputIssue::new(
                "unknown_citation",
                format!("Citation '[{inner}]' does not match any provided document"),
            ));
        }
        rest = after_open.get(close + 1..).unwrap_or("");
    }
}

fn collect_json_numbers(val: &Value, out: &mut Vec<f64>) {
    match val {
        Value::Number(n) => {
            if let Some(f) = n.as_f64()
                && f.is_finite()
            {
                out.push(f);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_json_numbers(item, out);
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                collect_json_numbers(v, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::String(_) => {}
    }
}

fn extract_numbers(text: &str) -> Vec<f64> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let Some(&b) = bytes.get(i) else {
            break;
        };
        let prev = i.checked_sub(1).and_then(|p| bytes.get(p).copied());
        let is_negative_start = b == b'-'
            && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)
            && !prev.is_some_and(|p| p.is_ascii_alphanumeric());
        let is_digit_start = b.is_ascii_digit() && !prev.is_some_and(|p| p.is_ascii_alphabetic());

        if !is_negative_start && !is_digit_start {
            i += 1;
            continue;
        }

        let start = i;
        if is_negative_start {
            i += 1;
        }
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if let Some(&sep) = bytes.get(i)
            && (sep == b'.' || sep == b',')
            && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)
        {
            i += 1;
            while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }

        if let Some(raw_slice) = text.get(start..i) {
            let normalized = raw_slice.replace(',', ".");
            if let Ok(num) = normalized.parse::<f64>()
                && num.is_finite()
            {
                out.push(num);
            }
        }
    }

    out
}

fn collect_allowed_numbers(eval: &Evaluation, docs: &[Document]) -> Vec<f64> {
    let mut allowed = Vec::new();
    if let Some(val) = &eval.latest_value {
        collect_json_numbers(val, &mut allowed);
    }
    if let Some(baseline) = eval.baseline
        && baseline.is_finite()
    {
        allowed.push(baseline);
        if let Ok(rounded) = format_number(baseline).parse::<f64>() {
            allowed.push(rounded);
        }
    }
    allowed.push(eval.history_count as f64);
    allowed.push(eval.history_needed as f64);
    if let Some(source_ref) = &eval.source_ref {
        allowed.extend(extract_numbers(source_ref));
    }
    allowed.extend(extract_numbers(&eval.summary));
    for doc in docs {
        allowed.extend(extract_numbers(&doc.text));
    }
    allowed
}

fn validate_numbers_and_percentages(
    text: &str,
    eval: &Evaluation,
    docs: &[Document],
    issues: &mut Vec<OutputIssue>,
) {
    let stripped = strip_citations(text);
    let allowed = collect_allowed_numbers(eval, docs);
    for num in extract_numbers(&stripped) {
        let grounded = allowed.iter().any(|&a| (a - num).abs() <= 1e-6);
        if !grounded {
            issues.push(OutputIssue::new(
                "ungrounded_number",
                format!("Number '{num}' does not appear in input evaluation or documents"),
            ));
        }
    }

    let lower = stripped.to_ascii_lowercase();
    let has_percent_symbol = lower.contains('%') || contains_word_ci(&lower, "percent");
    if has_percent_symbol {
        let input_allows_percent = eval.indicator == "oxygen_in_blood"
            || docs.iter().any(|d| {
                let dl = d.text.to_ascii_lowercase();
                dl.contains('%') || contains_word_ci(&dl, "percent")
            })
            || eval.source_ref.as_ref().is_some_and(|s| {
                let sl = s.to_ascii_lowercase();
                sl.contains('%') || contains_word_ci(&sl, "percent")
            });
        if !input_allows_percent {
            issues.push(OutputIssue::new(
                "calculated_percentage",
                "Percentage values are not allowed unless present in input data or documents",
            ));
        }

        let relative_percent_phrases = [
            "% lower",
            "% higher",
            "% decrease",
            "% increase",
            "% drop",
            "% change",
            "percent lower",
            "percent higher",
            "percent decrease",
            "percent increase",
            "percent drop",
            "percent change",
        ];
        if relative_percent_phrases.iter().any(|p| {
            lower.contains(p) && !docs.iter().any(|d| d.text.to_ascii_lowercase().contains(p))
        }) {
            issues.push(OutputIssue::new(
                "calculated_percentage",
                "Calculated relative percentage changes are not allowed",
            ));
        }
    }
}

fn contains_word_ci(lower: &str, phrase: &str) -> bool {
    contains_phrase_excluding_suffix(lower, phrase, None)
}

fn contains_phrase_excluding_suffix(
    lower: &str,
    phrase: &str,
    excluded_suffix: Option<&str>,
) -> bool {
    for (start, _) in lower.match_indices(phrase) {
        let end = start + phrase.len();
        let left_ok = lower
            .get(..start)
            .and_then(|s| s.chars().next_back())
            .is_none_or(|c| !c.is_alphanumeric());
        let right_ok = lower
            .get(end..)
            .and_then(|s| s.chars().next())
            .is_none_or(|c| !c.is_alphanumeric());
        let suffix_excluded =
            excluded_suffix.is_some_and(|suf| lower.get(end..).unwrap_or("").starts_with(suf));
        if left_ok && right_ok && !suffix_excluded {
            return true;
        }
    }
    false
}

fn contains_mg_unit(lower: &str) -> bool {
    for (start, _) in lower.match_indices("mg") {
        let end = start + 2;
        let left_ok = lower
            .get(..start)
            .and_then(|s| s.chars().next_back())
            .is_none_or(|c| !c.is_alphabetic());
        let right_ok = lower
            .get(end..)
            .and_then(|s| s.chars().next())
            .is_none_or(|c| !c.is_alphanumeric());
        let is_glucose_unit = lower.get(end..).unwrap_or("").starts_with("/dl");
        if left_ok && right_ok && !is_glucose_unit {
            return true;
        }
    }
    false
}

fn validate_status_consistency(text: &str, eval: &Evaluation, issues: &mut Vec<OutputIssue>) {
    let lower = text.to_lowercase();

    if eval.status == Status::Alarm || eval.escalation == Escalation::Urgent {
        let forbidden_for_alarm = [
            "stable",
            "within your recent range",
            "within range",
            "everything is fine",
            "all clear",
            "no issue",
            "no issues",
            "no action needed",
            "no action required",
            "no action is needed",
            "nothing to do",
            "do nothing",
            "no need to act",
            "continue as usual",
            "continue normally",
            "postpone",
            "postponed",
            "delay action",
            "wait and see",
            "can wait",
            "estable",
            "sin cambios",
            "todo bien",
            "no hay que hacer nada",
            "posponer",
            "posponida",
            "pospuesta",
            "aplazar",
        ];
        if let Some(found) = forbidden_for_alarm
            .iter()
            .find(|p| contains_word_ci(&lower, p))
        {
            issues.push(OutputIssue::new(
                "status_contradiction",
                format!("Alarm or urgent evaluation cannot contain '{found}'"),
            ));
        }
        return;
    }

    match eval.status {
        Status::Stable => {
            let forbidden_for_stable = [
                "alarm",
                "alarms",
                "urgent",
                "urgently",
                "urgency",
                "emergency",
                "immediate protocol",
                "critical",
                "flag for ground",
                "alarma",
                "urgente",
                "emergencia",
            ];
            if let Some(found) = forbidden_for_stable
                .iter()
                .find(|p| contains_word_ci(&lower, p))
            {
                issues.push(OutputIssue::new(
                    "status_contradiction",
                    format!("Stable evaluation cannot mention '{found}'"),
                ));
            }
        }
        Status::Change => {
            let forbidden_for_change = [
                "stable",
                "within your recent range",
                "alarm",
                "alarms",
                "urgent",
                "urgently",
                "urgency",
                "emergency",
                "immediate protocol",
                "estable",
                "alarma",
                "urgente",
                "emergencia",
            ];
            if let Some(found) = forbidden_for_change
                .iter()
                .find(|p| contains_word_ci(&lower, p))
            {
                issues.push(OutputIssue::new(
                    "status_contradiction",
                    format!("Change evaluation cannot claim '{found}'"),
                ));
            }
        }
        Status::NotEvaluated => {
            let forbidden_for_not_evaluated = [
                "stable",
                "within your recent range",
                "within range",
                "above your recent range",
                "below your recent range",
                "alarm",
                "alarms",
                "urgent",
                "emergency",
                "critical",
                "healthy",
                "estable",
                "alarma",
                "urgente",
            ];
            if let Some(found) = forbidden_for_not_evaluated
                .iter()
                .find(|p| contains_word_ci(&lower, p))
            {
                issues.push(OutputIssue::new(
                    "status_contradiction",
                    format!("Not-evaluated state cannot assert status '{found}'"),
                ));
            }
        }
        Status::Alarm => {}
    }
}

fn validate_forbidden_terms(text: &str, eval: &Evaluation, issues: &mut Vec<OutputIssue>) {
    let lower = text.to_lowercase();

    let diagnosis_terms = [
        "diagnosis",
        "diagnose",
        "diagnosed",
        "diagnosing",
        "diagnostic",
        "diagnóstico",
        "diagnostico",
        "diagnosticar",
        "diagnosticado",
    ];
    if let Some(term) = diagnosis_terms.iter().find(|t| contains_word_ci(&lower, t)) {
        issues.push(OutputIssue::new(
            "forbidden_diagnosis",
            format!("Forbidden diagnostic terminology '{term}'"),
        ));
    }

    let medication_terms = [
        "dose",
        "doses",
        "dosage",
        "mcg",
        "milligram",
        "milligrams",
        "medication",
        "medications",
        "medicine",
        "medicines",
        "drug",
        "drugs",
        "pill",
        "pills",
        "tablet",
        "tablets",
        "prescription",
        "prescribe",
        "prescribed",
        "dosis",
        "miligramo",
        "miligramos",
        "medicación",
        "medicacion",
        "medicamento",
        "medicamentos",
        "fármaco",
        "farmaco",
        "fármacos",
        "farmacos",
        "pastilla",
        "receta",
        "recetar",
    ];
    if let Some(term) = medication_terms
        .iter()
        .find(|t| contains_word_ci(&lower, t))
    {
        issues.push(OutputIssue::new(
            "forbidden_medication",
            format!("Forbidden medication or dosage term '{term}'"),
        ));
    } else if contains_mg_unit(&lower) {
        issues.push(OutputIssue::new(
            "forbidden_medication",
            "Forbidden dosage unit 'mg'",
        ));
    }

    let normal_range_terms = [
        "normal range",
        "normal values",
        "normal value",
        "normal limit",
        "normal limits",
        "within normal",
        "clinical normal",
        "healthy range",
        "rango normal",
        "valores normales",
        "valor normal",
        "límites normales",
        "limites normales",
        "dentro de lo normal",
    ];
    if let Some(term) = normal_range_terms
        .iter()
        .find(|t| contains_word_ci(&lower, t))
    {
        issues.push(OutputIssue::new(
            "forbidden_normal_range",
            format!("Forbidden clinical normal range claim '{term}'"),
        ));
    }

    if eval.source_kind != SourceKind::ApprovedProtocol {
        let approval_terms = [
            "approved protocol",
            "approved local protocol",
            "local approved protocol",
            "officially approved",
            "protocolo aprobado",
            "protocolo local aprobado",
        ];
        if let Some(term) = approval_terms.iter().find(|t| contains_word_ci(&lower, t)) {
            issues.push(OutputIssue::new(
                "unverified_protocol_approval",
                format!("Cannot claim '{term}' when source_kind is not approved_protocol"),
            ));
        }
    }
}

fn validate_english_language(text: &str, issues: &mut Vec<OutputIssue>) {
    let lower = text.to_lowercase();
    let has_hard_spanish_punct = lower.contains('¿') || lower.contains('¡') || lower.contains('ñ');
    let accent_count = lower
        .chars()
        .filter(|c| matches!(c, 'á' | 'é' | 'í' | 'ó' | 'ú' | 'ü'))
        .count();

    let spanish_words = [
        "el",
        "los",
        "las",
        "una",
        "unos",
        "unas",
        "del",
        "pero",
        "porque",
        "para",
        "por",
        "con",
        "sin",
        "sobre",
        "entre",
        "desde",
        "hasta",
        "según",
        "segun",
        "está",
        "están",
        "estan",
        "tiene",
        "tienen",
        "puede",
        "pueden",
        "debe",
        "deben",
        "tus",
        "sus",
        "como",
        "cómo",
        "cuando",
        "cuándo",
        "muy",
        "más",
        "también",
        "tambien",
        "todavía",
        "todavia",
        "aquí",
        "aqui",
        "estado",
        "rango",
        "reciente",
        "atención",
        "atencion",
        "urgente",
        "alarma",
        "estable",
        "datos",
        "pulso",
        "sueño",
        "ánimo",
        "animo",
        "frecuencia",
        "cardíaca",
        "cardiaca",
        "temperatura",
        "presión",
        "presion",
        "oxígeno",
        "oxigeno",
        "glucosa",
        "síntoma",
        "sintoma",
        "acción",
        "accion",
        "posponer",
        "posponida",
        "pospuesta",
        "lectura",
        "referencia",
        "evalúa",
        "evalua",
        "evaluación",
        "evaluacion",
        "valores",
        "medición",
        "medicion",
        "solicitar",
        "repetir",
        "protocolo",
        "aprobado",
        "ninguna",
        "ningún",
        "ningun",
        "hacer",
        "nada",
        "indica",
        "muestra",
        "encuentra",
        "dentro",
        "fuera",
        "encima",
        "debajo",
        "respecto",
    ];

    let word_hits = spanish_words
        .iter()
        .filter(|w| contains_word_ci(&lower, w))
        .count();

    if has_hard_spanish_punct
        || accent_count >= 2
        || word_hits >= 2
        || (accent_count >= 1 && word_hits >= 1)
    {
        issues.push(OutputIssue::new(
            "non_english_output",
            "Explanation output must be in English (RNF-14)",
        ));
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
    use merken_core::{RULES_VERSION, VERSION, time::Timestamp};
    use serde_json::json;
    use std::time::Duration;

    fn sample_eval(status: Status, escalation: Escalation, source_kind: SourceKind) -> Evaluation {
        Evaluation {
            indicator: "heart_rate".into(),
            body_zone: None,
            status,
            reason: None,
            direction: None,
            persistent: false,
            escalation,
            rule_id: "heart_rate.personal_change".into(),
            rule_version: RULES_VERSION.into(),
            source_kind,
            source_ref: Some("AHA 2020 Guidelines (>= 150/min)".into()),
            input_record_ids: vec!["r1".into()],
            input_event_ids: vec![],
            core_version: VERSION.into(),
            evaluated_at: Timestamp::from_millis(1_000_000),
            latest_value: Some(json!({ "bpm": 72 })),
            latest_measured_at: Some(Timestamp::from_millis(900_000)),
            baseline: Some(70.0),
            history_count: 5,
            history_needed: 3,
            summary: "Within your recent range".into(),
        }
    }

    fn docs() -> Vec<Document> {
        vec![
            Document {
                id: "hr-rest-1".into(),
                text:
                    "Compare resting pulse against your personal baseline after 5 minutes of rest."
                        .into(),
            },
            Document {
                id: "cmo-alarm".into(),
                text: "Notify the onboard CMO right away and open the offline procedure.".into(),
            },
        ]
    }

    #[test]
    fn good_stable_explanation_succeeds_with_label_and_citations() {
        let eval = sample_eval(
            Status::Stable,
            Escalation::Continue,
            SourceKind::DemoHeuristic,
        );
        let engine = MockEngine::with_responses([
            "Your latest heart rate is 72 bpm, which is within your recent range near your baseline of 70 bpm. Continue your usual routine [hr-rest-1].",
        ])
        .with_model_id("space-health-v5");
        let mut explainer = SlmExplainer::with_engine(engine);
        let ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));

        let explanation = explainer.explain(&eval, &docs(), &ctl).unwrap();
        assert_eq!(explanation.label, "AI-generated suggestion");
        assert_eq!(explanation.model_id, "space-health-v5");
        assert_eq!(explanation.cited_docs, vec!["hr-rest-1".to_owned()]);
    }

    #[test]
    fn good_alarm_with_approved_protocol_allows_phrase() {
        let mut eval = sample_eval(
            Status::Alarm,
            Escalation::Urgent,
            SourceKind::ApprovedProtocol,
        );
        eval.latest_value = Some(json!({ "bpm": 158 }));
        eval.summary = "Urgent attention recommended".into();

        let text = "Your heart rate is 158 bpm and requires urgent attention under the approved protocol [cmo-alarm]. Notify the onboard CMO immediately.";
        assert!(validate_explanation(text, &eval, &docs()).is_empty());
    }

    #[test]
    fn good_not_evaluated_with_counts_passes() {
        let mut eval = sample_eval(
            Status::NotEvaluated,
            Escalation::Continue,
            SourceKind::DemoHeuristic,
        );
        eval.reason = Some(NotEvaluatedReason::TooFewData);
        eval.history_count = 1;
        eval.history_needed = 3;
        eval.baseline = None;
        eval.summary = "Not enough recent history".into();

        let text = "Heart rate could not be evaluated yet because only 1 reading is available and 3 are required.";
        assert!(validate_explanation(text, &eval, &docs()).is_empty());
    }

    #[test]
    fn rejects_sol_p0_approved_protocol_without_source_kind() {
        let eval = sample_eval(Status::Alarm, Escalation::Urgent, SourceKind::DemoHeuristic);
        let text = "Your heart rate is 72 bpm. Follow the approved local protocol immediately [cmo-alarm].";
        let issues = validate_explanation(text, &eval, &docs());
        assert!(
            issues
                .iter()
                .any(|i| i.code == "unverified_protocol_approval")
        );
    }

    #[test]
    fn rejects_sol_normal_range_hallucination() {
        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let text = "Your heart rate of 72 bpm is within the normal range.";
        let issues = validate_explanation(text, &eval, &docs());
        assert!(issues.iter().any(|i| i.code == "forbidden_normal_range"));
    }

    #[test]
    fn rejects_sol_calculated_percentage_and_unknown_citation() {
        let mut eval = sample_eval(
            Status::Change,
            Escalation::SelfCare,
            SourceKind::DemoHeuristic,
        );
        eval.latest_value = Some(json!({ "bpm": 73 }));
        eval.baseline = Some(80.0);

        let text = "Your pulse shows an 8.75% decrease from baseline [missing-doc].";
        let issues = validate_explanation(text, &eval, &docs());
        assert!(issues.iter().any(|i| i.code == "ungrounded_number"));
        assert!(issues.iter().any(|i| i.code == "calculated_percentage"));
        assert!(issues.iter().any(|i| i.code == "unknown_citation"));
    }

    #[test]
    fn rejects_sol_alarm_contradiction_and_postponement() {
        let mut eval = sample_eval(Status::Alarm, Escalation::Urgent, SourceKind::Published);
        eval.latest_value = Some(json!({ "bpm": 160 }));

        let postponed = "Heart rate is 160 bpm, but the action is postponed for now.";
        let issues_postponed = validate_explanation(postponed, &eval, &docs());
        assert!(
            issues_postponed
                .iter()
                .any(|i| i.code == "status_contradiction")
        );

        let calm = "Heart rate is 160 bpm; it looks stable and no action is needed.";
        let issues_calm = validate_explanation(calm, &eval, &docs());
        assert!(issues_calm.iter().any(|i| i.code == "status_contradiction"));
    }

    #[test]
    fn rejects_alarm_language_when_stable_and_status_when_not_evaluated() {
        let stable = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let bad_stable = "Heart rate is 72 bpm, triggering an urgent alarm.";
        assert!(
            validate_explanation(bad_stable, &stable, &docs())
                .iter()
                .any(|i| i.code == "status_contradiction")
        );

        let mut not_eval = sample_eval(
            Status::NotEvaluated,
            Escalation::Continue,
            SourceKind::DemoHeuristic,
        );
        not_eval.reason = Some(NotEvaluatedReason::NoData);
        let bad_not_eval = "Heart rate is stable today.";
        assert!(
            validate_explanation(bad_not_eval, &not_eval, &docs())
                .iter()
                .any(|i| i.code == "status_contradiction")
        );
    }

    #[test]
    fn rejects_diagnosis_medication_and_dose_terms_but_allows_glucose_mg_dl() {
        let eval = sample_eval(Status::Change, Escalation::SelfCare, SourceKind::Published);
        let bad_med = "This diagnosis suggests taking a 10 mg medication dose.";
        let issues = validate_explanation(bad_med, &eval, &docs());
        assert!(issues.iter().any(|i| i.code == "forbidden_diagnosis"));
        assert!(issues.iter().any(|i| i.code == "forbidden_medication"));

        let mut glucose = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        glucose.indicator = "glucose".into();
        glucose.latest_value = Some(json!({ "mg_dl": 95 }));
        glucose.baseline = Some(92.0);
        let ok_glucose =
            "Your glucose reading of 95 mg/dL is near your recent reference of 92 mg/dL.";
        assert!(validate_explanation(ok_glucose, &glucose, &docs()).is_empty());
    }

    #[test]
    fn rejects_spanish_output_and_excessive_length() {
        let eval = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        let spanish = "Tu frecuencia cardíaca de 72 está dentro de tu rango reciente.";
        let issues_es = validate_explanation(spanish, &eval, &docs());
        assert!(issues_es.iter().any(|i| i.code == "non_english_output"));

        let six_sentences = "Pulse is 72. Baseline is 70. History has 5 readings. At least 3 are needed. Keep resting. Take care.";
        let issues_len = validate_explanation(six_sentences, &eval, &docs());
        assert!(issues_len.iter().any(|i| i.code == "sentence_count"));

        let engine = MockEngine::with_responses([spanish]);
        let mut explainer = SlmExplainer::new(engine, "v5");
        let ctl =
            Control::without_progress(CancelToken::new(), Deadline::after(Duration::from_secs(5)));
        assert!(matches!(
            explainer.explain(&eval, &docs(), &ctl),
            Err(SlmError::InvalidOutput(_))
        ));
        // Garantiza que MockEngine se consumió y no devolvió texto inválido al usuario.
        assert_eq!(explainer.engine().remaining(), 0);
    }

    #[test]
    fn oxygen_percentage_and_spo2_identifier_do_not_trigger_false_positives() {
        let mut spo2 = sample_eval(Status::Stable, Escalation::Continue, SourceKind::Published);
        spo2.indicator = "oxygen_in_blood".into();
        spo2.latest_value = Some(json!({ "percentage": 98 }));
        spo2.baseline = Some(97.0);
        let text = "Your SpO2 is 98%, close to your personal baseline of 97%.";
        assert!(validate_explanation(text, &spo2, &docs()).is_empty());
    }
}
