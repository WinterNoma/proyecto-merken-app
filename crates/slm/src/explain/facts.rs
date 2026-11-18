//! Bloque determinista de hechos clínicos renderizado por Rust desde `Evaluation` (RF-15, RNF-14).

use pulsys_core::{
    catalog::BodyZone,
    evaluate::{Direction, Escalation, Evaluation, NotEvaluatedReason, SourceKind, Status},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Tabla central de cadenas visibles en inglés (RNF-14).
mod strings {
    use pulsys_core::catalog::BodyZone;

    pub const INDICATOR_PREFIX: &str = "Indicator: ";
    pub const UNKNOWN_INDICATOR: &str = "Health indicator";
    pub const EVENT_SUFFIX: &str = " event";

    pub const INDICATOR_LABELS: &[(&str, &str)] = &[
        ("heart_rate", "Heart rate"),
        ("blood_pressure", "Blood pressure"),
        ("body_temperature", "Body temperature"),
        ("glucose", "Glucose"),
        ("sleep", "Sleep"),
        ("mood", "Mood"),
        ("oxygen_in_blood", "Blood oxygen"),
        ("space_radiation", "Space radiation"),
        ("distance_from_earth", "Distance from Earth"),
        ("nutrition", "Nutrition"),
        ("symptom", "Symptom"),
        ("need_help", "Need help event"),
        ("syncope", "Syncope event"),
        ("visual_field_loss", "Visual field loss event"),
        ("suspected_clot", "Suspected clot event"),
        ("feeling_report", "Feeling report event"),
    ];

    pub const INDICATOR_UNITS: &[(&str, &str)] = &[
        ("heart_rate", "bpm"),
        ("blood_pressure", "mmHg"),
        ("body_temperature", "°C"),
        ("glucose", "mg/dL"),
        ("sleep", "h"),
        ("mood", "/10"),
        ("oxygen_in_blood", "%"),
        ("space_radiation", "µSv/h"),
        ("distance_from_earth", "km"),
        ("nutrition", "kcal"),
        ("symptom", "/10"),
    ];

    pub const BODY_ZONE_LABELS: &[(BodyZone, &str)] = &[
        (BodyZone::Head, "head"),
        (BodyZone::Neck, "neck"),
        (BodyZone::Chest, "chest"),
        (BodyZone::Abdomen, "abdomen"),
        (BodyZone::Back, "back"),
        (BodyZone::LeftArm, "left arm"),
        (BodyZone::RightArm, "right arm"),
        (BodyZone::LeftHand, "left hand"),
        (BodyZone::RightHand, "right hand"),
        (BodyZone::LeftLeg, "left leg"),
        (BodyZone::RightLeg, "right leg"),
        (BodyZone::LeftFoot, "left foot"),
        (BodyZone::RightFoot, "right foot"),
        (BodyZone::Other, "other"),
    ];

    pub const STATUS_NOT_EVALUATED: &str = "Status: Not evaluated.";
    pub const STATUS_STABLE: &str = "Status: Stable — within your recent range.";
    pub const STATUS_CHANGE_UP: &str = "Status: Change — above your recent range.";
    pub const STATUS_CHANGE_DOWN: &str = "Status: Change — below your recent range.";
    pub const STATUS_CHANGE_GENERAL: &str = "Status: Change — outside your recent range.";
    pub const STATUS_ALARM: &str = "Status: Alarm — urgent attention recommended.";

    pub const REASON_NO_DATA: &str = "Reason: No data available.";
    pub const REASON_TOO_FEW_PREFIX: &str = "Reason: Need ";
    pub const REASON_TOO_FEW_MID: &str = " previous readings (";
    pub const REASON_TOO_FEW_SUFFIX: &str = " available).";
    pub const REASON_STALE: &str = "Reason: Latest reading is too old.";
    pub const REASON_NOT_APPLICABLE: &str =
        "Reason: Context-only indicator; health evaluation does not apply.";

    pub const DIRECTION_UP_PERSISTENT: &str =
        "Direction and persistence: upward trend, persistent.";
    pub const DIRECTION_UP_NON_PERSISTENT: &str =
        "Direction and persistence: upward trend, not persistent.";
    pub const DIRECTION_DOWN_PERSISTENT: &str =
        "Direction and persistence: downward trend, persistent.";
    pub const DIRECTION_DOWN_NON_PERSISTENT: &str =
        "Direction and persistence: downward trend, not persistent.";
    pub const DIRECTION_NONE_PERSISTENT: &str =
        "Direction and persistence: no directional shift, persistent.";
    pub const DIRECTION_NONE_NON_PERSISTENT: &str =
        "Direction and persistence: no directional shift, not persistent.";

    pub const LATEST_READING_PREFIX: &str = "Latest reading: ";
    pub const LATEST_READING_NONE: &str = "Latest reading: none.";
    pub const LATEST_READING_RECORDED: &str = "recorded";
    pub const LATEST_READING_AT: &str = " at ";
    pub const LATEST_READING_NO_TIME: &str = " (timestamp unavailable)";
    pub const SLEEP_QUALITY_PREFIX: &str = " (quality: ";
    pub const SLEEP_QUALITY_SUFFIX: &str = ")";

    pub const BASELINE_PREFIX: &str = "Personal baseline: ";
    pub const BASELINE_NONE: &str = "Personal baseline: none.";

    pub const HISTORY_PREFIX: &str = "History: ";
    pub const HISTORY_SUFFIX: &str = " previous readings.";

    pub const ESCALATION_CONTINUE: &str = "Recommended action: continue routine monitoring.";
    pub const ESCALATION_SELF_CARE: &str = "Recommended action: follow self-care guidance.";
    pub const ESCALATION_FLAG_FOR_GROUND: &str =
        "Recommended action: flagged for ground review at next sync.";
    pub const ESCALATION_URGENT: &str =
        "Recommended action: notify the onboard CMO, open the offline protocol and send to ground.";

    pub const SOURCE_PUBLISHED_PREFIX: &str = "Source: published evidence";
    pub const SOURCE_APPROVED_PREFIX: &str = "Source: approved protocol";
    pub const SOURCE_DEMO_PREFIX: &str = "Source: demo heuristic";
    pub const SOURCE_REF_SEP: &str = " — ";
    pub const SOURCE_NO_REF: &str = " (no reference cited)";
    pub const DEMO_DISCLAIMER: &str = " Demo rule, not medical advice.";
}

/// Identificador estable de cada línea determinista de hechos.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactId {
    Indicator,
    Status,
    Reason,
    Direction,
    LatestReading,
    Baseline,
    History,
    Escalation,
    Source,
}

