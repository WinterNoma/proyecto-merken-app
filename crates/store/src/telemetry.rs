//! Telemetría local append-only con trazabilidad criptográfica.
//!
//! Guarda las lecturas que recibe la interfaz desde la API de desarrollo
//! (`/api/astronauts`) en una tabla separada (`telemetry_readings`), por
//! astronauta e indicador, encadenadas con SHA-256. Nunca escribe en
//! `records`, `events`, `evaluations` ni `outbox`, ni entra en `clear_demo`.

use crate::{Store, StoreError};
use merken_core::{catalog::Indicator, time::Timestamp};
use rusqlite::{OptionalExtension, params, params_from_iter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write as _;

const TELEMETRY_SOURCE: &str = "live_api";
const GENESIS_PREV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_BATCH_READINGS: usize = 64;
const MAX_ASTRONAUT_KEY_CHARS: usize = 64;
const MAX_ASTRONAUT_NAME_CHARS: usize = 100;
const MAX_UNIT_CHARS: usize = 16;
const MAX_VALUE_JSON_BYTES: usize = 2048;
const MIN_REPEAT_INTERVAL_MS: i64 = 60_000;
const DEFAULT_LIST_LIMIT: u32 = 200;
const MAX_LIST_LIMIT: u32 = 1000;

/// Lista cerrada de indicadores admitidos en la telemetría local.
pub const ALLOWED_INDICATORS: [&str; 12] = [
    Indicator::HeartRate.as_str(),
    Indicator::OxygenInBlood.as_str(),
    Indicator::BloodPressure.as_str(),
    Indicator::Glucose.as_str(),
    Indicator::BodyTemperature.as_str(),
    Indicator::Sleep.as_str(),
    Indicator::Nutrition.as_str(),
    Indicator::SpaceRadiation.as_str(),
    Indicator::DistanceFromEarth.as_str(),
    Indicator::Mood.as_str(),
    "hrv",
    "body_battery",
];

/// Lectura individual enviada por la interfaz para registrar en SQLite.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TelemetryReadingInput {
    pub astronaut_key: String,
    pub astronaut_name: String,
    pub indicator: String,
    pub value: Value,
    pub unit: String,
}

/// Lote de lecturas de telemetría (entre 1 y 64 elementos).
#[derive(Clone, Debug, Deserialize)]
pub struct RecordTelemetryInput {
    pub readings: Vec<TelemetryReadingInput>,
}

/// Resultado de procesar un lote de telemetría local.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecordTelemetryResult {
    pub stored: u32,
    pub skipped: u32,
    pub batch_id: String,
}

/// Filtros para consultar lecturas almacenadas en `telemetry_readings`.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ListTelemetryInput {
    pub astronaut_key: String,
    pub indicator: Option<String>,
    pub from: Option<Timestamp>,
    pub to: Option<Timestamp>,
    pub limit: Option<u32>,
}

/// Registro almacenado de telemetría local con sus metadatos de trazabilidad.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TelemetryItem {
    pub reading_id: i64,
    pub astronaut_key: String,
    pub astronaut_name: String,
    pub indicator: String,
    pub value: Value,
    pub unit: String,
    pub observed_at: Timestamp,
    pub batch_id: String,
    pub source: String,
    pub app_version: String,
    pub prev_hash: String,
    pub row_hash: String,
}

/// Resultado de comprobar la integridad de la cadena de hashes local.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VerifyTelemetryResult {
    pub ok: bool,
    pub checked: u32,
    pub first_bad_reading_id: Option<i64>,
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> StoreError {
    StoreError {
        code: "invalid_value".into(),
        field: Some(field.into()),
        message: message.into(),
    }
}

fn validate_astronaut_key(key: &str, field: &str) -> Result<(), StoreError> {
    let chars = key.chars().count();
    if chars == 0 || chars > MAX_ASTRONAUT_KEY_CHARS || key.trim().is_empty() {
        return Err(invalid(
            field,
            "Astronaut key must contain between 1 and 64 characters",
        ));
    }
    if key.chars().any(char::is_control) {
        return Err(invalid(
            field,
            "Astronaut key must not contain control characters",
        ));
    }
    Ok(())
}

struct PreparedReading<'a> {
    reading: &'a TelemetryReadingInput,
    value_json: String,
}

