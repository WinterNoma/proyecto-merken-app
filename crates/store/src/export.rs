//! Exportación de todos los datos locales a un único documento JSON.
//!
//! Solo construye el `serde_json::Value`: la escritura del archivo vive en
//! `src-tauri`. Todas las lecturas ocurren dentro de UNA transacción de lectura
//! (snapshot coherente), incluida la verificación de la cadena de telemetría.

use crate::telemetry::{validate_astronaut_key, verify_telemetry_in};
use crate::{Store, StoreError};
use pulsys_core::time::Timestamp;
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// Versión del formato del archivo exportado.
pub const EXPORT_FORMAT_VERSION: u32 = 1;
/// Por encima de este número de lecturas el export exige filtrar por astronauta.
pub const MAX_EXPORT_TELEMETRY_ROWS: i64 = 1_000_000;

/// Opciones del export. Ambas son opcionales.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ExportInput {
    /// Por defecto `true`.
    pub include_telemetry: Option<bool>,
    /// Filtra SOLO la telemetría; el resto es del dispositivo (un solo perfil).
    pub astronaut_key: Option<String>,
}

/// Conteos de lo exportado, tal como los recibe la interfaz.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ExportCounts {
    pub records: u32,
    pub events: u32,
    pub evaluations: u32,
    pub telemetry_readings: u32,
    pub outbox_pending: u32,
}

fn ts(ms: i64) -> Value {
    Value::String(Timestamp::from_millis(ms).to_rfc3339())
}

fn ts_opt(ms: Option<i64>) -> Value {
    ms.map_or(Value::Null, ts)
}

/// JSON guardado como texto. Si una fila estuviera corrupta se conserva el texto
/// crudo en lugar de abortar el export completo.
fn parse_or_raw(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()))
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn sync_label(is_demo: bool, outbox_state: Option<String>) -> String {
    if is_demo {
        "local_only".to_owned()
    } else {
        outbox_state.unwrap_or_else(|| "synced".to_owned())
    }
}