/// Línea determinista en inglés construida directamente desde [`Evaluation`].
///
/// La interfaz puede mostrar este bloque completo sin requerir ningún modelo SLM.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactLine {
    pub id: FactId,
    pub text: String,
}

impl FactLine {
    fn new(id: FactId, text: impl Into<String>) -> Self {
        Self {
            id,
            text: text.into(),
        }
    }
}

/// Renderiza de forma pura y determinista los hechos visibles de una [`Evaluation`].
///
/// Funciona sin motor de inferencia: cuando no hay SLM o su nota se descarta, la UI
/// muestra únicamente estas líneas en inglés (RNF-14).
pub fn render_facts(eval: &Evaluation) -> Vec<FactLine> {
    let mut facts = Vec::with_capacity(9);

    facts.push(FactLine::new(
        FactId::Indicator,
        format_indicator(&eval.indicator, eval.body_zone),
    ));
    facts.push(FactLine::new(
        FactId::Status,
        status_text(eval.status, eval.direction),
    ));

    if let Some(reason) = eval.reason {
        facts.push(FactLine::new(
            FactId::Reason,
            reason_text(reason, eval.history_count, eval.history_needed),
        ));
    }

    if eval.status != Status::NotEvaluated || eval.direction.is_some() || eval.persistent {
        facts.push(FactLine::new(
            FactId::Direction,
            direction_text(eval.direction, eval.persistent),
        ));
    }

    facts.push(FactLine::new(
        FactId::LatestReading,
        format_latest_reading(
            &eval.indicator,
            eval.latest_value.as_ref(),
            eval.latest_measured_at.map(|ts| ts.to_rfc3339()),
        ),
    ));

    facts.push(FactLine::new(
        FactId::Baseline,
        format_baseline(&eval.indicator, eval.baseline),
    ));

    facts.push(FactLine::new(
        FactId::History,
        format!(
            "{}{}/{}{}",
            strings::HISTORY_PREFIX,
            eval.history_count,
            eval.history_needed,
            strings::HISTORY_SUFFIX
        ),
    ));

    facts.push(FactLine::new(
        FactId::Escalation,
        escalation_text(eval.status, eval.escalation),
    ));

    facts.push(FactLine::new(
        FactId::Source,
        format_source(eval.source_kind, eval.source_ref.as_deref()),
    ));

    facts
}

