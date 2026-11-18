use super::{DemoTarget, PreparedPulse, PulseTarget};
use crate::{
    StoreError,
    profile::{ASTRONAUT_ID_FIELD, ProfileIdentity, profile_identity},
    record_graph::correction_root,
    records::err,
};
use pulsys_core::catalog::Indicator;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct PulseContext {
    pub(super) target: PulseTarget,
    pub(super) correction_root_id: Option<String>,
    pub(super) demo_to_replace: Option<DemoTarget>,
}

struct Replacement {
    root_id: String,
    is_demo: bool,
}

pub(super) fn pulse_context(
    conn: &Connection,
    replaces: Option<&str>,
) -> Result<PulseContext, StoreError> {
    let (store_id, generation, has_demo): (String, String, bool) = conn.query_row(
        "SELECT
             (SELECT value FROM sync_state WHERE key = 'pulse_store_id'),
             (SELECT value FROM sync_state WHERE key = 'demo_generation'),
             EXISTS(SELECT 1 FROM records WHERE is_demo = 1)
                 OR EXISTS(SELECT 1 FROM events WHERE is_demo = 1)
                 OR EXISTS(SELECT 1 FROM evaluations WHERE is_demo = 1)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let (local_id, demo_id) = match profile_identity(conn)? {
        Some(ProfileIdentity::Local(id)) => (Some(id), None),
        Some(ProfileIdentity::Demo(id)) => (None, Some(id)),
        None => (None, None),
    };
    let demo = (has_demo || demo_id.is_some()).then_some(DemoTarget {
        astronaut_id: demo_id,
        generation,
    });
    let replacement = replaces.map(|id| pulse_replacement(conn, id)).transpose()?;
    let (target, demo_to_replace) = if replacement.as_ref().is_some_and(|record| record.is_demo) {
        (PulseTarget::Demo(demo.ok_or_else(expired)?), None)
    } else {
        (
            PulseTarget::Local {
                store_id,
                astronaut_id: local_id,
            },
            demo,
        )
    };
    Ok(PulseContext {
        target,
        correction_root_id: replacement.map(|record| record.root_id),
        demo_to_replace,
    })
}

pub(super) fn revalidate_target(
    conn: &Connection,
    action: &PreparedPulse,
) -> Result<PulseContext, StoreError> {
    let context = pulse_context(conn, action.replaces.as_deref()).map_err(|error| {
        if error.field.as_deref() == Some(ASTRONAUT_ID_FIELD)
            || (error.code == "not_found" && action.is_demo())
        {
            return expired();
        }
        error
    })?;
    if context.target != action.context.target
        || context.correction_root_id != action.context.correction_root_id
    {
        return Err(expired());
    }
    Ok(context)
}

fn pulse_replacement(conn: &Connection, id: &str) -> Result<Replacement, StoreError> {
    let old: Option<(String, bool)> = conn
        .query_row(
            "SELECT indicator, is_demo FROM records WHERE record_id = ?1
               AND pulsys_valid_history_record(indicator, value_json, unit, source)",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (indicator, is_demo) = old.ok_or_else(|| {
        err(
            "not_found",
            "replaces",
            "The record to correct does not exist",
        )
    })?;
    if indicator != Indicator::HeartRate.as_str() {
        return Err(err(
            "invalid_value",
            "replaces",
            "A pulse can only correct another pulse",
        ));
    }
    Ok(Replacement {
        root_id: correction_root(conn, id)?.ok_or_else(unresolved_root)?,
        is_demo,
    })
}

pub(super) fn unresolved_root() -> StoreError {
    err(
        "unresolved_root",
        "replaces",
        "The correction chain does not have a resolvable root",
    )
}

pub(super) fn expired() -> StoreError {
    err(
        "action_expired",
        "key",
        "The prepared pulse no longer applies to its destination",
    )
}
