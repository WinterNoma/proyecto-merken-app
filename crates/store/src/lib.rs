//! Persistencia local y cola de sincronización.
pub mod api;
pub mod records;

pub use merken_seed::Scenario;
pub use records::PulledChange;

use api::ProfileResponse;
use merken_core::{ValidationError, time::Timestamp};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use uuid::ContextV7;

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
    fn from(_: rusqlite::Error) -> Self {
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
            merken_core::ErrorCode::InvalidValue => "invalid_value",
            merken_core::ErrorCode::InvalidSchema => "invalid_schema",
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
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::initialize(conn)
    }
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Self::initialize(conn)
    }
    fn initialize(mut conn: Connection) -> Result<Self, StoreError> {
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 3 {
            return Err(StoreError {
                code: "invalid_schema".into(),
                field: None,
                message: "Database version is not supported".into(),
            });
        }
        if version == 0 {
            let tx = conn.transaction()?;
            tx.execute_batch("CREATE TABLE profile (id INTEGER PRIMARY KEY CHECK(id=1), astronaut_json TEXT NOT NULL, medical_profile_json TEXT NOT NULL, medical_conditions_json TEXT NOT NULL, profile_version INTEGER NOT NULL, is_demo INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE records (local_order INTEGER PRIMARY KEY AUTOINCREMENT, record_id TEXT NOT NULL UNIQUE, indicator TEXT NOT NULL, body_zone TEXT, value_json TEXT NOT NULL, unit TEXT NOT NULL, measured_at INTEGER NOT NULL, logged_at INTEGER NOT NULL, source TEXT NOT NULL, interpreted_by_json TEXT, replaces TEXT, root_id TEXT NOT NULL, note TEXT, seq INTEGER, received_at INTEGER, is_demo INTEGER NOT NULL);
CREATE TABLE events (event_id TEXT PRIMARY KEY, kind TEXT NOT NULL, certainty TEXT NOT NULL, message TEXT, body_zone TEXT, priority TEXT NOT NULL, occurred_at INTEGER NOT NULL, logged_at INTEGER NOT NULL, seq INTEGER, received_at INTEGER, is_demo INTEGER NOT NULL);
CREATE TABLE evaluations (evaluation_id TEXT PRIMARY KEY, series_key TEXT NOT NULL, payload_json TEXT NOT NULL, evaluated_at INTEGER NOT NULL, is_demo INTEGER NOT NULL);
CREATE TABLE outbox (outbox_id INTEGER PRIMARY KEY AUTOINCREMENT, entity TEXT NOT NULL, entity_id TEXT NOT NULL, payload_json TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('pending','synced','failed')), attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT, created_at INTEGER NOT NULL, UNIQUE(entity, entity_id));
CREATE TABLE sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE INDEX records_measurement ON records(measured_at, local_order);
CREATE INDEX evaluations_series ON evaluations(series_key, evaluated_at);
PRAGMA user_version=1;")?;
            tx.commit()?;
        }
        if version <= 1 {
            // v2 (revisión de Sol, B4): todo lo que llega en `pull` se conserva
            // íntegro, aunque la app aún no sepa mostrar esa entidad.
            let tx = conn.transaction()?;
            tx.execute_batch(
                "CREATE TABLE pulled_changes (seq INTEGER PRIMARY KEY, entity TEXT NOT NULL,
                     entity_id TEXT NOT NULL, payload_json TEXT NOT NULL, applied_at INTEGER NOT NULL);
                 PRAGMA user_version=2;",
            )?;
            tx.commit()?;
        }
        if version <= 2 {
            // v3 (revisión de Sol, I1): `headache` salió del catálogo el 14-nov.
            // Lo pendiente de subir con ese indicador se conserva, pero no se envía.
            let tx = conn.transaction()?;
            tx.execute_batch(
                "UPDATE outbox SET state = 'failed',
                     last_error = 'retired_indicator: headache (catalog 2026-11-14); kept locally, not sent'
                 WHERE state = 'pending' AND entity IN ('record', 'evaluation')
                   AND json_extract(payload_json, '$.indicator') = 'headache';
                 PRAGMA user_version=3;",
            )?;
            tx.commit()?;
        }
        Ok(Self {
            conn,
            uuid_context: ContextV7::new(),
        })
    }
    pub fn get_profile(&self) -> Result<ProfileResponse, StoreError> {
        let row: Option<(String, String, String, bool)> = self.conn.query_row("SELECT astronaut_json, medical_profile_json, medical_conditions_json, is_demo FROM profile WHERE id=1", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let (profile, is_demo) = match row {
            Some((a, m, c, d)) => (
                Some(
                    serde_json::json!({"astronaut":serde_json::from_str::<Value>(&a)?,"medical_profile":serde_json::from_str::<Value>(&m)?,"medical_conditions":serde_json::from_str::<Value>(&c)?}),
                ),
                d,
            ),
            None => (None, false),
        };
        Ok(ProfileResponse {
            profile,
            is_demo,
            core_version: merken_core::VERSION,
            rules_version: merken_core::RULES_VERSION,
        })
    }
    pub fn upsert_profile(
        &mut self,
        profile: &Value,
        is_demo: bool,
        now: Timestamp,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        upsert_profile_in(&tx, profile, is_demo, now)?;
        tx.commit()?;
        Ok(())
    }
    pub fn clear_demo(&mut self) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        clear_demo_in(&tx)?;
        tx.commit()?;
        Ok(())
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

/// Guarda el perfil dentro de una transacción ya abierta. Un perfil real
/// borra antes todo lo DEMO.
pub(crate) fn upsert_profile_in(
    tx: &rusqlite::Transaction<'_>,
    profile: &Value,
    is_demo: bool,
    now: Timestamp,
) -> Result<(), StoreError> {
    let field = |name: &str| {
        profile.get(name).ok_or_else(|| StoreError {
            code: "invalid_schema".into(),
            field: Some(name.into()),
            message: "Required profile field is missing".into(),
        })
    };
    let a = field("astronaut")?;
    let m = field("medical_profile")?;
    let c = field("medical_conditions")?;
    let version = m
        .get("profile_version")
        .and_then(Value::as_i64)
        .ok_or_else(|| StoreError {
            code: "invalid_schema".into(),
            field: Some("profile_version".into()),
            message: "Profile version is required".into(),
        })?;
    if !is_demo {
        clear_demo_in(tx)?;
    }
    tx.execute("INSERT INTO profile(id,astronaut_json,medical_profile_json,medical_conditions_json,profile_version,is_demo,updated_at) VALUES(1,?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET astronaut_json=excluded.astronaut_json, medical_profile_json=excluded.medical_profile_json, medical_conditions_json=excluded.medical_conditions_json, profile_version=excluded.profile_version, is_demo=excluded.is_demo, updated_at=excluded.updated_at", params![serde_json::to_string(a)?,serde_json::to_string(m)?,serde_json::to_string(c)?,version,is_demo,now.millis()])?;
    Ok(())
}

/// Borra todo lo DEMO dentro de una transacción ya abierta. Se llama en la
/// misma transacción de la primera escritura real (revisión de Sol, B1), para
/// que un dato real nunca conviva con historial sintético.
pub(crate) fn clear_demo_in(tx: &rusqlite::Transaction<'_>) -> Result<(), StoreError> {
    tx.execute("DELETE FROM evaluations WHERE is_demo=1", [])?;
    tx.execute("DELETE FROM records WHERE is_demo=1", [])?;
    tx.execute("DELETE FROM events WHERE is_demo=1", [])?;
    tx.execute("DELETE FROM profile WHERE is_demo=1", [])?;
    Ok(())
}

pub fn version_sqlite() -> Result<String, rusqlite::Error> {
    let conn = Connection::open_in_memory()?;
    conn.query_row("SELECT sqlite_version()", [], |row| row.get(0))
}

#[derive(Clone, Debug, Serialize)]
pub struct OutboxItem {
    pub outbox_id: i64,
    pub entity: String,
    pub entity_id: String,
    pub payload: Value,
    pub state: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub created_at: Timestamp,
}
impl Store {
    pub fn pending_outbox(&self, limit: usize) -> Result<Vec<OutboxItem>, StoreError> {
        let mut stmt=self.conn.prepare("SELECT outbox_id,entity,entity_id,payload_json,state,attempts,last_error,created_at FROM outbox WHERE state='pending' ORDER BY outbox_id LIMIT ?1")?;
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = stmt.query_map([limit], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, i64>(7)?,
            ))
        })?;
        let mut result = Vec::new();
        for row in rows {
            let (outbox_id, entity, entity_id, payload, state, attempts, last_error, created_at) =
                row?;
            result.push(OutboxItem {
                outbox_id,
                entity,
                entity_id,
                payload: serde_json::from_str(&payload)?,
                state,
                attempts,
                last_error,
                created_at: Timestamp::from_millis(created_at),
            });
        }
        Ok(result)
    }
    pub fn mark_synced(
        &mut self,
        outbox_id: i64,
        seq: i64,
        received_at: Timestamp,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        let item: Option<(String, String)> = tx
            .query_row(
                "SELECT entity,entity_id FROM outbox WHERE outbox_id=?1",
                [outbox_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let (entity, id) = item.ok_or_else(|| StoreError {
            code: "not_found".into(),
            field: Some("outbox_id".into()),
            message: "Outbox entry not found".into(),
        })?;
        match entity.as_str() {
            "record" => {
                tx.execute(
                    "UPDATE records SET seq=?1, received_at=?2 WHERE record_id=?3",
                    params![seq, received_at.millis(), id],
                )?;
            }
            "event" => {
                tx.execute(
                    "UPDATE events SET seq=?1, received_at=?2 WHERE event_id=?3",
                    params![seq, received_at.millis(), id],
                )?;
            }
            _ => {}
        }
        tx.execute("UPDATE outbox SET state='synced', attempts=attempts+1, last_error=NULL WHERE outbox_id=?1",[outbox_id])?;
        tx.execute("INSERT INTO sync_state(key,value) VALUES('last_push_at',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value",[received_at.to_rfc3339()])?;
        tx.commit()?;
        Ok(())
    }
    pub fn mark_failed(
        &mut self,
        outbox_id: i64,
        error: &str,
        retryable: bool,
    ) -> Result<(), StoreError> {
        let state = if retryable { "pending" } else { "failed" };
        let n = self.conn.execute(
            "UPDATE outbox SET state=?1, attempts=attempts+1,last_error=?2 WHERE outbox_id=?3",
            params![state, error, outbox_id],
        )?;
        if n == 0 {
            return Err(StoreError {
                code: "not_found".into(),
                field: Some("outbox_id".into()),
                message: "Outbox entry not found".into(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migration_can_be_reopened() -> Result<(), Box<dyn std::error::Error>> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/merken-store-tests");
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
        let profile = merken_seed::demo_profile(now);
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
        let millis = u64::try_from(now.millis()).map_err(|_| StoreError {
            code: "invalid_value".into(),
            field: Some("now".into()),
            message: "Timestamp precedes the Unix epoch".into(),
        })?;
        let seconds = millis / 1000;
        let nanos =
            u32::try_from((millis % 1000) * 1_000_000).map_err(|_| StoreError::storage())?;
        Ok(uuid::Uuid::new_v7(uuid::Timestamp::from_unix(
            &self.uuid_context,
            seconds,
            nanos,
        ))
        .to_string())
    }
}

impl Store {
    pub fn list_events(&self, limit: usize) -> Result<Vec<api::EventItem>, StoreError> {
        use merken_core::record::{Certainty, EventKind, Priority};
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
                .map(|z| merken_core::catalog::BodyZone::parse(&z))
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
        use merken_core::evaluate::{EventView, evaluate_events};
        use merken_core::record::{Certainty, EventDto, EventKind};
        if !matches!(input.kind, EventKind::NeedHelp | EventKind::FeelingReport) {
            return Err(StoreError {
                code: "invalid_value".into(),
                field: Some("kind".into()),
                message: "Event kind is not allowed from the app".into(),
            });
        }
        if let Some(message) = &input.message {
            merken_core::validate_text("message", message, 2000)?;
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
    use merken_core::record::EventKind;
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