fn validate_reading<'a>(
    idx: usize,
    reading: &'a TelemetryReadingInput,
) -> Result<PreparedReading<'a>, StoreError> {
    validate_astronaut_key(
        &reading.astronaut_key,
        &format!("readings[{idx}].astronaut_key"),
    )?;

    if reading.astronaut_name.chars().count() > MAX_ASTRONAUT_NAME_CHARS
        || reading.astronaut_name.chars().any(char::is_control)
    {
        return Err(invalid(
            format!("readings[{idx}].astronaut_name"),
            "Astronaut name must contain at most 100 non-control characters",
        ));
    }

    if !ALLOWED_INDICATORS.contains(&reading.indicator.as_str()) {
        return Err(invalid(
            format!("readings[{idx}].indicator"),
            "Indicator is not in the allowed telemetry catalog",
        ));
    }

    if reading.unit.chars().count() > MAX_UNIT_CHARS || reading.unit.chars().any(char::is_control) {
        return Err(invalid(
            format!("readings[{idx}].unit"),
            "Unit must contain at most 16 non-control characters",
        ));
    }

    if reading.value.is_null() {
        return Err(invalid(
            format!("readings[{idx}].value"),
            "Telemetry value must not be null",
        ));
    }

    let value_json = serde_json::to_string(&reading.value)?;
    if value_json.len() > MAX_VALUE_JSON_BYTES {
        return Err(invalid(
            format!("readings[{idx}].value"),
            "Telemetry value JSON exceeds 2048 bytes",
        ));
    }

    Ok(PreparedReading {
        reading,
        value_json,
    })
}

struct HashFields<'a> {
    astronaut_key: &'a str,
    indicator: &'a str,
    value_json: &'a str,
    unit: &'a str,
    observed_at: i64,
    batch_id: &'a str,
    source: &'a str,
    app_version: &'a str,
    prev_hash: &'a str,
}

