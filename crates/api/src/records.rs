use crate::{
    ApiError, App, BodyZone, Direction, Escalation, NotEvaluatedReason, PreparedPulse, PulsePoint,
    SaveOutcome, SourceKind, Status,
};
use pulsys_core::{catalog::Indicator, evaluate::Evaluation, time::Timestamp};
use pulsys_store::pulse::{PulseDraft, checked_timestamp};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct PulseInput {
    pub key: String,
    pub bpm: f64,
    pub measured_at_ms: Option<i64>,
    pub note: Option<String>,
    pub replaces: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryQuery {
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymptomEvaluation {
    pub status: Status,
    pub reason: Option<NotEvaluatedReason>,
    pub evaluated_at_ms: i64,
    pub rule_id: String,
    pub rule_version: String,
    pub source_kind: SourceKind,
    pub source_ref: Option<String>,
    pub input_record_ids: Vec<String>,
    pub input_event_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZoneState {
    pub zone_id: BodyZone,
    pub status: Status,
    pub symptoms: Vec<SymptomEvaluation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PulseEvaluation {
    pub status: Status,
    pub reason: Option<NotEvaluatedReason>,
    pub direction: Option<Direction>,
    pub persistent: bool,
    pub escalation: Escalation,
    pub evaluated_at_ms: i64,
    pub latest_measured_at_ms: Option<i64>,
    pub rule_id: String,
    pub rule_version: String,
    pub source_kind: SourceKind,
    pub source_ref: Option<String>,
    pub input_record_ids: Vec<String>,
    pub history_count: usize,
    pub history_needed: usize,
    pub baseline_bpm: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ZoneSnapshot {
    pub evaluated_at_ms: i64,
    pub is_demo: bool,
    pub is_store_empty: bool,
    pub zones: Vec<ZoneState>,
    pub pulse: PulseEvaluation,
}

pub fn prepare_pulse(app: &App, input: PulseInput) -> Result<PreparedPulse, ApiError> {
    let now = app.now()?;
    let draft = PulseDraft {
        key: input.key,
        bpm: input.bpm,
        measured_at_ms: input.measured_at_ms.unwrap_or(now.millis()),
        note: input.note,
        replaces: input.replaces,
    };
    app.store.prepare_pulse(draft, now).map_err(ApiError::from)
}

pub fn confirm_pulse(app: &mut App, action: PreparedPulse) -> Result<SaveOutcome, ApiError> {
    let clock = &app.clock;
    app.store
        .confirm_pulse(
            action,
            || clock().map_err(ApiError::into_store_error),
            &app.app_version,
        )
        .map_err(ApiError::from)
}

pub fn effective_pulse_history(
    app: &App,
    query: HistoryQuery,
) -> Result<Vec<PulsePoint>, ApiError> {
    let from = query
        .from_ms
        .map(|ms| checked_timestamp(ms, "from_ms"))
        .transpose()?;
    let to = query
        .to_ms
        .map(|ms| checked_timestamp(ms, "to_ms"))
        .transpose()?;
    if matches!((from, to), (Some(from), Some(to)) if from > to) {
        return Err(ApiError::new(
            "invalid_value",
            Some("to_ms"),
            "The end of the interval precedes its start",
        ));
    }
    app.store
        .effective_pulse_history(from, to, query.limit)
        .map_err(ApiError::from)
}

pub fn zone_status(app: &mut App) -> Result<ZoneSnapshot, ApiError> {
    let now = app.now()?;
    let status = app.store.status(now, &app.app_version)?;
    let pulse = status
        .indicators
        .into_iter()
        .find(|evaluation| evaluation.indicator == Indicator::HeartRate.as_str())
        .ok_or_else(|| ApiError::new("invalid_schema", None, "Pulse evaluation is missing"))?;
    let mut symptoms = BTreeMap::<BodyZone, Vec<SymptomEvaluation>>::new();
    for evaluation in status.symptoms {
        if let Some(zone) = evaluation.body_zone {
            symptoms
                .entry(zone)
                .or_default()
                .push(SymptomEvaluation::from_evaluation(evaluation));
        }
    }
    let zones = status
        .body_zones
        .into_iter()
        .map(|zone| ZoneState {
            zone_id: zone.body_zone,
            status: zone.status,
            symptoms: symptoms.remove(&zone.body_zone).unwrap_or_default(),
        })
        .collect();
    Ok(ZoneSnapshot {
        evaluated_at_ms: status.evaluated_at.millis(),
        is_demo: status.is_demo,
        is_store_empty: app.store.is_empty_for_demo()?,
        zones,
        pulse: PulseEvaluation::from_evaluation(pulse),
    })
}

impl SymptomEvaluation {
    fn from_evaluation(evaluation: Evaluation) -> Self {
        Self {
            status: evaluation.status,
            reason: evaluation.reason,
            evaluated_at_ms: evaluation.evaluated_at.millis(),
            rule_id: evaluation.rule_id,
            rule_version: evaluation.rule_version,
            source_kind: evaluation.source_kind,
            source_ref: evaluation.source_ref,
            input_record_ids: evaluation.input_record_ids,
            input_event_ids: evaluation.input_event_ids,
        }
    }
}

impl PulseEvaluation {
    fn from_evaluation(evaluation: Evaluation) -> Self {
        Self {
            status: evaluation.status,
            reason: evaluation.reason,
            direction: evaluation.direction,
            persistent: evaluation.persistent,
            escalation: evaluation.escalation,
            evaluated_at_ms: evaluation.evaluated_at.millis(),
            latest_measured_at_ms: evaluation.latest_measured_at.map(Timestamp::millis),
            rule_id: evaluation.rule_id,
            rule_version: evaluation.rule_version,
            source_kind: evaluation.source_kind,
            source_ref: evaluation.source_ref,
            input_record_ids: evaluation.input_record_ids,
            history_count: evaluation.history_count,
            history_needed: evaluation.history_needed,
            baseline_bpm: evaluation.baseline,
        }
    }
}