fn indicator_label(indicator: &str) -> String {
    if let Some((_, label)) = strings::INDICATOR_LABELS
        .iter()
        .find(|(key, _)| *key == indicator)
    {
        return (*label).to_owned();
    }
    if indicator.is_empty() {
        return strings::UNKNOWN_INDICATOR.to_owned();
    }
    let formatted = indicator.replace('_', " ");
    let mut chars = formatted.chars();
    match chars.next() {
        Some(first) => {
            let mut out: String = first.to_uppercase().collect();
            out.push_str(chars.as_str());
            if indicator.ends_with("_event")
                || strings::INDICATOR_UNITS
                    .iter()
                    .any(|(key, _)| *key == indicator)
            {
                out
            } else {
                format!("{out}{}", strings::EVENT_SUFFIX)
            }
        }
        None => strings::UNKNOWN_INDICATOR.to_owned(),
    }
}

fn body_zone_label(zone: BodyZone) -> &'static str {
    strings::BODY_ZONE_LABELS
        .iter()
        .find(|(z, _)| *z == zone)
        .map_or("other", |(_, label)| *label)
}

fn format_indicator(indicator: &str, body_zone: Option<BodyZone>) -> String {
    let name = indicator_label(indicator);
    match body_zone {
        Some(zone) => format!(
            "{}{} ({}).",
            strings::INDICATOR_PREFIX,
            name,
            body_zone_label(zone)
        ),
        None => format!("{}{name}.", strings::INDICATOR_PREFIX),
    }
}

fn status_text(status: Status, direction: Option<Direction>) -> &'static str {
    match (status, direction) {
        (Status::NotEvaluated, _) => strings::STATUS_NOT_EVALUATED,
        (Status::Stable, _) => strings::STATUS_STABLE,
        (Status::Change, Some(Direction::Up)) => strings::STATUS_CHANGE_UP,
        (Status::Change, Some(Direction::Down)) => strings::STATUS_CHANGE_DOWN,
        (Status::Change, None) => strings::STATUS_CHANGE_GENERAL,
        (Status::Alarm, _) => strings::STATUS_ALARM,
    }
}

fn reason_text(reason: NotEvaluatedReason, history_count: usize, history_needed: usize) -> String {
    match reason {
        NotEvaluatedReason::NoData => strings::REASON_NO_DATA.to_owned(),
        NotEvaluatedReason::TooFewData => format!(
            "{}{}{}{}/{}{}",
            strings::REASON_TOO_FEW_PREFIX,
            history_needed,
            strings::REASON_TOO_FEW_MID,
            history_count,
            history_needed,
            strings::REASON_TOO_FEW_SUFFIX
        ),
        NotEvaluatedReason::Stale => strings::REASON_STALE.to_owned(),
        NotEvaluatedReason::NotApplicable => strings::REASON_NOT_APPLICABLE.to_owned(),
    }
}

fn direction_text(direction: Option<Direction>, persistent: bool) -> &'static str {
    match (direction, persistent) {
        (Some(Direction::Up), true) => strings::DIRECTION_UP_PERSISTENT,
        (Some(Direction::Up), false) => strings::DIRECTION_UP_NON_PERSISTENT,
        (Some(Direction::Down), true) => strings::DIRECTION_DOWN_PERSISTENT,
        (Some(Direction::Down), false) => strings::DIRECTION_DOWN_NON_PERSISTENT,
        (None, true) => strings::DIRECTION_NONE_PERSISTENT,
        (None, false) => strings::DIRECTION_NONE_NON_PERSISTENT,
    }
}

fn escalation_text(status: Status, escalation: Escalation) -> &'static str {
    if status == Status::Alarm || escalation == Escalation::Urgent {
        return strings::ESCALATION_URGENT;
    }
    match escalation {
        Escalation::Continue => strings::ESCALATION_CONTINUE,
        Escalation::SelfCare => strings::ESCALATION_SELF_CARE,
        Escalation::FlagForGround => strings::ESCALATION_FLAG_FOR_GROUND,
        Escalation::Urgent => strings::ESCALATION_URGENT,
    }
}

