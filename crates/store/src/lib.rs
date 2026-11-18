//! Persistencia local y cola de sincronización.
pub mod api;
pub mod export;
mod outbox;
mod profile;
pub mod pulse;
mod pulse_history;
mod record_graph;
pub mod records;
mod schema;
pub mod telemetry;

pub use export::{ExportCounts, ExportInput};
pub use outbox::OutboxItem;
pub(crate) use profile::{clear_demo_in, upsert_profile_in};
pub use pulsys_seed::Scenario;
pub use records::PulledChange;
pub use telemetry::{
    ListTelemetryInput, RecordTelemetryInput, RecordTelemetryResult, TelemetryItem,
    TelemetryReadingInput, VerifyTelemetryResult,
};

use pulsys_core::{ValidationError, time::Timestamp};
use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use uuid::ContextV7;

const SCHEMA_VERSION: i64 = 6;

#[derive(Clone, Debug, Serialize)]
pub struct StoreError {
    pub code: String,
    pub field: Option<String>,
    pub message: String,
}
impl StoreError {
    fn storage() -> Self {
        Self {
            code: "storage_error".into(),
            field: None,
            message: "A storage operation failed".into(),
        }
    }
}
impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        if matches!(
            error.sqlite_error_code(),
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
        ) {
            return Self {
                code: "busy".into(),
                field: None,
                message: "The database is busy".into(),
            };
        }
        Self::storage()
    }
}
impl From<serde_json::Error> for StoreError {
    fn from(_: serde_json::Error) -> Self {
        Self::storage()
    }
}
impl From<ValidationError> for StoreError {
    fn from(error: ValidationError) -> Self {
        let code = match error.code {
            pulsys_core::ErrorCode::InvalidValue => "invalid_value",
            pulsys_core::ErrorCode::InvalidSchema => "invalid_schema",
        };
        Self {
            code: code.into(),
            field: Some(error.field),
            message: error.message,
        }
    }
}
impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for StoreError {}

pub struct Store {
    conn: Connection,
    uuid_context: ContextV7,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        schema::supported_schema_version(&conn)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "recursive_triggers", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::initialize(conn)
    }
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "recursive_triggers", "ON")?;
        Self::initialize(conn)
    }
    pub fn close(self) -> Result<(), StoreError> {
        self.conn.close().map_err(|(_, error)| error.into())
    }

    pub fn is_empty_for_demo(&self) -> Result<bool, StoreError> {
        Ok(self.conn.query_row(
            "SELECT NOT (EXISTS(SELECT 1 FROM records)
                OR EXISTS(SELECT 1 FROM events)
                OR EXISTS(SELECT 1 FROM telemetry_readings)
                OR EXISTS(SELECT 1 FROM profile))",
            [],
            |row| row.get(0),
        )?)
    }

    fn initialize(mut conn: Connection) -> Result<Self, StoreError> {
        schema::migrate(&mut conn)?;
        record_graph::register_record_validation(&conn)?;
        Ok(Self {
            conn,
            uuid_context: ContextV7::new(),
        })
    }

    pub fn get_cursor(&self) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM sync_state WHERE key='pull_cursor'",
                [],
                |r| r.get(0),
            )
            .optional()?)
    }
}

