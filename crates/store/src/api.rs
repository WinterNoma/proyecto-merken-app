//! Tipos de entrada y salida para la interfaz.
use merken_core::{
    catalog::{BodyZone, Indicator},
    evaluate::{Evaluation, Overall, Status},
    plausibility::PlausibilityWarning,
    record::{Certainty, EventKind, Priority, RecordSource},
    time::Timestamp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
pub struct ProfileResponse {
    pub profile: Option<Value>,
    pub is_demo: bool,
    pub core_version: &'static str,
    pub rules_version: &'static str,
}
#[derive(Clone, Debug, Serialize)]
pub struct RecordItem {
    pub record_id: String,
    pub indicator: Indicator,
    pub body_zone: Option<BodyZone>,
    pub value: Value,
    pub unit: String,
    pub measured_at: Timestamp,
    pub logged_at: Timestamp,
    pub source: RecordSource,
    pub note: Option<String>,
    pub replaces: Option<String>,
    pub root_id: String,
    pub effective: bool,
    pub superseded_by: Option<String>,
    pub future_timestamp: bool,
    pub sync: String,
    pub is_demo: bool,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LogResult {
    NeedsConfirmation {
        warnings: Vec<PlausibilityWarning>,
    },
    Saved {
        record: Box<RecordItem>,
        warnings: Vec<PlausibilityWarning>,
        evaluation: Box<Evaluation>,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct BodyZoneStatus {
    pub body_zone: BodyZone,
    pub status: Status,
}
#[derive(Clone, Debug, Serialize)]
pub struct SyncStatus {
    pub pending: i64,
    pub failed: i64,
    pub last_push_at: Option<Timestamp>,
    pub last_pull_at: Option<Timestamp>,
}
#[derive(Clone, Debug, Serialize)]
pub struct StatusResponse {
    pub evaluated_at: Timestamp,
    pub met: Option<String>,
    pub overall: Overall,
    pub indicators: Vec<Evaluation>,
    pub symptoms: Vec<Evaluation>,
    pub alerts: Vec<Evaluation>,
    pub body_zones: Vec<BodyZoneStatus>,
    pub sync: SyncStatus,
    pub is_demo: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct EventItem {
    pub event_id: String,
    pub kind: EventKind,
    pub certainty: Certainty,
    pub message: Option<String>,
    pub body_zone: Option<BodyZone>,
    pub priority: Priority,
    pub occurred_at: Timestamp,
    pub logged_at: Timestamp,
    pub sync: String,
    pub is_demo: bool,
}
#[derive(Clone, Debug, Serialize)]
pub struct RaiseEventResult {
    pub event: EventItem,
    pub evaluation: Option<Evaluation>,
}
#[derive(Clone, Debug, Deserialize)]
pub struct LogRecordInput {
    pub indicator: Indicator,
    pub value: Value,
    pub measured_at: Option<Timestamp>,
    pub source: Option<RecordSource>,
    pub note: Option<String>,
    #[serde(default)]
    pub confirm_warnings: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct CorrectRecordInput {
    pub replaces: String,
    pub value: Value,
    pub measured_at: Option<Timestamp>,
    pub note: Option<String>,
    #[serde(default)]
    pub confirm_warnings: bool,
}
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ListRecordsInput {
    pub indicator: Option<Indicator>,
    pub body_zone: Option<BodyZone>,
    pub from: Option<Timestamp>,
    pub to: Option<Timestamp>,
    pub limit: Option<usize>,
    #[serde(default)]
    pub include_superseded: bool,
}
#[derive(Clone, Debug, Deserialize)]
pub struct RaiseEventInput {
    pub kind: EventKind,
    pub message: Option<String>,
    pub body_zone: Option<BodyZone>,
    pub occurred_at: Option<Timestamp>,
}

/// Resultado de `import_synthetic` (datos de la API de simulación, guardados como DEMO).
#[derive(Clone, Debug, Serialize)]
pub struct ImportResult {
    pub astronaut_id: String,
    pub display_name: String,
    pub imported: usize,
    pub rejected: usize,
    pub first_measured_at: Option<Timestamp>,
    pub last_measured_at: Option<Timestamp>,
}