fn export_profile(conn: &Connection) -> Result<Value, StoreError> {
    let row: Option<(String, String, String, bool, i64)> = conn
        .query_row(
            "SELECT astronaut_json, medical_profile_json, medical_conditions_json, is_demo, updated_at
             FROM profile WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .optional()?;
    Ok(match row {
        None => Value::Null,
        Some((astronaut, medical, conditions, is_demo, updated_at)) => json!({
            "astronaut": parse_or_raw(&astronaut),
            "medical_profile": parse_or_raw(&medical),
            "medical_conditions": parse_or_raw(&conditions),
            "is_demo": is_demo,
            "updated_at": ts(updated_at),
        }),
    })
}

fn export_records(conn: &Connection) -> Result<Vec<Value>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT r.record_id, r.indicator, r.body_zone, r.value_json, r.unit, r.measured_at,
                r.logged_at, r.source, r.interpreted_by_json, r.replaces, r.root_id, r.note,
                r.seq, r.received_at, r.is_demo, o.state
         FROM records r
         LEFT JOIN outbox o ON o.entity = 'record' AND o.entity_id = r.record_id
         ORDER BY r.local_order",
    )?;
    let rows = stmt.query_map([], |r| {
        let value_json: String = r.get(3)?;
        let interpreted_by: Option<String> = r.get(8)?;
        let is_demo: bool = r.get(14)?;
        let outbox_state: Option<String> = r.get(15)?;
        Ok(json!({
            "record_id": r.get::<_, String>(0)?,
            "indicator": r.get::<_, String>(1)?,
            "body_zone": r.get::<_, Option<String>>(2)?,
            "value": parse_or_raw(&value_json),
            "unit": r.get::<_, String>(4)?,
            "measured_at": ts(r.get(5)?),
            "logged_at": ts(r.get(6)?),
            "source": r.get::<_, String>(7)?,
            "interpreted_by": interpreted_by.as_deref().map_or(Value::Null, parse_or_raw),
            "replaces": r.get::<_, Option<String>>(9)?,
            "root_id": r.get::<_, String>(10)?,
            "note": r.get::<_, Option<String>>(11)?,
            "seq": r.get::<_, Option<i64>>(12)?,
            "received_at": ts_opt(r.get(13)?),
            "is_demo": is_demo,
            "sync": sync_label(is_demo, outbox_state),
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn export_events(conn: &Connection) -> Result<Vec<Value>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT e.event_id, e.kind, e.certainty, e.message, e.body_zone, e.priority,
                e.occurred_at, e.logged_at, e.seq, e.received_at, e.is_demo, o.state
         FROM events e
         LEFT JOIN outbox o ON o.entity = 'event' AND o.entity_id = e.event_id
         ORDER BY e.occurred_at ASC, e.rowid ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        let is_demo: bool = r.get(10)?;
        let outbox_state: Option<String> = r.get(11)?;
        Ok(json!({
            "event_id": r.get::<_, String>(0)?,
            "kind": r.get::<_, String>(1)?,
            "certainty": r.get::<_, String>(2)?,
            "message": r.get::<_, Option<String>>(3)?,
            "body_zone": r.get::<_, Option<String>>(4)?,
            "priority": r.get::<_, String>(5)?,
            "occurred_at": ts(r.get(6)?),
            "logged_at": ts(r.get(7)?),
            "seq": r.get::<_, Option<i64>>(8)?,
            "received_at": ts_opt(r.get(9)?),
            "is_demo": is_demo,
            "sync": sync_label(is_demo, outbox_state),
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn export_evaluations(conn: &Connection) -> Result<Vec<Value>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT evaluation_id, series_key, payload_json, evaluated_at, is_demo
         FROM evaluations ORDER BY evaluated_at ASC, rowid ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        let payload: String = r.get(2)?;
        Ok(json!({
            "evaluation_id": r.get::<_, String>(0)?,
            "series_key": r.get::<_, String>(1)?,
            "evaluated_at": ts(r.get(3)?),
            "is_demo": r.get::<_, bool>(4)?,
            "payload": parse_or_raw(&payload),
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn export_outbox(conn: &Connection) -> Result<(Vec<Value>, u32), StoreError> {
    let mut stmt = conn.prepare(
        "SELECT outbox_id, entity, entity_id, payload_json, state, attempts, last_error, created_at
         FROM outbox ORDER BY outbox_id ASC",
    )?;
    let mut pending: u32 = 0;
    let mut items = Vec::new();
    let rows = stmt.query_map([], |r| {
        let payload: String = r.get(3)?;
        let state: String = r.get(4)?;
        Ok((
            state.clone(),
            json!({
                "outbox_id": r.get::<_, i64>(0)?,
                "entity": r.get::<_, String>(1)?,
                "entity_id": r.get::<_, String>(2)?,
                "state": state,
                "attempts": r.get::<_, i64>(5)?,
                "last_error": r.get::<_, Option<String>>(6)?,
                "created_at": ts(r.get(7)?),
                "payload": parse_or_raw(&payload),
            }),
        ))
    })?;
    for row in rows {
        let (state, item) = row?;
        if state == "pending" {
            pending = pending.saturating_add(1);
        }
        items.push(item);
    }
    Ok((items, pending))
}

fn export_pulled_changes(conn: &Connection) -> Result<Vec<Value>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT seq, entity, entity_id, payload_json, applied_at FROM pulled_changes ORDER BY seq ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        let payload: String = r.get(3)?;
        Ok(json!({
            "seq": r.get::<_, i64>(0)?,
            "entity": r.get::<_, String>(1)?,
            "entity_id": r.get::<_, String>(2)?,
            "applied_at": ts(r.get(4)?),
            "payload": parse_or_raw(&payload),
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn export_sync_state(conn: &Connection) -> Result<Value, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT key, value FROM sync_state
         WHERE key IN ('pull_cursor', 'last_pull_at', 'last_push_at')
         ORDER BY key ASC",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut map = Map::new();
    for row in rows {
        let (key, value) = row?;
        map.insert(key, Value::String(value));
    }
    Ok(Value::Object(map))
}

fn export_telemetry(
    conn: &Connection,
    astronaut_key: Option<&str>,
) -> Result<(Value, u32), StoreError> {
    let total: i64 = match astronaut_key {
        Some(key) => conn.query_row(
            "SELECT COUNT(*) FROM telemetry_readings WHERE astronaut_key = ?1",
            [key],
            |r| r.get(0),
        )?,
        None => conn.query_row("SELECT COUNT(*) FROM telemetry_readings", [], |r| r.get(0))?,
    };
    if total > MAX_EXPORT_TELEMETRY_ROWS {
        return Err(StoreError {
            code: "invalid_value".into(),
            field: Some("include_telemetry".into()),
            message: "Too many telemetry readings to export in one file; filter by astronaut_key"
                .into(),
        });
    }

    let filter = if astronaut_key.is_some() {
        "WHERE astronaut_key = ?1"
    } else {
        ""
    };
    let readings_sql = format!(
        "SELECT reading_id, astronaut_key, astronaut_name, indicator, value_json, unit,
                observed_at, batch_id, source, app_version, prev_hash, row_hash
         FROM telemetry_readings {filter} ORDER BY reading_id ASC"
    );
    let heads_sql = format!(
        "SELECT astronaut_key, indicator, last_reading_id, last_hash, count
         FROM telemetry_heads {filter} ORDER BY astronaut_key ASC, indicator ASC"
    );
    let bind: Vec<&str> = astronaut_key.into_iter().collect();

    let mut readings = Vec::new();
    {
        let mut stmt = conn.prepare(&readings_sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(bind.iter()), |r| {
            let value_json: String = r.get(4)?;
            Ok(json!({
                "reading_id": r.get::<_, i64>(0)?,
                "astronaut_key": r.get::<_, String>(1)?,
                "astronaut_name": r.get::<_, String>(2)?,
                "indicator": r.get::<_, String>(3)?,
                // `value` es cómodo de leer; el hash se calcula SIEMPRE sobre los bytes
                // UTF-8 exactos de `value_json`.
                "value": serde_json::from_str::<Value>(&value_json).unwrap_or(Value::Null),
                "value_json": value_json,
                "unit": r.get::<_, String>(5)?,
                "observed_at": ts(r.get(6)?),
                "observed_at_ms": r.get::<_, i64>(6)?,
                "batch_id": r.get::<_, String>(7)?,
                "source": r.get::<_, String>(8)?,
                "app_version": r.get::<_, String>(9)?,
                "prev_hash": r.get::<_, String>(10)?,
                "row_hash": r.get::<_, String>(11)?,
            }))
        })?;
        for row in rows {
            readings.push(row?);
        }
    }

    let mut heads = Vec::new();
    {
        let mut stmt = conn.prepare(&heads_sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(bind.iter()), |r| {
            Ok(json!({
                "astronaut_key": r.get::<_, String>(0)?,
                "indicator": r.get::<_, String>(1)?,
                "last_reading_id": r.get::<_, i64>(2)?,
                "last_hash": r.get::<_, String>(3)?,
                "count": r.get::<_, i64>(4)?,
            }))
        })?;
        for row in rows {
            heads.push(row?);
        }
    }

    let verification = verify_telemetry_in(conn, astronaut_key.map(str::to_owned))?;
    let n = count(readings.len());
    Ok((
        json!({
            "readings": readings,
            "heads": heads,
            "verification": verification,
        }),
        n,
    ))
}

impl Store {
    /// Construye el documento de export y sus conteos en un único snapshot de lectura.
    pub fn export_snapshot(
        &self,
        input: &ExportInput,
        now: Timestamp,
        app_version: &str,
    ) -> Result<(Value, ExportCounts), StoreError> {
        if let Some(key) = &input.astronaut_key {
            validate_astronaut_key(key, "astronaut_key")?;
        }
        let include_telemetry = input.include_telemetry.unwrap_or(true);

        let tx = self.conn.unchecked_transaction()?;
        let profile = export_profile(&tx)?;
        let records = export_records(&tx)?;
        let events = export_events(&tx)?;
        let evaluations = export_evaluations(&tx)?;
        let (outbox, outbox_pending) = export_outbox(&tx)?;
        let pulled = export_pulled_changes(&tx)?;
        let sync_state = export_sync_state(&tx)?;
        let (telemetry, telemetry_readings) = if include_telemetry {
            export_telemetry(&tx, input.astronaut_key.as_deref())?
        } else {
            (Value::Null, 0)
        };
        tx.commit()?;

        let counts = ExportCounts {
            records: count(records.len()),
            events: count(events.len()),
            evaluations: count(evaluations.len()),
            telemetry_readings,
            outbox_pending,
        };
        let doc = json!({
            "format": "merken-export",
            "format_version": EXPORT_FORMAT_VERSION,
            "exported_at": now.to_rfc3339(),
            "app_version": app_version,
            "core_version": pulsys_core::VERSION,
            "rules_version": pulsys_core::RULES_VERSION,
            "scope": {
                "include_telemetry": include_telemetry,
                "astronaut_key": input.astronaut_key,
            },
            "profile": profile,
            "records": records,
            "events": events,
            "evaluations": evaluations,
            "outbox": outbox,
            "pulled_changes": pulled,
            "sync_state": sync_state,
            "telemetry": telemetry,
        });
        Ok((doc, counts))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::{
        Scenario,
        api::{CorrectRecordInput, LogRecordInput, LogResult, RaiseEventInput},
        telemetry::{RecordTelemetryInput, TelemetryReadingInput},
    };
    use pulsys_core::{catalog::Indicator, record::EventKind};
    use sha2::{Digest, Sha256};

    const V: &str = "0.1.0";

    fn t(ms: i64) -> Timestamp {
        Timestamp::from_millis(1_800_000_000_000 + ms)
    }

    fn reading(key: &str, indicator: &str, value: Value, unit: &str) -> TelemetryReadingInput {
        TelemetryReadingInput {
            astronaut_key: key.to_owned(),
            astronaut_name: format!("Astronaut {key}"),
            indicator: indicator.to_owned(),
            value,
            unit: unit.to_owned(),
        }
    }

    fn telemetry(store: &mut Store, at: i64, readings: Vec<TelemetryReadingInput>) {
        store
            .record_telemetry(RecordTelemetryInput { readings }, t(at), V)
            .unwrap();
    }

    fn log(store: &mut Store, value: Value, at: i64) -> String {
        match store
            .log_record(
                LogRecordInput {
                    indicator: Indicator::HeartRate,
                    value,
                    measured_at: Some(t(at)),
                    source: None,
                    note: None,
                    confirm_warnings: true,
                },
                t(at),
                V,
            )
            .unwrap()
        {
            LogResult::Saved { record, .. } => record.record_id,
            LogResult::NeedsConfirmation { .. } => panic!("expected saved"),
        }
    }

    fn mixed_store() -> Store {
        let mut store = Store::open_in_memory().unwrap();
        let first = log(&mut store, json!({"bpm": 72}), 0);
        log(&mut store, json!({"bpm": 75}), 1_000);
        store
            .correct_record(
                CorrectRecordInput {
                    replaces: first,
                    value: json!({"bpm": 70}),
                    measured_at: None,
                    note: Some("typo".into()),
                    confirm_warnings: true,
                },
                t(2_000),
                V,
            )
            .unwrap();
        store
            .raise_event(
                RaiseEventInput {
                    kind: EventKind::FeelingReport,
                    message: Some("dizzy".into()),
                    body_zone: None,
                    occurred_at: Some(t(3_000)),
                },
                t(3_000),
                V,
            )
            .unwrap();
        telemetry(
            &mut store,
            10_000,
            vec![
                reading("1", "heart_rate", json!({"bpm": 70}), "bpm"),
                reading("2", "heart_rate", json!({"bpm": 80}), "bpm"),
                reading("2", "oxygen_in_blood", json!({"percentage": 97}), "%"),
            ],
        );
        telemetry(
            &mut store,
            80_000,
            vec![
                reading("1", "heart_rate", json!({"bpm": 71}), "bpm"),
                reading("2", "heart_rate", json!({"bpm": 82}), "bpm"),
            ],
        );
        store
    }

    fn export(store: &Store, input: &ExportInput) -> (Value, ExportCounts) {
        store.export_snapshot(input, t(100_000), V).unwrap()
    }

    /// Recalcula el hash SOLO con el algoritmo documentado y el JSON exportado.
    fn hash_from_json(r: &Value) -> String {
        let mut h = Sha256::new();
        h.update(b"merken-telemetry-v1");
        let mut feed = |b: &[u8]| {
            h.update((b.len() as u64).to_be_bytes());
            h.update(b);
        };
        feed(r["astronaut_key"].as_str().unwrap().as_bytes());
        feed(r["indicator"].as_str().unwrap().as_bytes());
        feed(r["value_json"].as_str().unwrap().as_bytes());
        feed(r["unit"].as_str().unwrap().as_bytes());
        feed(&r["observed_at_ms"].as_i64().unwrap().to_be_bytes());
        feed(r["batch_id"].as_str().unwrap().as_bytes());
        feed(r["source"].as_str().unwrap().as_bytes());
        feed(r["app_version"].as_str().unwrap().as_bytes());
        feed(r["prev_hash"].as_str().unwrap().as_bytes());
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Recorre las lecturas del JSON y devuelve el primer `reading_id` malo.
    fn first_bad(doc: &Value) -> Option<i64> {
        let genesis = "0".repeat(64);
        let mut last: std::collections::BTreeMap<(String, String), String> =
            std::collections::BTreeMap::new();
        for r in doc["telemetry"]["readings"].as_array().unwrap() {
            let key = (
                r["astronaut_key"].as_str().unwrap().to_owned(),
                r["indicator"].as_str().unwrap().to_owned(),
            );
            let expected_prev = last.get(&key).cloned().unwrap_or_else(|| genesis.clone());
            let id = r["reading_id"].as_i64().unwrap();
            if r["prev_hash"].as_str().unwrap() != expected_prev
                || r["row_hash"].as_str().unwrap() != hash_from_json(r)
            {
                return Some(id);
            }
            last.insert(key, r["row_hash"].as_str().unwrap().to_owned());
        }
        None
    }

    #[test]
    fn export_has_expected_keys_and_counts() {
        let store = mixed_store();
        let (doc, counts) = export(&store, &ExportInput::default());
        assert_eq!(doc["format"], "merken-export");
        assert_eq!(doc["format_version"], 1);
        assert_eq!(doc["exported_at"], t(100_000).to_rfc3339());
        assert_eq!(
            doc["scope"],
            json!({"include_telemetry": true, "astronaut_key": null})
        );
        for key in [
            "app_version",
            "core_version",
            "rules_version",
            "profile",
            "records",
            "events",
            "evaluations",
            "outbox",
            "pulled_changes",
            "sync_state",
            "telemetry",
        ] {
            assert!(doc.get(key).is_some(), "missing key {key}");
        }
        assert_eq!(counts.records, 3);
        assert_eq!(counts.events, 1);
        assert_eq!(counts.telemetry_readings, 5);
        assert_eq!(doc["records"].as_array().unwrap().len(), 3);
        assert_eq!(doc["events"].as_array().unwrap().len(), 1);
        assert_eq!(
            doc["evaluations"].as_array().unwrap().len(),
            counts.evaluations as usize
        );
        let pending = doc["outbox"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["state"] == "pending")
            .count();
        assert_eq!(counts.outbox_pending as usize, pending);
        assert!(pending > 0);
        // La corrección apunta a su original y los valores salen ya parseados.
        let records = doc["records"].as_array().unwrap();
        assert_eq!(records[2]["replaces"], records[0]["record_id"]);
        assert_eq!(records[0]["value"]["bpm"], 72.0);
        assert_eq!(records[0]["sync"], "pending");
        assert!(records[0]["measured_at"].as_str().unwrap().ends_with('Z'));
        assert!(records[0]["note"].is_null());
    }

    #[test]
    fn telemetry_chain_can_be_recomputed_from_the_json_alone() {
        let store = mixed_store();
        let (doc, _) = export(&store, &ExportInput::default());
        assert_eq!(first_bad(&doc), None);
        assert_eq!(doc["telemetry"]["verification"]["ok"], true);
        // `heads` coincide con la última lectura de cada serie.
        let readings = doc["telemetry"]["readings"].as_array().unwrap();
        let heads = doc["telemetry"]["heads"].as_array().unwrap();
        assert_eq!(heads.len(), 3);
        for h in heads {
            let series: Vec<&Value> = readings
                .iter()
                .filter(|r| {
                    r["astronaut_key"] == h["astronaut_key"] && r["indicator"] == h["indicator"]
                })
                .collect();
            let last = series.last().unwrap();
            assert_eq!(last["reading_id"], h["last_reading_id"]);
            assert_eq!(last["row_hash"], h["last_hash"]);
            assert_eq!(series.len() as i64, h["count"].as_i64().unwrap());
        }
    }

    #[test]
    fn tampering_with_the_exported_json_is_detected() {
        let store = mixed_store();
        let (mut doc, _) = export(&store, &ExportInput::default());
        let target = doc["telemetry"]["readings"][1]["reading_id"]
            .as_i64()
            .unwrap();
        doc["telemetry"]["readings"][1]["value_json"] = json!("{\"bpm\":999.0}");
        assert_eq!(first_bad(&doc), Some(target));
        // Cambiar la unidad de otra lectura también se detecta.
        let (mut doc, _) = export(&store, &ExportInput::default());
        doc["telemetry"]["readings"][4]["unit"] = json!("x");
        assert!(first_bad(&doc).is_some());
    }

    #[test]
    fn include_telemetry_false_and_astronaut_filter() {
        let store = mixed_store();
        let (doc, counts) = export(
            &store,
            &ExportInput {
                include_telemetry: Some(false),
                astronaut_key: None,
            },
        );
        assert!(doc["telemetry"].is_null());
        assert_eq!(counts.telemetry_readings, 0);
        assert_eq!(doc["scope"]["include_telemetry"], false);

        let (doc, counts) = export(
            &store,
            &ExportInput {
                include_telemetry: None,
                astronaut_key: Some("2".into()),
            },
        );
        assert_eq!(counts.telemetry_readings, 3);
        assert_eq!(counts.records, 3, "el filtro solo afecta a la telemetría");
        assert!(
            doc["telemetry"]["readings"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["astronaut_key"] == "2")
        );
        assert_eq!(doc["telemetry"]["heads"].as_array().unwrap().len(), 2);
        assert_eq!(doc["telemetry"]["verification"]["ok"], true);
        assert_eq!(first_bad(&doc), None);
        assert_eq!(doc["scope"]["astronaut_key"], "2");

        let err = store
            .export_snapshot(
                &ExportInput {
                    include_telemetry: None,
                    astronaut_key: Some("  ".into()),
                },
                t(0),
                V,
            )
            .unwrap_err();
        assert_eq!(err.code, "invalid_value");
    }

    #[test]
    fn empty_database_exports_empty_lists() {
        let store = Store::open_in_memory().unwrap();
        let (doc, counts) = export(&store, &ExportInput::default());
        assert_eq!(counts, ExportCounts::default());
        assert!(doc["profile"].is_null());
        for key in [
            "records",
            "events",
            "evaluations",
            "outbox",
            "pulled_changes",
        ] {
            assert_eq!(doc[key], json!([]), "{key}");
        }
        assert_eq!(doc["sync_state"], json!({}));
        assert_eq!(doc["telemetry"]["readings"], json!([]));
        assert_eq!(doc["telemetry"]["heads"], json!([]));
        assert_eq!(doc["telemetry"]["verification"]["ok"], true);
    }

    #[test]
    fn export_sync_state_omits_internal_keys() -> Result<(), Box<dyn std::error::Error>> {
        let store = Store::open_in_memory()?;
        let internal_keys: i64 = store.conn.query_row(
            "SELECT count(*) FROM sync_state WHERE key IN ('pulse_store_id', 'demo_generation')",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(internal_keys, 2);
        store.conn.execute_batch(
            "INSERT INTO sync_state(key, value) VALUES
                ('pull_cursor', 'test-cursor'),
                ('last_pull_at', '2027-01-15T08:00:00.000Z'),
                ('last_push_at', '2027-01-15T08:10:00.000Z');",
        )?;

        let (doc, _) = export(&store, &ExportInput::default());
        assert_eq!(
            doc["sync_state"],
            json!({
                "pull_cursor": "test-cursor",
                "last_pull_at": "2027-01-15T08:00:00.000Z",
                "last_push_at": "2027-01-15T08:10:00.000Z",
            })
        );
        Ok(())
    }

    #[test]
    fn demo_data_is_marked_local_only() {
        let mut store = Store::open_in_memory().unwrap();
        store.load_demo(Scenario::Routine, t(0), V).unwrap();
        let (doc, counts) = export(&store, &ExportInput::default());
        assert!(counts.records > 0);
        assert!(
            doc["records"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["is_demo"] == true && r["sync"] == "local_only")
        );
        assert_eq!(doc["profile"]["is_demo"], true);
    }

    #[test]
    fn export_does_not_modify_the_database() {
        let store = mixed_store();
        let (a, _) = export(&store, &ExportInput::default());
        let (b, _) = export(&store, &ExportInput::default());
        assert_eq!(a, b);
    }
}