pub fn version_sqlite() -> Result<String, rusqlite::Error> {
    let conn = Connection::open_in_memory()?;
    conn.query_row("SELECT sqlite_version()", [], |row| row.get(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_emptiness_checks_all_user_data_tables() -> Result<(), Box<dyn std::error::Error>> {
        for insert in [
            "INSERT INTO records(record_id, indicator, value_json, unit, measured_at,
                logged_at, source, root_id, is_demo)
             VALUES('symptom', 'symptom', '{}', 'score', 0, 0, 'form', 'symptom', 0)",
            "INSERT INTO events(event_id, kind, certainty, priority, occurred_at, logged_at, is_demo)
             VALUES('help', 'need_help', 'confirmed', 'urgent', 0, 0, 0)",
            "INSERT INTO telemetry_readings(astronaut_key, astronaut_name, indicator, value_json,
                unit, observed_at, batch_id, source, app_version, prev_hash, row_hash)
             VALUES('crew', 'Crew', 'sleep', '{}', 'h', 0, 'batch', 'live_api', 'test', '0', '1')",
            "INSERT INTO profile(id, astronaut_json, medical_profile_json, medical_conditions_json,
                profile_version, is_demo, updated_at) VALUES(1, '{}', '{}', '{}', 1, 0, 0)",
        ] {
            let store = Store::open_in_memory()?;
            assert!(store.is_empty_for_demo()?);
            store.conn.execute_batch(insert)?;
            assert!(!store.is_empty_for_demo()?);
        }
        Ok(())
    }

    #[test]
    fn derived_evaluations_do_not_fill_the_user_store() -> Result<(), Box<dyn std::error::Error>> {
        let mut store = Store::open_in_memory()?;
        store.status(Timestamp::from_millis(1_800_000_000_000), "test")?;
        assert!(store.is_empty_for_demo()?);
        Ok(())
    }

    #[test]
    fn migration_can_be_reopened() -> Result<(), Box<dyn std::error::Error>> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/pulsys-store-tests");
        std::fs::create_dir_all(&root)?;
        let path = root.join(format!("migration-{}.sqlite3", std::process::id()));
        {
            let store = Store::open(&path)?;
            assert!(store.get_profile()?.profile.is_none());
        }
        {
            let store = Store::open(&path)?;
            assert!(store.get_profile()?.profile.is_none());
        }
        std::fs::remove_file(path)?;
        Ok(())
    }
    #[test]
    fn profile_round_trip_and_clear_demo() -> Result<(), Box<dyn std::error::Error>> {
        let mut store = Store::open_in_memory()?;
        let now = Timestamp::from_millis(1_800_000_000_000);
        let profile = pulsys_seed::demo_profile(now);
        store.upsert_profile(&profile, true, now)?;
        assert_eq!(store.get_profile()?.profile, Some(profile));
        store.clear_demo()?;
        assert!(store.get_profile()?.profile.is_none());
        Ok(())
    }
}

impl Store {
    /// Genera un identificador a partir del instante proporcionado por la capa externa.
    pub fn new_id(&self, now: Timestamp) -> Result<String, StoreError> {
        new_id_at(&self.uuid_context, now)
    }
}

fn new_id_at(context: &ContextV7, now: Timestamp) -> Result<String, StoreError> {
    // El reloj de UUIDv7 empieza en 1970; los instantes clínicos conservan su signo.
    let millis = u64::try_from(now.millis().max(0)).map_err(|_| StoreError::storage())?;
    let seconds = millis / 1000;
    let nanos = u32::try_from((millis % 1000) * 1_000_000).map_err(|_| StoreError::storage())?;
    Ok(uuid::Uuid::new_v7(uuid::Timestamp::from_unix(context, seconds, nanos)).to_string())
}

