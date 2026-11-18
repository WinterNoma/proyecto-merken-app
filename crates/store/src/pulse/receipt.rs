use super::{
    PreparedPulse, PulseOutboxState, PulseReceipt,
    target::{expired, unresolved_root},
};
use crate::{
    StoreError,
    records::{err, insert_local_record},
};
use pulsys_core::{
    catalog::Indicator,
    measurement::Measurement,
    record::{RecordDto, RecordSource},
    time::Timestamp,
};
use rusqlite::{OptionalExtension, Transaction, params};
use sha2::{Digest, Sha256};

pub(super) fn fingerprint(action: &PreparedPulse) -> Result<[u8; 32], StoreError> {
    let fields = (
        "save_pulse.v1",
        &action.context.target,
        &action.context.correction_root_id,
        &action.replaces,
        action.bpm.to_bits(),
        &action.note,
        RecordSource::Form,
        action.measured_at_ms,
    );
    Ok(Sha256::digest(serde_json::to_vec(&fields)?).into())
}

pub(super) fn replay_pulse(
    tx: &Transaction<'_>,
    key: &str,
    fingerprint: &[u8; 32],
) -> Result<Option<PulseReceipt>, StoreError> {
    let operation: Option<(Option<Vec<u8>>, Option<String>)> = tx
        .query_row(
            "SELECT fingerprint, receipt_json FROM pulse_operations WHERE key = ?1",
            [key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    match operation {
        None => Ok(None),
        Some((None, None)) => Err(expired()),
        Some((Some(saved), Some(receipt))) if saved.as_slice() == fingerprint.as_slice() => {
            Ok(Some(serde_json::from_str(&receipt)?))
        }
        Some((Some(_), Some(_))) => Err(err(
            "idempotency_conflict",
            "key",
            "The key already belongs to a different pulse",
        )),
        Some(_) => Err(StoreError::storage()),
    }
}

pub(super) fn pulse_record(
    action: &PreparedPulse,
    measurement: Measurement,
    measured_at: Timestamp,
    now: Timestamp,
    record_id: String,
    app_version: &str,
) -> RecordDto {
    RecordDto {
        record_id,
        indicator: Indicator::HeartRate,
        value: measurement.to_value(),
        unit: measurement.unit().to_owned(),
        measured_at,
        logged_at: now,
        source: RecordSource::Form,
        interpreted_by: None,
        replaces: action.replaces.clone(),
        external_id: None,
        note: action.note.clone(),
        schema_version: 1,
        app_version: Some(app_version.into()),
    }
}

pub(super) fn save_pulse(
    tx: &Transaction<'_>,
    action: PreparedPulse,
    dto: &RecordDto,
    fingerprint: &[u8; 32],
) -> Result<PulseReceipt, StoreError> {
    let is_demo = action.is_demo();
    let root_id = if dto.replaces.is_some() {
        action
            .context
            .correction_root_id
            .ok_or_else(unresolved_root)?
    } else {
        dto.record_id.clone()
    };
    insert_local_record(tx, dto, None, &root_id, is_demo)?;
    let receipt = PulseReceipt {
        key: action.key,
        record_id: dto.record_id.clone(),
        root_id,
        replaces: action.replaces,
        bpm: action.bpm,
        unit: dto.unit.clone(),
        source: dto.source,
        measured_at_ms: dto.measured_at.millis(),
        logged_at_ms: dto.logged_at.millis(),
        is_demo,
        outbox_state: if is_demo {
            PulseOutboxState::LocalOnly
        } else {
            PulseOutboxState::Pending
        },
    };
    tx.execute(
        "INSERT INTO pulse_operations(key, fingerprint, receipt_json, is_demo)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            receipt.key,
            fingerprint.as_slice(),
            serde_json::to_string(&receipt)?,
            is_demo
        ],
    )?;
    Ok(receipt)
}
