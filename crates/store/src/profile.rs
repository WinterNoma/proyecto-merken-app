use crate::{Store, StoreError, api::ProfileResponse, records::err};
use pulsys_core::time::Timestamp;
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::Value;

pub(crate) const ASTRONAUT_ID_FIELD: &str = "astronaut.astronaut_id";

pub(crate) enum ProfileIdentity {
    Local(String),
    Demo(String),
}

pub(crate) fn profile_identity(conn: &Connection) -> Result<Option<ProfileIdentity>, StoreError> {
    let profile: Option<(String, bool)> = conn
        .query_row(
            "SELECT astronaut_json, is_demo FROM profile WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((astronaut_json, is_demo)) = profile else {
        return Ok(None);
    };
    let astronaut: Value = serde_json::from_str(&astronaut_json)?;
    let id = astronaut_id(&astronaut)?.to_owned();
    let identity = if is_demo {
        ProfileIdentity::Demo(id)
    } else {
        ProfileIdentity::Local(id)
    };
    Ok(Some(identity))
}

fn astronaut_id(astronaut: &Value) -> Result<&str, StoreError> {
    astronaut
        .get("astronaut_id")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
        .ok_or_else(|| {
            err(
                "invalid_schema",
                ASTRONAUT_ID_FIELD,
                "The profile needs a stable astronaut ID",
            )
        })
}

impl Store {
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
            core_version: pulsys_core::VERSION,
            rules_version: pulsys_core::RULES_VERSION,
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
}

/// Guarda el perfil dentro de una transacción ya abierta. Un perfil real
/// borra antes todo lo DEMO.
pub(crate) fn upsert_profile_in(
    tx: &Transaction<'_>,
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
    astronaut_id(a)?;
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
    if is_demo {
        tx.execute(
            "UPDATE sync_state SET value = lower(hex(randomblob(16))) WHERE key = 'demo_generation'",
            [],
        )?;
    } else {
        clear_demo_in(tx)?;
    }
    tx.execute("INSERT INTO profile(id,astronaut_json,medical_profile_json,medical_conditions_json,profile_version,is_demo,updated_at) VALUES(1,?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET astronaut_json=excluded.astronaut_json, medical_profile_json=excluded.medical_profile_json, medical_conditions_json=excluded.medical_conditions_json, profile_version=excluded.profile_version, is_demo=excluded.is_demo, updated_at=excluded.updated_at", params![serde_json::to_string(a)?,serde_json::to_string(m)?,serde_json::to_string(c)?,version,is_demo,now.millis()])?;
    Ok(())
}

/// Borra todo lo DEMO dentro de una transacción ya abierta. Se llama en la
/// misma transacción de la primera escritura real (revisión de Sol, B1), para
/// que un dato real nunca conviva con historial sintético.
pub(crate) fn clear_demo_in(tx: &Transaction<'_>) -> Result<(), StoreError> {
    tx.execute(
        "UPDATE pulse_operations SET fingerprint = NULL, receipt_json = NULL
         WHERE is_demo = 1 AND receipt_json IS NOT NULL",
        [],
    )?;
    tx.execute("DELETE FROM evaluations WHERE is_demo=1", [])?;
    tx.execute("DELETE FROM records WHERE is_demo=1", [])?;
    tx.execute("DELETE FROM events WHERE is_demo=1", [])?;
    tx.execute("DELETE FROM profile WHERE is_demo=1", [])?;
    Ok(())
}