impl Store {
    pub fn list_events(&self, limit: usize) -> Result<Vec<api::EventItem>, StoreError> {
        use pulsys_core::record::{Certainty, EventKind, Priority};
        let mut stmt=self.conn.prepare("SELECT e.event_id,e.kind,e.certainty,e.message,e.body_zone,e.priority,e.occurred_at,e.logged_at,e.is_demo,COALESCE(o.state,'synced') FROM events e LEFT JOIN outbox o ON o.entity='event' AND o.entity_id=e.event_id ORDER BY e.occurred_at DESC,e.rowid DESC LIMIT ?1")?;
        let rows = stmt.query_map([i64::try_from(limit.min(500)).unwrap_or(500)], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, bool>(8)?,
                r.get::<_, String>(9)?,
            ))
        })?;
        let mut events = Vec::new();
        for row in rows {
            let (
                event_id,
                kind,
                certainty,
                message,
                zone,
                priority,
                occurred_at,
                logged_at,
                is_demo,
                sync,
            ) = row?;
            let parse =
                |value: &str| -> Result<Value, StoreError> { Ok(Value::String(value.into())) };
            let kind: EventKind = serde_json::from_value(parse(&kind)?)?;
            let certainty: Certainty = serde_json::from_value(parse(&certainty)?)?;
            let priority: Priority = serde_json::from_value(parse(&priority)?)?;
            let body_zone = zone
                .map(|z| pulsys_core::catalog::BodyZone::parse(&z))
                .transpose()?;
            events.push(api::EventItem {
                event_id,
                kind,
                certainty,
                message,
                body_zone,
                priority,
                occurred_at: Timestamp::from_millis(occurred_at),
                logged_at: Timestamp::from_millis(logged_at),
                sync: if is_demo { "local_only".into() } else { sync },
                is_demo,
            });
        }
        Ok(events)
    }
    pub fn raise_event(
        &mut self,
        input: api::RaiseEventInput,
        now: Timestamp,
        app_version: &str,
    ) -> Result<api::RaiseEventResult, StoreError> {
        use pulsys_core::evaluate::{EventView, evaluate_events};
        use pulsys_core::record::{Certainty, EventDto, EventKind};
        if !matches!(input.kind, EventKind::NeedHelp | EventKind::FeelingReport) {
            return Err(StoreError {
                code: "invalid_value".into(),
                field: Some("kind".into()),
                message: "Event kind is not allowed from the app".into(),
            });
        }
        if let Some(message) = &input.message {
            pulsys_core::validate_text("message", message, 2000)?;
        }
        let event_id = self.new_id(now)?;
        let priority = input.kind.default_priority();
        let occurred_at = input.occurred_at.unwrap_or(now);
        let dto = EventDto {
            event_id: event_id.clone(),
            kind: input.kind,
            certainty: Certainty::Symptom,
            message: input.message.clone(),
            body_zone: input.body_zone,
            priority,
            occurred_at,
            logged_at: now,
            schema_version: 1,
            app_version: Some(app_version.into()),
        };
        let payload = serde_json::to_string(&dto)?;
        let kind = input.kind.as_str();
        let priority_str = if input.kind == EventKind::NeedHelp {
            "urgent"
        } else {
            "normal"
        };
        let tx = self.conn.transaction()?;
        clear_demo_in(&tx)?;
        tx.execute("INSERT INTO events(event_id,kind,certainty,message,body_zone,priority,occurred_at,logged_at,is_demo) VALUES(?1,?2,'symptom',?3,?4,?5,?6,?7,0)",params![event_id,kind,input.message,input.body_zone.map(|z|z.as_str()),priority_str,occurred_at.millis(),now.millis()])?;
        tx.execute("INSERT INTO outbox(entity,entity_id,payload_json,state,created_at) VALUES('event',?1,?2,'pending',?3)",params![event_id,payload,now.millis()])?;
        tx.commit()?;
        let evaluation = evaluate_events(
            &[EventView {
                event_id: event_id.clone(),
                kind: input.kind,
                occurred_at,
                priority,
            }],
            now,
        )
        .into_iter()
        .next();
        let event = api::EventItem {
            event_id,
            kind: input.kind,
            certainty: Certainty::Symptom,
            message: input.message,
            body_zone: input.body_zone,
            priority,
            occurred_at,
            logged_at: now,
            sync: "pending".into(),
            is_demo: false,
        };
        Ok(api::RaiseEventResult { event, evaluation })
    }
}

#[cfg(test)]
mod event_tests {
    use super::*;
    use pulsys_core::record::EventKind;
    #[test]
    fn urgent_event_is_queued() -> Result<(), Box<dyn std::error::Error>> {
        let mut store = Store::open_in_memory()?;
        let now = Timestamp::from_millis(1_800_000_000_000);
        let result = store.raise_event(
            api::RaiseEventInput {
                kind: EventKind::NeedHelp,
                message: Some("Help".into()),
                body_zone: None,
                occurred_at: None,
            },
            now,
            "0.1.0",
        )?;
        assert!(result.evaluation.is_some());
        let queued = store.pending_outbox(10)?;
        assert_eq!(queued.len(), 1);
        assert_eq!(
            queued
                .first()
                .and_then(|entry| entry.payload.get("priority"))
                .and_then(Value::as_str),
            Some("urgent")
        );
        Ok(())
    }
}

#[cfg(test)]
mod flow_tests;

#[cfg(test)]
mod pulse_tests;