fn feed_field(hasher: &mut Sha256, bytes: &[u8]) {
    let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    hasher.update(len.to_be_bytes());
    hasher.update(bytes);
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Calcula el SHA-256 hexadecimal prefijando cada campo con su longitud en 64 bits
/// big-endian para evitar cualquier ambigüedad entre límites de campos.
fn compute_row_hash(f: &HashFields<'_>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"merken-telemetry-v1");
    feed_field(&mut hasher, f.astronaut_key.as_bytes());
    feed_field(&mut hasher, f.indicator.as_bytes());
    feed_field(&mut hasher, f.value_json.as_bytes());
    feed_field(&mut hasher, f.unit.as_bytes());
    feed_field(&mut hasher, &f.observed_at.to_be_bytes());
    feed_field(&mut hasher, f.batch_id.as_bytes());
    feed_field(&mut hasher, f.source.as_bytes());
    feed_field(&mut hasher, f.app_version.as_bytes());
    feed_field(&mut hasher, f.prev_hash.as_bytes());
    to_hex(&hasher.finalize())
}

impl Store {
    /// Guarda un lote atómico (1..=64) de lecturas en `telemetry_readings`.
    ///
    /// Solo valida forma (sin reglas médicas ni evaluación). Dentro de la misma
    /// transacción omite (`skipped`) una lectura si su valor coincide con la
    /// última guardada para esa serie `(astronaut_key, indicator)` y han pasado
    /// menos de 60 000 ms.
    pub fn record_telemetry(
        &mut self,
        input: RecordTelemetryInput,
        now: Timestamp,
        app_version: &str,
    ) -> Result<RecordTelemetryResult, StoreError> {
        if input.readings.is_empty() || input.readings.len() > MAX_BATCH_READINGS {
            return Err(invalid(
                "readings",
                "Telemetry batch must contain between 1 and 64 readings",
            ));
        }

        let mut prepared = Vec::with_capacity(input.readings.len());
        for (idx, reading) in input.readings.iter().enumerate() {
            prepared.push(validate_reading(idx, reading)?);
        }

        let batch_id = self.new_id(now)?;
        let observed_at = now.millis();
        let tx = self.conn.transaction()?;

        let mut stored: u32 = 0;
        let mut skipped: u32 = 0;

        {
            let mut last_stmt = tx.prepare_cached(
                "SELECT value_json, unit, observed_at, row_hash
                 FROM telemetry_readings
                 WHERE astronaut_key = ?1 AND indicator = ?2
                 ORDER BY reading_id DESC
                 LIMIT 1",
            )?;
            let mut insert_stmt = tx.prepare_cached(
                "INSERT INTO telemetry_readings (
                     astronaut_key, astronaut_name, indicator, value_json, unit,
                     observed_at, batch_id, source, app_version, prev_hash, row_hash
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;

            for item in prepared {
                let r = item.reading;
                let previous: Option<(String, String, i64, String)> = last_stmt
                    .query_row(params![r.astronaut_key, r.indicator], |row| {
                        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                    })
                    .optional()?;

                let prev_hash = match previous {
                    Some((last_value_json, last_unit, last_observed_at, last_row_hash)) => {
                        let same_value = serde_json::from_str::<Value>(&last_value_json)
                            .ok()
                            .as_ref()
                            == Some(&r.value)
                            && last_unit == r.unit;
                        let elapsed_ms = i128::from(observed_at) - i128::from(last_observed_at);
                        if same_value && elapsed_ms < i128::from(MIN_REPEAT_INTERVAL_MS) {
                            skipped = skipped.saturating_add(1);
                            continue;
                        }
                        last_row_hash
                    }
                    None => GENESIS_PREV_HASH.to_owned(),
                };

                let row_hash = compute_row_hash(&HashFields {
                    astronaut_key: &r.astronaut_key,
                    indicator: &r.indicator,
                    value_json: &item.value_json,
                    unit: &r.unit,
                    observed_at,
                    batch_id: &batch_id,
                    source: TELEMETRY_SOURCE,
                    app_version,
                    prev_hash: &prev_hash,
                });

                insert_stmt.execute(params![
                    r.astronaut_key,
                    r.astronaut_name,
                    r.indicator,
                    item.value_json,
                    r.unit,
                    observed_at,
                    batch_id,
                    TELEMETRY_SOURCE,
                    app_version,
                    prev_hash,
                    row_hash,
                ])?;
                stored = stored.saturating_add(1);
            }
        }

        tx.commit()?;
        Ok(RecordTelemetryResult {
            stored,
            skipped,
            batch_id,
        })
    }

    /// Lista las lecturas locales de un astronauta, de la más reciente a la más antigua.
    pub fn list_telemetry(
        &self,
        input: ListTelemetryInput,
    ) -> Result<Vec<TelemetryItem>, StoreError> {
        validate_astronaut_key(&input.astronaut_key, "astronaut_key")?;
        let limit = i64::from(
            input
                .limit
                .unwrap_or(DEFAULT_LIST_LIMIT)
                .clamp(1, MAX_LIST_LIMIT),
        );

        let mut sql = String::from(
            "SELECT reading_id, astronaut_key, astronaut_name, indicator, value_json, unit,
                    observed_at, batch_id, source, app_version, prev_hash, row_hash
             FROM telemetry_readings
             WHERE astronaut_key = ?1",
        );
        let mut sql_params: Vec<rusqlite::types::Value> = vec![input.astronaut_key.into()];
        let mut next_idx = 2_usize;

        if let Some(indicator) = input.indicator {
            let _ = write!(sql, " AND indicator = ?{next_idx}");
            sql_params.push(indicator.into());
            next_idx += 1;
        }
        if let Some(from) = input.from {
            let _ = write!(sql, " AND observed_at >= ?{next_idx}");
            sql_params.push(from.millis().into());
            next_idx += 1;
        }
        if let Some(to) = input.to {
            let _ = write!(sql, " AND observed_at <= ?{next_idx}");
            sql_params.push(to.millis().into());
            next_idx += 1;
        }
        let _ = write!(
            sql,
            " ORDER BY observed_at DESC, reading_id DESC LIMIT ?{next_idx}"
        );
        sql_params.push(limit.into());

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(sql_params), |row| {
            Ok((
                (
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ),
                (
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                ),
            ))
        })?;

        let mut items = Vec::new();
        for row in rows {
            let (
                (reading_id, astronaut_key, astronaut_name, indicator, value_json, unit),
                (observed_at, batch_id, source, app_version, prev_hash, row_hash),
            ) = row?;
            let value = serde_json::from_str::<Value>(&value_json)?;
            items.push(TelemetryItem {
                reading_id,
                astronaut_key,
                astronaut_name,
                indicator,
                value,
                unit,
                observed_at: Timestamp::from_millis(observed_at),
                batch_id,
                source,
                app_version,
                prev_hash,
                row_hash,
            });
        }
        Ok(items)
    }

    /// Recorre las series `(astronaut_key, indicator)` en orden de `reading_id`,
    /// recalcula cada `row_hash` y comprueba la continuidad de `prev_hash`.
    pub fn verify_telemetry(
        &self,
        astronaut_key: Option<String>,
    ) -> Result<VerifyTelemetryResult, StoreError> {
        if let Some(key) = &astronaut_key {
            validate_astronaut_key(key, "astronaut_key")?;
        }

        let (sql, sql_params): (&str, Vec<rusqlite::types::Value>) = match astronaut_key {
            Some(key) => (
                "SELECT reading_id, astronaut_key, indicator, value_json, unit,
                        observed_at, batch_id, source, app_version, prev_hash, row_hash
                 FROM telemetry_readings
                 WHERE astronaut_key = ?1
                 ORDER BY reading_id ASC",
                vec![key.into()],
            ),
            None => (
                "SELECT reading_id, astronaut_key, indicator, value_json, unit,
                        observed_at, batch_id, source, app_version, prev_hash, row_hash
                 FROM telemetry_readings
                 ORDER BY reading_id ASC",
                Vec::new(),
            ),
        };

        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params_from_iter(sql_params), |row| {
            Ok((
                (
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ),
                (
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                ),
            ))
        })?;

        let mut series_tail: HashMap<(String, String), String> = HashMap::new();
        let mut checked: u32 = 0;
        let mut first_bad_reading_id: Option<i64> = None;

        for row in rows {
            let (
                (reading_id, astronaut_key, indicator, value_json, unit),
                (observed_at, batch_id, source, app_version, prev_hash, row_hash),
            ) = row?;
            checked = checked.saturating_add(1);

            let key = (astronaut_key.clone(), indicator.clone());
            let expected_prev = series_tail
                .get(&key)
                .map_or(GENESIS_PREV_HASH, String::as_str);

            let expected_hash = compute_row_hash(&HashFields {
                astronaut_key: &astronaut_key,
                indicator: &indicator,
                value_json: &value_json,
                unit: &unit,
                observed_at,
                batch_id: &batch_id,
                source: &source,
                app_version: &app_version,
                prev_hash: &prev_hash,
            });

            let valid_row = prev_hash == expected_prev
                && source == TELEMETRY_SOURCE
                && serde_json::from_str::<Value>(&value_json).is_ok()
                && row_hash == expected_hash;

            if !valid_row && first_bad_reading_id.is_none() {
                first_bad_reading_id = Some(reading_id);
            }

            series_tail.insert(key, row_hash);
        }

        Ok(VerifyTelemetryResult {
            ok: first_bad_reading_id.is_none(),
            checked,
            first_bad_reading_id,
        })
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
        api::{ListRecordsInput, LogRecordInput},
    };
    use merken_core::catalog::Indicator;
    use rusqlite::Connection;
    use serde_json::json;

    const V: &str = "0.1.0";

    fn t(ms: i64) -> Timestamp {
        Timestamp::from_millis(1_800_000_000_000 + ms)
    }

    fn reading(
        astronaut_key: &str,
        indicator: &str,
        value: Value,
        unit: &str,
    ) -> TelemetryReadingInput {
        TelemetryReadingInput {
            astronaut_key: astronaut_key.to_owned(),
            astronaut_name: format!("Astronaut {astronaut_key}"),
            indicator: indicator.to_owned(),
            value,
            unit: unit.to_owned(),
        }
    }

    #[test]
    fn insert_list_order_filters_and_max_limit() {
        let mut store = Store::open_in_memory().unwrap();

        // Todos los indicadores de la lista cerrada, incluidos hrv y body_battery.
        let all_twelve = vec![
            reading("a1", "heart_rate", json!({"bpm": 72}), "bpm"),
            reading("a1", "oxygen_in_blood", json!({"percentage": 98.4}), "%"),
            reading(
                "a1",
                "blood_pressure",
                json!({"systolic": 118, "diastolic": 76}),
                "mmHg",
            ),
            reading("a1", "glucose", json!({"mg_dl": 94}), "mg/dL"),
            reading("a1", "body_temperature", json!({"celsius": 36.7}), "°C"),
            reading("a1", "sleep", json!({"hours": 7.5}), "h"),
            reading("a1", "nutrition", json!({"kcal": 2300}), "kcal"),
            reading("a1", "space_radiation", json!({"usv_h": 14.1}), "µSv/h"),
            reading("a1", "distance_from_earth", json!({"km": 418.2}), "km"),
            reading("a1", "mood", json!({"label": "focused"}), ""),
            reading("a1", "hrv", json!({"ms": 62}), "ms"),
            reading("a1", "body_battery", json!({"value": 85}), ""),
        ];
        let res = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: all_twelve,
                },
                t(0),
                V,
            )
            .unwrap();
        assert_eq!(res.stored, 12);
        assert_eq!(res.skipped, 0);
        assert_eq!(res.batch_id.len(), 36);

        // Más lecturas de heart_rate en distintos instantes y otro astronauta.
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "heart_rate", json!({"bpm": 75}), "bpm"),
                        reading("a2", "heart_rate", json!({"bpm": 64}), "bpm"),
                    ],
                },
                t(10_000),
                V,
            )
            .unwrap();
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 78}), "bpm")],
                },
                t(20_000),
                V,
            )
            .unwrap();

        let all_a1 = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "a1".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(all_a1.len(), 14);
        assert_eq!(all_a1[0].source, "live_api");
        assert_eq!(all_a1[0].app_version, V);
        assert!(all_a1.windows(2).all(|w| {
            w[0].observed_at > w[1].observed_at
                || (w[0].observed_at == w[1].observed_at && w[0].reading_id > w[1].reading_id)
        }));

        // Filtro por indicador y rango [from, to].
        let filtered = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "a1".into(),
                indicator: Some("heart_rate".into()),
                from: Some(t(5_000)),
                to: Some(t(20_000)),
                limit: Some(10),
            })
            .unwrap();
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].value, json!({"bpm": 78}));
        assert_eq!(filtered[1].value, json!({"bpm": 75}));

        // Límite por defecto (200) y máximo (1000).
        for batch in 0_i64..20 {
            let mut readings = Vec::with_capacity(55);
            for j in 0_i64..55 {
                let bpm = 60 + batch * 55 + j;
                readings.push(reading("bulk", "heart_rate", json!({"bpm": bpm}), "bpm"));
            }
            store
                .record_telemetry(
                    RecordTelemetryInput { readings },
                    t(100_000 + batch * 100),
                    V,
                )
                .unwrap();
        }
        let default_list = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "bulk".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(default_list.len(), 200);

        let capped = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "bulk".into(),
                limit: Some(5000),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(capped.len(), 1000);

        let clamped_min = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "bulk".into(),
                limit: Some(0),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(clamped_min.len(), 1);
    }

    #[test]
    fn frequency_deduplication_60s_rule() {
        let mut store = Store::open_in_memory().unwrap();

        let first = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 72}), "bpm")],
                },
                t(0),
                V,
            )
            .unwrap();
        assert_eq!((first.stored, first.skipped), (1, 0));

        // Igual dentro de 60 s (a los 2 s y a los 59 999 ms) → se omite.
        let skip_2s = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 72}), "bpm")],
                },
                t(2_000),
                V,
            )
            .unwrap();
        assert_eq!((skip_2s.stored, skip_2s.skipped), (0, 1));

        let skip_59s = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 72}), "bpm")],
                },
                t(59_999),
                V,
            )
            .unwrap();
        assert_eq!((skip_59s.stored, skip_59s.skipped), (0, 1));

        // Igual exactamente a los 60 000 ms → se guarda.
        let store_60s = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 72}), "bpm")],
                },
                t(60_000),
                V,
            )
            .unwrap();
        assert_eq!((store_60s.stored, store_60s.skipped), (1, 0));

        // Distinto dentro de 60 s (a los 62 000 ms) → se guarda.
        let store_changed = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 74}), "bpm")],
                },
                t(62_000),
                V,
            )
            .unwrap();
        assert_eq!((store_changed.stored, store_changed.skipped), (1, 0));

        // Duplicado en el mismo lote → el primero se guarda y el segundo se omite.
        let intra_batch = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "heart_rate", json!({"bpm": 80}), "bpm"),
                        reading("a1", "heart_rate", json!({"bpm": 80}), "bpm"),
                    ],
                },
                t(64_000),
                V,
            )
            .unwrap();
        assert_eq!((intra_batch.stored, intra_batch.skipped), (1, 1));

        let listed = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "a1".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(listed.len(), 4);
        assert!(store.verify_telemetry(None).unwrap().ok);
    }

    #[test]
    fn invalid_reading_rejects_whole_batch_atomically_with_indexed_field() {
        let mut store = Store::open_in_memory().unwrap();

        let err = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "heart_rate", json!({"bpm": 70}), "bpm"),
                        reading("a1", "oxygen_in_blood", json!({"percentage": 98}), "%"),
                        reading("a1", "glucose", json!({"mg_dl": 90}), "mg/dL"),
                        reading("a1", "headache", json!({"score": 5}), "score_0_10"),
                    ],
                },
                t(0),
                V,
            )
            .unwrap_err();
        assert_eq!(err.code, "invalid_value");
        assert_eq!(err.field.as_deref(), Some("readings[3].indicator"));

        // Ninguna de las 3 lecturas previas del lote debe haberse guardado.
        assert!(
            store
                .list_telemetry(ListTelemetryInput {
                    astronaut_key: "a1".into(),
                    ..Default::default()
                })
                .unwrap()
                .is_empty()
        );
        assert_eq!(store.verify_telemetry(None).unwrap().checked, 0);

        // Otros errores de forma con su índice correspondiente.
        let empty_err = store
            .record_telemetry(RecordTelemetryInput { readings: vec![] }, t(0), V)
            .unwrap_err();
        assert_eq!(empty_err.code, "invalid_value");
        assert_eq!(empty_err.field.as_deref(), Some("readings"));

        let too_many: Vec<_> = (0..65)
            .map(|i| reading("a1", "heart_rate", json!({"bpm": 60 + i}), "bpm"))
            .collect();
        let batch_err = store
            .record_telemetry(RecordTelemetryInput { readings: too_many }, t(0), V)
            .unwrap_err();
        assert_eq!(batch_err.code, "invalid_value");
        assert_eq!(batch_err.field.as_deref(), Some("readings"));

        let bad_key = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1\n", "heart_rate", json!({"bpm": 70}), "bpm")],
                },
                t(0),
                V,
            )
            .unwrap_err();
        assert_eq!(bad_key.field.as_deref(), Some("readings[0].astronaut_key"));

        let mut big_val = reading("a1", "mood", json!({"label": "x".repeat(2049)}), "");
        big_val.astronaut_name = "Alice".into();
        let bad_val = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![big_val],
                },
                t(0),
                V,
            )
            .unwrap_err();
        assert_eq!(bad_val.field.as_deref(), Some("readings[0].value"));

        let mut bad_unit = reading("a1", "heart_rate", json!({"bpm": 70}), "12345678901234567");
        bad_unit.astronaut_name = "Alice".into();
        let unit_err = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![bad_unit],
                },
                t(0),
                V,
            )
            .unwrap_err();
        assert_eq!(unit_err.field.as_deref(), Some("readings[0].unit"));

        let mut long_name = reading("a1", "heart_rate", json!({"bpm": 70}), "bpm");
        long_name.astronaut_name = "n".repeat(101);
        let name_err = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![long_name],
                },
                t(0),
                V,
            )
            .unwrap_err();
        assert_eq!(
            name_err.field.as_deref(),
            Some("readings[0].astronaut_name")
        );
    }

    #[test]
    fn append_only_triggers_block_update_and_delete() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 72}), "bpm")],
                },
                t(0),
                V,
            )
            .unwrap();

        let update_err = store
            .conn
            .execute(
                "UPDATE telemetry_readings SET value_json = '{\"bpm\":99}' WHERE reading_id = 1",
                [],
            )
            .unwrap_err();
        assert!(
            update_err
                .to_string()
                .contains("telemetry_readings is append-only"),
            "unexpected update error: {update_err}"
        );

        let delete_err = store
            .conn
            .execute("DELETE FROM telemetry_readings WHERE reading_id = 1", [])
            .unwrap_err();
        assert!(
            delete_err
                .to_string()
                .contains("telemetry_readings is append-only"),
            "unexpected delete error: {delete_err}"
        );
    }

    #[test]
    fn clear_demo_load_demo_and_real_writes_do_not_touch_telemetry() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "heart_rate", json!({"bpm": 72}), "bpm"),
                        reading("a1", "hrv", json!({"ms": 58}), "ms"),
                    ],
                },
                t(0),
                V,
            )
            .unwrap();

        store.load_demo(Scenario::Routine, t(1_000), V).unwrap();
        assert_eq!(
            store
                .list_telemetry(ListTelemetryInput {
                    astronaut_key: "a1".into(),
                    ..Default::default()
                })
                .unwrap()
                .len(),
            2
        );

        store.clear_demo().unwrap();
        assert_eq!(
            store
                .list_telemetry(ListTelemetryInput {
                    astronaut_key: "a1".into(),
                    ..Default::default()
                })
                .unwrap()
                .len(),
            2
        );

        store
            .load_demo(Scenario::PersistentChange, t(2_000), V)
            .unwrap();
        // Escritura real borra DEMO en `records`/`profile`, pero conserva `telemetry_readings`.
        store
            .log_record(
                LogRecordInput {
                    indicator: Indicator::HeartRate,
                    value: json!({"bpm": 68}),
                    measured_at: Some(t(3_000)),
                    source: None,
                    note: None,
                    confirm_warnings: false,
                },
                t(3_000),
                V,
            )
            .unwrap();

        assert_eq!(
            store
                .list_records(ListRecordsInput {
                    include_superseded: true,
                    ..Default::default()
                })
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .list_telemetry(ListTelemetryInput {
                    astronaut_key: "a1".into(),
                    ..Default::default()
                })
                .unwrap()
                .len(),
            2
        );
        let verify = store.verify_telemetry(None).unwrap();
        assert!(verify.ok);
        assert_eq!(verify.checked, 2);
    }

    #[test]
    fn verify_detects_tampering_after_dropping_trigger() {
        let mut store = Store::open_in_memory().unwrap();
        for (idx, bpm) in [70, 72, 74].into_iter().enumerate() {
            store
                .record_telemetry(
                    RecordTelemetryInput {
                        readings: vec![reading("a1", "heart_rate", json!({"bpm": bpm}), "bpm")],
                    },
                    t(i64::try_from(idx).unwrap() * 2_000),
                    V,
                )
                .unwrap();
        }

        let ok = store.verify_telemetry(None).unwrap();
        assert_eq!(
            ok,
            VerifyTelemetryResult {
                ok: true,
                checked: 3,
                first_bad_reading_id: None,
            }
        );

        // Desactivar el trigger y alterar la lectura 2 directamente con SQL.
        store
            .conn
            .execute_batch("DROP TRIGGER telemetry_readings_no_update;")
            .unwrap();
        store
            .conn
            .execute(
                "UPDATE telemetry_readings SET value_json = '{\"bpm\":199}' WHERE reading_id = 2",
                [],
            )
            .unwrap();

        let tampered = store.verify_telemetry(Some("a1".into())).unwrap();
        assert_eq!(
            tampered,
            VerifyTelemetryResult {
                ok: false,
                checked: 3,
                first_bad_reading_id: Some(2),
            }
        );

        // Si se elimina la primera fila tras quitar el trigger, se rompe la cadena en la fila 2.
        let mut store2 = Store::open_in_memory().unwrap();
        for idx in 0_i64..2 {
            store2
                .record_telemetry(
                    RecordTelemetryInput {
                        readings: vec![reading(
                            "a1",
                            "heart_rate",
                            json!({"bpm": 70 + idx}),
                            "bpm",
                        )],
                    },
                    t(idx * 2_000),
                    V,
                )
                .unwrap();
        }
        store2
            .conn
            .execute_batch("DROP TRIGGER telemetry_readings_no_delete;")
            .unwrap();
        store2
            .conn
            .execute("DELETE FROM telemetry_readings WHERE reading_id = 1", [])
            .unwrap();
        let broken = store2.verify_telemetry(None).unwrap();
        assert_eq!(
            broken,
            VerifyTelemetryResult {
                ok: false,
                checked: 1,
                first_bad_reading_id: Some(2),
            }
        );
    }

    #[test]
    fn telemetry_never_creates_outbox_rows() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "heart_rate", json!({"bpm": 72}), "bpm"),
                        reading("a1", "body_battery", json!({"value": 80}), ""),
                    ],
                },
                t(0),
                V,
            )
            .unwrap();

        assert!(store.pending_outbox(100).unwrap().is_empty());
        let outbox_rows: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM outbox", [], |r| r.get(0))
            .unwrap();
        let record_rows: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM records", [], |r| r.get(0))
            .unwrap();
        let eval_rows: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM evaluations", [], |r| r.get(0))
            .unwrap();
        assert_eq!((outbox_rows, record_rows, eval_rows), (0, 0, 0));
    }

    #[test]
    fn migration_v3_to_v4_preserves_data_and_reopen_is_idempotent() {
        let path = std::env::temp_dir().join(format!(
            "merken-telemetry-v3-v4-{}-{:?}.sqlite3",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);

        // Construir una base en versión 3 con datos existentes.
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE profile (id INTEGER PRIMARY KEY CHECK(id=1), astronaut_json TEXT NOT NULL, medical_profile_json TEXT NOT NULL, medical_conditions_json TEXT NOT NULL, profile_version INTEGER NOT NULL, is_demo INTEGER NOT NULL, updated_at INTEGER NOT NULL);
                 CREATE TABLE records (local_order INTEGER PRIMARY KEY AUTOINCREMENT, record_id TEXT NOT NULL UNIQUE, indicator TEXT NOT NULL, body_zone TEXT, value_json TEXT NOT NULL, unit TEXT NOT NULL, measured_at INTEGER NOT NULL, logged_at INTEGER NOT NULL, source TEXT NOT NULL, interpreted_by_json TEXT, replaces TEXT, root_id TEXT NOT NULL, note TEXT, seq INTEGER, received_at INTEGER, is_demo INTEGER NOT NULL);
                 CREATE TABLE events (event_id TEXT PRIMARY KEY, kind TEXT NOT NULL, certainty TEXT NOT NULL, message TEXT, body_zone TEXT, priority TEXT NOT NULL, occurred_at INTEGER NOT NULL, logged_at INTEGER NOT NULL, seq INTEGER, received_at INTEGER, is_demo INTEGER NOT NULL);
                 CREATE TABLE evaluations (evaluation_id TEXT PRIMARY KEY, series_key TEXT NOT NULL, payload_json TEXT NOT NULL, evaluated_at INTEGER NOT NULL, is_demo INTEGER NOT NULL);
                 CREATE TABLE outbox (outbox_id INTEGER PRIMARY KEY AUTOINCREMENT, entity TEXT NOT NULL, entity_id TEXT NOT NULL, payload_json TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('pending','synced','failed')), attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT, created_at INTEGER NOT NULL, UNIQUE(entity, entity_id));
                 CREATE TABLE sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE pulled_changes (seq INTEGER PRIMARY KEY, entity TEXT NOT NULL, entity_id TEXT NOT NULL, payload_json TEXT NOT NULL, applied_at INTEGER NOT NULL);
                 INSERT INTO records(record_id, indicator, body_zone, value_json, unit, measured_at, logged_at, source, root_id, is_demo)
                 VALUES ('r-v3', 'heart_rate', NULL, '{\"bpm\":70}', 'bpm', 1800000000000, 1800000000000, 'form', 'r-v3', 0);
                 INSERT INTO outbox(entity, entity_id, payload_json, state, created_at)
                 VALUES ('record', 'r-v3', '{\"record_id\":\"r-v3\"}', 'pending', 1800000000000);
                 INSERT INTO pulled_changes(seq, entity, entity_id, payload_json, applied_at)
                 VALUES (1, 'analysis', 'an-1', '{\"seq\":1}', 1800000000000);
                 PRAGMA user_version=3;",
            )
            .unwrap();
        }

        // Primera apertura: sube a v4 y conserva los datos v3.
        {
            let mut store = Store::open(&path).unwrap();
            let ver: i64 = store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(ver, 4);
            assert_eq!(
                store
                    .list_records(ListRecordsInput::default())
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(store.pending_outbox(10).unwrap().len(), 1);
            let pulled: i64 = store
                .conn
                .query_row("SELECT COUNT(*) FROM pulled_changes", [], |r| r.get(0))
                .unwrap();
            assert_eq!(pulled, 1);

            store
                .record_telemetry(
                    RecordTelemetryInput {
                        readings: vec![reading("a1", "heart_rate", json!({"bpm": 72}), "bpm")],
                    },
                    t(0),
                    V,
                )
                .unwrap();
        }

        // Segunda apertura: idempotente y mantiene tanto v3 como v4.
        {
            let store = Store::open(&path).unwrap();
            let ver: i64 = store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(ver, 4);
            assert_eq!(
                store
                    .list_telemetry(ListTelemetryInput {
                        astronaut_key: "a1".into(),
                        ..Default::default()
                    })
                    .unwrap()
                    .len(),
                1
            );
            assert!(store.verify_telemetry(None).unwrap().ok);
        }

        // Versión futura (5) rechazada con invalid_schema.
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", 5).unwrap();
        }
        let err = match Store::open(&path) {
            Ok(_) => panic!("expected unsupported schema version error"),
            Err(err) => err,
        };
        assert_eq!(err.code, "invalid_schema");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn two_astronauts_same_indicator_have_independent_chains() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("astro-1", "heart_rate", json!({"bpm": 70}), "bpm"),
                        reading("astro-2", "heart_rate", json!({"bpm": 70}), "bpm"),
                    ],
                },
                t(0),
                V,
            )
            .unwrap();
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("astro-1", "heart_rate", json!({"bpm": 75}), "bpm")],
                },
                t(2_000),
                V,
            )
            .unwrap();

        let a1 = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "astro-1".into(),
                ..Default::default()
            })
            .unwrap();
        let a2 = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "astro-2".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(a1.len(), 2);
        assert_eq!(a2.len(), 1);
        assert_eq!(a1[1].prev_hash, GENESIS_PREV_HASH);
        assert_eq!(a2[0].prev_hash, GENESIS_PREV_HASH);
        assert_eq!(a1[0].prev_hash, a1[1].row_hash);
        assert_ne!(a1[0].prev_hash, a2[0].row_hash);

        // Alterar la cadena de astro-1 no invalida la de astro-2.
        store
            .conn
            .execute_batch("DROP TRIGGER telemetry_readings_no_update;")
            .unwrap();
        store
            .conn
            .execute(
                "UPDATE telemetry_readings SET value_json = '{\"bpm\":222}' WHERE reading_id = ?1",
                [a1[0].reading_id],
            )
            .unwrap();

        let v2 = store.verify_telemetry(Some("astro-2".into())).unwrap();
        assert!(v2.ok);
        assert_eq!(v2.checked, 1);

        let v1 = store.verify_telemetry(Some("astro-1".into())).unwrap();
        assert!(!v1.ok);
        assert_eq!(v1.first_bad_reading_id, Some(a1[0].reading_id));
    }

    #[test]
    fn temp_probe_explain_query_plan() {
        let store = Store::open_in_memory().unwrap();
        let cols = "reading_id, astronaut_key, astronaut_name, indicator, value_json, unit, observed_at, batch_id, source, app_version, prev_hash, row_hash";
        let queries: Vec<(&str, String, Vec<rusqlite::types::Value>)> = vec![
            (
                "list_all_old",
                format!("SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 ORDER BY observed_at DESC, reading_id DESC LIMIT ?2"),
                vec!["a1".to_owned().into(), 200_i64.into()],
            ),
            (
                "list_ind_old",
                format!("SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 AND indicator = ?2 ORDER BY observed_at DESC, reading_id DESC LIMIT ?3"),
                vec!["a1".to_owned().into(), "heart_rate".to_owned().into(), 200_i64.into()],
            ),
            (
                "list_all_new_no_idx",
                format!("SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 ORDER BY reading_id DESC LIMIT ?2"),
                vec!["a1".to_owned().into(), 200_i64.into()],
            ),
            (
                "list_ind_new_no_idx",
                format!("SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 AND indicator = ?2 ORDER BY reading_id DESC LIMIT ?3"),
                vec!["a1".to_owned().into(), "heart_rate".to_owned().into(), 200_i64.into()],
            ),
            (
                "list_all_range_no_idx",
                format!("SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 AND observed_at >= ?2 AND observed_at <= ?3 ORDER BY reading_id DESC LIMIT ?4"),
                vec!["a1".to_owned().into(), 100_i64.into(), 1000_i64.into(), 200_i64.into()],
            ),
            (
                "list_ind_range_no_idx",
                format!("SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 AND indicator = ?2 AND observed_at >= ?3 AND observed_at <= ?4 ORDER BY reading_id DESC LIMIT ?5"),
                vec!["a1".to_owned().into(), "heart_rate".to_owned().into(), 100_i64.into(), 1000_i64.into(), 200_i64.into()],
            ),
            (
                "verify_astro_no_idx",
                format!("SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 ORDER BY reading_id ASC"),
                vec!["a1".to_owned().into()],
            ),
            (
                "verify_all",
                format!("SELECT {cols} FROM telemetry_readings ORDER BY reading_id ASC"),
                vec![],
            ),
        ];
        for (name, sql, p) in &queries {
            let eqp = format!("EXPLAIN QUERY PLAN {sql}");
            let mut stmt = store.conn.prepare(&eqp).unwrap();
            let rows = stmt
                .query_map(params_from_iter(p.iter().cloned()), |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })
                .unwrap();
            println!("=== BEFORE {name} ===");
            for row in rows {
                let (id, parent, notused, detail) = row.unwrap();
                println!("  {id}|{parent}|{notused}|{detail}");
            }
        }
        store
            .conn
            .execute_batch(
                "CREATE INDEX IF NOT EXISTS telemetry_readings_astronaut_reading ON telemetry_readings(astronaut_key, reading_id);",
            )
            .unwrap();
        for (name, sql, p) in &queries {
            let eqp = format!("EXPLAIN QUERY PLAN {sql}");
            let mut stmt = store.conn.prepare(&eqp).unwrap();
            let rows = stmt
                .query_map(params_from_iter(p.iter().cloned()), |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })
                .unwrap();
            println!("=== AFTER {name} ===");
            for row in rows {
                let (id, parent, notused, detail) = row.unwrap();
                println!("  {id}|{parent}|{notused}|{detail}");
            }
        }
    }
}
