//! DTO de registros y eventos para sincronización.
use crate::{
    catalog::{BodyZone, Indicator},
    time::Timestamp,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
macro_rules! dto_enum { ($name:ident { $($v:ident),+ $(,)? }) => { #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)] #[serde(rename_all = "snake_case")] pub enum $name { $($v),+ } }; }
dto_enum!(RecordSource {
    Form,
    Text,
    Voice,
    Earth,
    Garmin
});
dto_enum!(EventKind {
    NeedHelp,
    Syncope,
    VisualFieldLoss,
    SuspectedClot,
    FeelingReport
});
dto_enum!(Certainty { Symptom, Suspicion });
dto_enum!(Priority { Urgent, Normal });
impl EventKind {
    pub const fn default_priority(self) -> Priority {
        if self.is_alarm() {
            Priority::Urgent
        } else {
            Priority::Normal
        }
    }
    pub const fn is_alarm(self) -> bool {
        !matches!(self, Self::FeelingReport)
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeedHelp => "need_help",
            Self::Syncope => "syncope",
            Self::VisualFieldLoss => "visual_field_loss",
            Self::SuspectedClot => "suspected_clot",
            Self::FeelingReport => "feeling_report",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InterpretedBy {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordDto {
    pub record_id: String,
    pub indicator: Indicator,
    pub value: Value,
    pub unit: String,
    pub measured_at: Timestamp,
    pub logged_at: Timestamp,
    pub source: RecordSource,
    pub interpreted_by: Option<InterpretedBy>,
    pub replaces: Option<String>,
    pub external_id: Option<String>,
    pub note: Option<String>,
    pub schema_version: u32,
    pub app_version: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventDto {
    pub event_id: String,
    pub kind: EventKind,
    pub certainty: Certainty,
    pub message: Option<String>,
    pub body_zone: Option<BodyZone>,
    pub priority: Priority,
    pub occurred_at: Timestamp,
    pub logged_at: Timestamp,
    pub schema_version: u32,
    pub app_version: Option<String>,
}
