mod receipt;
mod target;

use crate::{
    Store, StoreError, new_id_at,
    records::{MAX_TEXT, err},
};
use pulsys_core::{
    measurement::Measurement, plausibility, record::RecordSource, time::Timestamp, units::Bpm,
    validate_text,
};
use rusqlite::TransactionBehavior;
use serde::{Deserialize, Serialize};

use receipt::{fingerprint, pulse_record, replay_pulse, save_pulse};
use target::{PulseContext, pulse_context, revalidate_target};

pub struct PulseDraft {
    pub key: String,
    pub bpm: f64,
    pub measured_at_ms: i64,
    pub note: Option<String>,
    pub replaces: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemoTarget {
    pub astronaut_id: Option<String>,
    pub generation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PulseTarget {
    Local {
        store_id: String,
        astronaut_id: Option<String>,
    },
    Demo(DemoTarget),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PulseWarning {
    pub code: String,
    pub rule_id: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Confirmation {
    NotRequired,
    Required,
    Approved,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PreparedPulse {
    key: String,
    bpm: f64,
    measured_at_ms: i64,
    note: Option<String>,
    replaces: Option<String>,
    context: PulseContext,
    warnings: Vec<PulseWarning>,
    confirmation: Confirmation,
}

impl PreparedPulse {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    pub fn measured_at_ms(&self) -> i64 {
        self.measured_at_ms
    }

    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    pub fn replaces(&self) -> Option<&str> {
        self.replaces.as_deref()
    }

    pub fn target(&self) -> &PulseTarget {
        &self.context.target
    }

    pub fn warnings(&self) -> &[PulseWarning] {
        &self.warnings
    }

    pub fn demo_to_replace(&self) -> Option<&DemoTarget> {
        self.context.demo_to_replace.as_ref()
    }

    pub fn requires_confirmation(&self) -> bool {
        self.confirmation != Confirmation::Approved
            && (self.confirmation == Confirmation::Required
                || needs_approval(&self.context, &self.warnings))
    }

    pub fn approve(mut self) -> Self {
        self.confirmation = Confirmation::Approved;
        self
    }

    fn is_demo(&self) -> bool {
        matches!(self.context.target, PulseTarget::Demo(_))
    }

    fn refresh_confirmation(&mut self, context: PulseContext, warnings: Vec<PulseWarning>) {
        let changed =
            self.warnings != warnings || self.context.demo_to_replace != context.demo_to_replace;
        self.context = context;
        self.warnings = warnings;
        if changed || self.requires_confirmation() {
            self.confirmation = Confirmation::Required;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PulseOutboxState {
    Pending,
    LocalOnly,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq)]
pub enum SaveOutcome {
    NeedsConfirmation(PreparedPulse),
    Saved(PulseReceipt),
}

pub fn checked_timestamp(ms: i64, field: &str) -> Result<Timestamp, StoreError> {
    let timestamp = Timestamp::from_millis(ms);
    if Timestamp::parse_rfc3339(&timestamp.to_rfc3339()) == Ok(timestamp) {
        return Ok(timestamp);
    }
    Err(err(
        "invalid_value",
        field,
        "Timestamp is outside the supported UTC range",
    ))
}

impl Store {
    pub fn prepare_pulse(
        &self,
        input: PulseDraft,
        now: Timestamp,
    ) -> Result<PreparedPulse, StoreError> {
        let key = normalized_key(&input.key)?;
        let (measurement, measured_at) =
            validate_pulse(input.bpm, input.measured_at_ms, input.note.as_deref())?;
        let tx = self.conn.unchecked_transaction()?;
        let context = pulse_context(&tx, input.replaces.as_deref())?;
        let warnings = pulse_warnings(&measurement, measured_at, now);
        let confirmation = if needs_approval(&context, &warnings) {
            Confirmation::Required
        } else {
            Confirmation::NotRequired
        };
        Ok(PreparedPulse {
            key,
            bpm: input.bpm,
            measured_at_ms: measured_at.millis(),
            note: input.note,
            replaces: input.replaces,
            context,
            warnings,
            confirmation,
        })
    }

    pub fn confirm_pulse(
        &mut self,
        mut action: PreparedPulse,
        clock: impl FnOnce() -> Result<Timestamp, StoreError>,
        app_version: &str,
    ) -> Result<SaveOutcome, StoreError> {
        action.key = normalized_key(&action.key)?;
        let fingerprint = fingerprint(&action)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(receipt) = replay_pulse(&tx, &action.key, &fingerprint)? {
            return Ok(SaveOutcome::Saved(receipt));
        }

        let context = revalidate_target(&tx, &action)?;
        let (measurement, measured_at) =
            validate_pulse(action.bpm, action.measured_at_ms, action.note.as_deref())?;
        let now = checked_timestamp(clock()?.millis(), "now")?;
        let warnings = pulse_warnings(&measurement, measured_at, now);
        action.refresh_confirmation(context, warnings);
        if action.requires_confirmation() {
            return Ok(SaveOutcome::NeedsConfirmation(action));
        }

        let dto = pulse_record(
            &action,
            measurement,
            measured_at,
            now,
            new_id_at(&self.uuid_context, now)?,
            app_version,
        );
        let receipt = save_pulse(&tx, action, &dto, &fingerprint)?;
        tx.commit()?;
        Ok(SaveOutcome::Saved(receipt))
    }
}

fn needs_approval(context: &PulseContext, warnings: &[PulseWarning]) -> bool {
    !warnings.is_empty() || context.demo_to_replace.is_some()
}

fn normalized_key(key: &str) -> Result<String, StoreError> {
    uuid::Uuid::parse_str(key)
        .map(|key| key.hyphenated().to_string())
        .map_err(|_| err("invalid_value", "key", "Expected a UUID"))
}

fn validate_pulse(
    bpm: f64,
    measured_at_ms: i64,
    note: Option<&str>,
) -> Result<(Measurement, Timestamp), StoreError> {
    let bpm =
        Bpm::try_from(bpm).map_err(|error| err("invalid_value", "value.bpm", &error.message))?;
    let measured_at = checked_timestamp(measured_at_ms, "measured_at_ms")?;
    if let Some(note) = note {
        validate_text("note", note, MAX_TEXT)?;
    }
    Ok((Measurement::HeartRate { bpm }, measured_at))
}

fn pulse_warnings(
    measurement: &Measurement,
    measured_at: Timestamp,
    now: Timestamp,
) -> Vec<PulseWarning> {
    plausibility::check(measurement, measured_at, now)
        .into_iter()
        .map(|warning| PulseWarning {
            code: warning.code,
            rule_id: warning.rule_id,
        })
        .collect()
}