fn format_source(source_kind: SourceKind, source_ref: Option<&str>) -> String {
    let prefix = match source_kind {
        SourceKind::Published => strings::SOURCE_PUBLISHED_PREFIX,
        SourceKind::ApprovedProtocol => strings::SOURCE_APPROVED_PREFIX,
        SourceKind::DemoHeuristic => strings::SOURCE_DEMO_PREFIX,
    };

    let mut out = String::from(prefix);
    match source_ref.map(str::trim).filter(|s| !s.is_empty()) {
        Some(reference) => {
            out.push_str(strings::SOURCE_REF_SEP);
            out.push_str(reference);
            out.push('.');
        }
        None => {
            out.push_str(strings::SOURCE_NO_REF);
            out.push('.');
        }
    }

    if source_kind == SourceKind::DemoHeuristic {
        out.push_str(strings::DEMO_DISCLAIMER);
    }
    out
}

fn indicator_unit(indicator: &str) -> &'static str {
    strings::INDICATOR_UNITS
        .iter()
        .find(|(key, _)| *key == indicator)
        .map_or("", |(_, unit)| *unit)
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

fn format_value_with_unit(num: f64, unit: &str) -> String {
    let formatted = format_number(num);
    match unit {
        "" => formatted,
        "%" | "/10" => format!("{formatted}{unit}"),
        _ => format!("{formatted} {unit}"),
    }
}

fn format_reading_value(indicator: &str, value: &Value) -> String {
    let unit = indicator_unit(indicator);
    match indicator {
        "heart_rate" => json_num(value, "bpm").map(|n| format_value_with_unit(n, unit)),
        "blood_pressure" => match (json_num(value, "systolic"), json_num(value, "diastolic")) {
            (Some(sys), Some(dia)) => Some(format!(
                "{}/{} {unit}",
                format_number(sys),
                format_number(dia)
            )),
            (Some(sys), None) => Some(format_value_with_unit(sys, unit)),
            (None, Some(dia)) => Some(format_value_with_unit(dia, unit)),
            (None, None) => None,
        },
        "body_temperature" => json_num(value, "celsius").map(|n| format_value_with_unit(n, unit)),
        "glucose" => json_num(value, "mg_dl").map(|n| format_value_with_unit(n, unit)),
        "sleep" => json_num(value, "hours").map(|n| {
            let mut s = format_value_with_unit(n, unit);
            if let Some(q) = value
                .get("quality")
                .and_then(Value::as_str)
                .filter(|q| matches!(*q, "poor" | "fair" | "good"))
            {
                s.push_str(strings::SLEEP_QUALITY_PREFIX);
                s.push_str(q);
                s.push_str(strings::SLEEP_QUALITY_SUFFIX);
            }
            s
        }),
        "mood" | "symptom" => json_num(value, "score").map(|n| format_value_with_unit(n, unit)),
        "oxygen_in_blood" => json_num(value, "percentage").map(|n| format_value_with_unit(n, unit)),
        "space_radiation" => json_num(value, "usv_h").map(|n| format_value_with_unit(n, unit)),
        "distance_from_earth" => json_num(value, "km").map(|n| format_value_with_unit(n, unit)),
        "nutrition" => json_num(value, "kcal").map(|n| format_value_with_unit(n, unit)),
        _ => None,
    }
    .or_else(|| {
        value
            .as_f64()
            .filter(|n| n.is_finite())
            .map(|n| format_value_with_unit(n, unit))
    })
    .or_else(|| {
        value.as_object().and_then(|map| {
            map.values()
                .find_map(|v| v.as_f64().filter(|n| n.is_finite()))
                .map(|n| format_value_with_unit(n, unit))
        })
    })
    .unwrap_or_else(|| strings::LATEST_READING_RECORDED.to_owned())
}

fn json_num(value: &Value, key: &str) -> Option<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
}

fn format_latest_reading(
    indicator: &str,
    latest_value: Option<&Value>,
    measured_at: Option<String>,
) -> String {
    let Some(val) = latest_value else {
        return strings::LATEST_READING_NONE.to_owned();
    };
    let rendered = format_reading_value(indicator, val);
    match measured_at {
        Some(ts) => format!(
            "{}{rendered}{}{ts}.",
            strings::LATEST_READING_PREFIX,
            strings::LATEST_READING_AT
        ),
        None => format!(
            "{}{rendered}{}.",
            strings::LATEST_READING_PREFIX,
            strings::LATEST_READING_NO_TIME
        ),
    }
}

fn format_baseline(indicator: &str, baseline: Option<f64>) -> String {
    let Some(b) = baseline.filter(|n| n.is_finite()) else {
        return strings::BASELINE_NONE.to_owned();
    };
    let unit = indicator_unit(indicator);
    format!(
        "{}{}.",
        strings::BASELINE_PREFIX,
        format_value_with_unit(b, unit)
    )
}
