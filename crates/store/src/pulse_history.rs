use crate::{
    Store, StoreError,
    pulse::PulsePoint,
    record_graph::{correction_root, winning_record},
    records::{DEFAULT_LIMIT, validated_record},
};
use pulsys_core::{catalog::Indicator, measurement::Measurement, rules, time::Timestamp};
use rusqlite::{Connection, Row, params};
use std::collections::{HashMap, hash_map::Entry};

const MAX_PULSE_HISTORY_LIMIT: usize = 2_000;

const PULSE_CANDIDATES: &str = "
SELECT record_id, replaces, value_json, unit, source, measured_at, logged_at, is_demo
FROM records INDEXED BY records_indicator_history
WHERE indicator = 'heart_rate' AND measured_at BETWEEN ?1 AND ?2
  AND measured_at <= min(coalesce(received_at, logged_at), 9223372036854775807 - ?3) + ?3
  AND pulsys_valid_history_record(indicator, value_json, unit, source)
ORDER BY measured_at DESC, local_order DESC";

impl Store {
    pub fn effective_pulse_history(
        &self,
        from: Option<Timestamp>,
        to: Option<Timestamp>,
        limit: Option<usize>,
    ) -> Result<Vec<PulsePoint>, StoreError> {
        let limit = limit
            .unwrap_or(DEFAULT_LIMIT)
            .clamp(1, MAX_PULSE_HISTORY_LIMIT);
        let tx = self.conn.unchecked_transaction()?;
        let mut statement = tx.prepare_cached(PULSE_CANDIDATES)?;
        let mut rows = statement.query(params![
            from.map_or(i64::MIN, Timestamp::millis),
            to.map_or(i64::MAX, Timestamp::millis),
            rules::FUTURE,
        ])?;
        let mut winners = HashMap::new();
        let mut points = Vec::new();
        while points.len() < limit {
            let Some(row) = rows.next()? else { break };
            let mut point = pulse_point(row)?;
            if let Some(root_id) = candidate_root(&tx, &point)? {
                if !is_winning_point(&tx, &mut winners, &root_id, &point.record_id)? {
                    continue;
                }
                point.root_id = root_id;
            }
            points.push(point);
        }
        Ok(points)
    }
}

fn candidate_root(conn: &Connection, point: &PulsePoint) -> Result<Option<String>, StoreError> {
    if point.replaces.is_none() {
        return Ok(Some(point.record_id.clone()));
    }
    correction_root(conn, &point.record_id)
}

fn is_winning_point(
    conn: &Connection,
    winners: &mut HashMap<String, Option<String>>,
    root_id: &str,
    record_id: &str,
) -> Result<bool, StoreError> {
    let winner = match winners.entry(root_id.to_owned()) {
        Entry::Occupied(entry) => entry.into_mut(),
        Entry::Vacant(entry) => {
            let winner = winning_record(conn, entry.key())?;
            entry.insert(winner)
        }
    };
    Ok(winner.as_deref() == Some(record_id))
}

fn pulse_point(row: &Row<'_>) -> Result<PulsePoint, StoreError> {
    let record_id: String = row.get(0)?;
    let value: String = row.get(2)?;
    let unit: String = row.get(3)?;
    let source: String = row.get(4)?;
    let Some((Measurement::HeartRate { bpm }, _, source)) =
        validated_record(Indicator::HeartRate.as_str(), &value, &unit, &source)
    else {
        return Err(StoreError::storage());
    };
    Ok(PulsePoint {
        root_id: record_id.clone(),
        record_id,
        replaces: row.get(1)?,
        bpm: bpm.value(),
        unit,
        source,
        measured_at_ms: row.get(5)?,
        logged_at_ms: row.get(6)?,
        is_demo: row.get(7)?,
    })
}
