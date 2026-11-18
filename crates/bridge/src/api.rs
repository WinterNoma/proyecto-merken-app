use crate::{durable, worker::Worker};
use flutter_rust_bridge::frb;
use pulsys_api as facade;
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};

#[frb(opaque)]
pub struct Session {
    worker: Worker,
}

pub fn open_store(private_dir: String, app_version: String) -> Result<Session, ApiError> {
    let worker = Worker::open(
        PathBuf::from(private_dir),
        app_version,
        Box::new(facade::system_clock),
    )?;
    Ok(Session { worker })
}

pub fn close_store(session: &Session) -> Result<(), ApiError> {
    session.worker.close()
}

pub fn prepare_pulse(session: &Session, input: PulseInput) -> Result<PreparedPulse, ApiError> {
    session
        .worker
        .run(move |app| facade::prepare_pulse(app, input.into())?.try_into())
}

pub fn confirm_pulse(session: &Session, action: PreparedPulse) -> Result<SaveOutcome, ApiError> {
    session.worker.run(move |app| {
        let action = action.try_into()?;
        facade::confirm_pulse(app, action)?.try_into()
    })
}

pub fn effective_pulse_history(
    session: &Session,
    query: HistoryQuery,
) -> Result<Vec<PulsePoint>, ApiError> {
    session.worker.run(move |app| {
        Ok(facade::effective_pulse_history(app, query.into())?
            .into_iter()
            .map(PulsePoint::from)
            .collect())
    })
}

pub fn zone_status(session: &Session) -> Result<ZoneSnapshot, ApiError> {
    session
        .worker
        .run(|app| Ok(facade::zone_status(app)?.into()))
}

pub fn encode_prepared_pulse(action: PreparedPulse) -> Result<Vec<u8>, ApiError> {
    durable::encode(action)
}

pub fn decode_prepared_pulse(bytes: Vec<u8>) -> Result<PreparedPulse, ApiError> {
    durable::decode(&bytes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiError {
    pub code: String,
    pub field: Option<String>,
    pub message: String,
}

impl fmt::Display for ApiError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

#[derive(Clone, Debug, PartialEq)]
pub struct PulseInput {
    pub key: String,
    pub bpm: f64,
    pub measured_at_ms: Option<i64>,
    pub note: Option<String>,
    pub replaces: Option<String>,
}

// These fields and their Serde names are the on-disk schema of a pending pulse V1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedPulse {
    pub key: String,
    pub bpm: f64,
    pub measured_at_ms: i64,
    pub note: Option<String>,
    pub replaces: Option<String>,
    pub context: PulseContext,
    pub warnings: Vec<PulseWarning>,
    pub confirmation: PulseConfirmation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PulseContext {
    pub target: PulseTarget,
    pub correction_root_id: Option<String>,
    pub demo_to_replace: Option<DemoTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PulseTarget {
    Local {
        store_id: String,
        astronaut_id: Option<String>,
    },
    Demo(DemoTarget),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoTarget {
    pub astronaut_id: Option<String>,
    pub generation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PulseWarning {
    pub code: String,
    pub rule_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PulseConfirmation {
    NotRequired,
    Required,
    Approved,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SaveOutcome {
    NeedsConfirmation { prepared: PreparedPulse },
    Saved { receipt: PulseReceipt },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PulseReceipt {
    pub key: String,
    pub record_id: String,
    pub root_id: String,
    pub replaces: Option<String>,
    pub bpm: f64,
    pub unit: String,
    pub source: RecordSource,
    pub measured_at_ms: i64,
    pub logged_at_ms: i64,
    pub is_demo: bool,
    pub outbox_state: PulseOutboxState,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryQuery {
    pub from_ms: Option<i64>,
    pub to_ms: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PulsePoint {
    pub record_id: String,
    pub root_id: String,
    pub replaces: Option<String>,
    pub bpm: f64,
    pub unit: String,
    pub source: RecordSource,
    pub measured_at_ms: i64,
    pub logged_at_ms: i64,
    pub is_demo: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymptomEvaluation {
    pub status: EvaluationStatus,
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
    pub status: EvaluationStatus,
    pub symptoms: Vec<SymptomEvaluation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PulseEvaluation {
    pub status: EvaluationStatus,
    pub reason: Option<NotEvaluatedReason>,
    pub direction: Option<TrendDirection>,
    pub persistent: bool,
    pub escalation: EscalationLevel,
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
    pub zones: Vec<ZoneState>,
    pub pulse: PulseEvaluation,
}

macro_rules! transport_enum {
    ($name:ident from $domain:ident { $($variant:ident ),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl From<facade::$domain> for $name {
            fn from(value: facade::$domain) -> Self {
                match value {
                    $(facade::$domain::$variant => Self::$variant),+
                }
            }
        }
    };
}

transport_enum!(RecordSource from RecordSource { Form, Text, Voice, Earth, Garmin });
transport_enum!(PulseOutboxState from PulseOutboxState { Pending, LocalOnly });
transport_enum!(EvaluationStatus from Status { NotEvaluated, Stable, Change, Alarm });
transport_enum!(NotEvaluatedReason from NotEvaluatedReason { NoData, TooFewData, Stale, NotApplicable });
transport_enum!(TrendDirection from Direction { Up, Down });
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscalationLevel {
    ContinueReading,
    SelfCare,
    FlagForGround,
    Urgent,
}

impl From<facade::Escalation> for EscalationLevel {
    fn from(value: facade::Escalation) -> Self {
        match value {
            facade::Escalation::Continue => Self::ContinueReading,
            facade::Escalation::SelfCare => Self::SelfCare,
            facade::Escalation::FlagForGround => Self::FlagForGround,
            facade::Escalation::Urgent => Self::Urgent,
        }
    }
}
transport_enum!(SourceKind from SourceKind { Published, ApprovedProtocol, DemoHeuristic });
transport_enum!(BodyZone from BodyZone {
    Head, Neck, Chest, Abdomen, Back, LeftArm, RightArm, LeftHand, RightHand, LeftLeg, RightLeg,
    LeftFoot, RightFoot, Other,
});
