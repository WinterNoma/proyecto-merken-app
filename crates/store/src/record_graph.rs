use crate::{StoreError, records::validated_record};
use pulsys_core::rules;
use rusqlite::{
    Connection, OptionalExtension,
    functions::{Context, FunctionFlags},
    params,
};

pub(crate) const VALID_RECORD_FUNCTION: &str = "pulsys_valid_history_record";

// UNION deduplica los enlaces visitados para que las cadenas circulares terminen.
const CORRECTION_ROOT: &str = "
WITH RECURSIVE ancestors(record_id, replaces) AS (
    SELECT record_id, replaces FROM records
    WHERE record_id = ?1 AND pulsys_valid_history_record(indicator, value_json, unit, source)
    UNION
    SELECT parent.record_id, parent.replaces
    FROM ancestors a JOIN records parent ON parent.record_id = a.replaces
    WHERE pulsys_valid_history_record(parent.indicator, parent.value_json, parent.unit, parent.source)
)
SELECT record_id FROM ancestors WHERE replaces IS NULL";

const WINNING_RECORD: &str = "
WITH RECURSIVE members(record_id, seq, local_order, measured_at, logged_at, received_at) AS (
    SELECT record_id, seq, local_order, measured_at, logged_at, received_at FROM records
    WHERE record_id = ?1
    UNION ALL
    SELECT child.record_id, child.seq, child.local_order, child.measured_at,
           child.logged_at, child.received_at
    FROM members m JOIN records child ON child.replaces = m.record_id
    WHERE pulsys_valid_history_record(child.indicator, child.value_json, child.unit, child.source)
)
SELECT record_id FROM members
WHERE measured_at <= min(coalesce(received_at, logged_at), 9223372036854775807 - ?2) + ?2
ORDER BY (seq IS NULL) DESC, seq DESC, local_order DESC, record_id DESC
LIMIT 1";

pub(crate) fn register_record_validation(conn: &Connection) -> Result<(), StoreError> {
    conn.create_scalar_function(
        VALID_RECORD_FUNCTION,
        4,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        valid_record_from_sql,
    )?;
    Ok(())
}

pub(crate) fn valid_record_from_sql(context: &mut Context<'_>) -> rusqlite::Result<bool> {
    let indicator: String = context.get(0)?;
    let value: String = context.get(1)?;
    let unit: String = context.get(2)?;
    let source: String = context.get(3)?;
    Ok(validated_record(&indicator, &value, &unit, &source).is_some())
}

pub(crate) fn correction_root(
    conn: &Connection,
    record_id: &str,
) -> Result<Option<String>, StoreError> {
    conn.prepare_cached(CORRECTION_ROOT)?
        .query_row([record_id], |row| row.get(0))
        .optional()
        .map_err(StoreError::from)
}

pub(crate) fn winning_record(
    conn: &Connection,
    root_id: &str,
) -> Result<Option<String>, StoreError> {
    conn.prepare_cached(WINNING_RECORD)?
        .query_row(params![root_id, rules::FUTURE], |row| row.get(0))
        .optional()
        .map_err(StoreError::from)
}
