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
use std::collections::BTreeMap;
use std::fmt::Write as _;

const TELEMETRY_SOURCE: &str = "live_api";
const GENESIS_PREV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_BATCH_READINGS: usize = 64;
const MAX_ASTRONAUT_KEY_CHARS: usize = 64;
const MAX_ASTRONAUT_NAME_CHARS: usize = 100;
const MAX_UNIT_CHARS: usize = 16;
const MAX_MOOD_LABEL_CHARS: usize = 64;
const MAX_VALUE_JSON_BYTES: usize = 2048;
/// Intervalo mínimo (ms) entre lecturas guardadas cuando el valor cambia en una serie.
pub const MIN_CHANGE_INTERVAL_MS: i64 = 10_000;
/// Intervalo (ms) tras el cual se guarda un latido (heartbeat) aunque el valor no cambie.
pub const HEARTBEAT_MS: i64 = 60_000;
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
    pub series_checked: u32,
    pub checked_through_reading_id: Option<i64>,
    pub first_bad_reading_id: Option<i64>,
    pub problem: Option<String>,
}

fn invalid(field: impl Into<String>, message: impl Into<String>) -> StoreError {
    StoreError {
        code: "invalid_value".into(),
        field: Some(field.into()),
        message: message.into(),
    }
}

pub(crate) fn validate_astronaut_key(key: &str, field: &str) -> Result<(), StoreError> {
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

fn expected_numeric_keys(indicator: &str) -> Option<&'static [&'static str]> {
    match indicator {
        "heart_rate" => Some(&["bpm"]),
        "oxygen_in_blood" => Some(&["percentage"]),
        "blood_pressure" => Some(&["systolic", "diastolic"]),
        "glucose" => Some(&["mg_dl"]),
        "body_temperature" => Some(&["celsius"]),
        "sleep" => Some(&["hours"]),
        "nutrition" => Some(&["kcal"]),
        "space_radiation" => Some(&["usv_h"]),
        "distance_from_earth" => Some(&["km"]),
        "hrv" => Some(&["ms"]),
        "body_battery" => Some(&["value"]),
        _ => None,
    }
}

fn validate_mood_value(idx: usize, map: &serde_json::Map<String, Value>) -> Result<(), StoreError> {
    let label = map.get("label").ok_or_else(|| {
        invalid(
            format!("readings[{idx}].value.label"),
            "Mood telemetry value must contain key 'label'",
        )
    })?;
    if map.len() != 1 {
        return Err(invalid(
            format!("readings[{idx}].value"),
            "Mood telemetry value must contain only key 'label'",
        ));
    }
    let text = label.as_str().ok_or_else(|| {
        invalid(
            format!("readings[{idx}].value.label"),
            "Mood label must be a string",
        )
    })?;
    let chars = text.chars().count();
    if chars == 0 || chars > MAX_MOOD_LABEL_CHARS || text.trim().is_empty() {
        return Err(invalid(
            format!("readings[{idx}].value.label"),
            "Mood label must contain between 1 and 64 non-blank characters",
        ));
    }
    if text.chars().any(char::is_control) {
        return Err(invalid(
            format!("readings[{idx}].value.label"),
            "Mood label must not contain control characters",
        ));
    }
    Ok(())
}

fn validate_value_shape(idx: usize, indicator: &str, value: &Value) -> Result<(), StoreError> {
    let map = value.as_object().ok_or_else(|| {
        invalid(
            format!("readings[{idx}].value"),
            "Telemetry value must be a JSON object",
        )
    })?;

    if indicator == "mood" {
        return validate_mood_value(idx, map);
    }

    let expected = expected_numeric_keys(indicator).ok_or_else(|| {
        invalid(
            format!("readings[{idx}].indicator"),
            "Indicator is not in the allowed telemetry catalog",
        )
    })?;

    for &key in expected {
        let field_val = map.get(key).ok_or_else(|| {
            invalid(
                format!("readings[{idx}].value.{key}"),
                format!("Telemetry value is missing required key '{key}'"),
            )
        })?;
        if !field_val.as_f64().is_some_and(f64::is_finite) {
            return Err(invalid(
                format!("readings[{idx}].value.{key}"),
                format!("Telemetry field '{key}' must be a finite number"),
            ));
        }
    }

    if map.len() != expected.len() {
        return Err(invalid(
            format!("readings[{idx}].value"),
            "Telemetry value contains unexpected extra keys",
        ));
    }

    Ok(())
}

// Comparación numérica exacta por f64 requerida para tratar 72 y 72.0 como iguales.
#[allow(clippy::float_cmp)]
fn numbers_equal(x: &serde_json::Number, y: &serde_json::Number) -> bool {
    match (x.as_f64(), y.as_f64()) {
        (Some(xf), Some(yf)) => xf == yf,
        _ => x == y,
    }
}

/// Igualdad de valores de telemetría: numérica por `f64` y estructural para texto y objetos.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => numbers_equal(x, y),
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(xi, yi)| values_equal(xi, yi))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, xv)| y.get(k).is_some_and(|yv| values_equal(xv, yv)))
        }
        _ => false,
    }
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

    validate_value_shape(idx, &reading.indicator, &reading.value)?;

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

struct SeriesHead {
    last_reading_id: i64,
    last_hash: String,
    count: i64,
}

struct SeriesActual {
    first_reading_id: i64,
    last_reading_id: i64,
    last_hash: String,
    count: i64,
}

fn load_telemetry_heads(
    conn: &rusqlite::Connection,
    astronaut_key: Option<&str>,
) -> Result<BTreeMap<(String, String), SeriesHead>, StoreError> {
    let (sql, sql_params): (&str, Vec<rusqlite::types::Value>) = match astronaut_key {
        Some(key) => (
            "SELECT astronaut_key, indicator, last_reading_id, last_hash, count
             FROM telemetry_heads
             WHERE astronaut_key = ?1
             ORDER BY astronaut_key ASC, indicator ASC",
            vec![key.to_owned().into()],
        ),
        None => (
            "SELECT astronaut_key, indicator, last_reading_id, last_hash, count
             FROM telemetry_heads
             ORDER BY astronaut_key ASC, indicator ASC",
            Vec::new(),
        ),
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params_from_iter(sql_params), |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;

    let mut heads = BTreeMap::new();
    for row in rows {
        let (astro, indicator, last_reading_id, last_hash, count) = row?;
        heads.insert(
            (astro, indicator),
            SeriesHead {
                last_reading_id,
                last_hash,
                count,
            },
        );
    }
    Ok(heads)
}

fn check_series_against_heads(
    heads: &BTreeMap<(String, String), SeriesHead>,
    actual_series: &BTreeMap<(String, String), SeriesActual>,
    first_problem: &mut Option<(Option<i64>, &'static str)>,
) -> u32 {
    let mut series_count = heads.len();

    for (key, head) in heads {
        let issue = match actual_series.get(key) {
            None => Some((None, "missing_series")),
            Some(actual) if actual.last_reading_id < head.last_reading_id => {
                Some((Some(head.last_reading_id), "missing_tail"))
            }
            Some(actual) if actual.last_reading_id > head.last_reading_id => {
                Some((Some(actual.last_reading_id), "count_mismatch"))
            }
            Some(actual) if actual.last_hash != head.last_hash => {
                Some((Some(actual.last_reading_id), "hash_mismatch"))
            }
            Some(actual) if actual.count != head.count => Some((None, "count_mismatch")),
            Some(_) => None,
        };
        if first_problem.is_none() {
            *first_problem = issue;
        }
    }

    for (key, actual) in actual_series {
        if !heads.contains_key(key) {
            series_count += 1;
            if first_problem.is_none() {
                *first_problem = Some((Some(actual.first_reading_id), "count_mismatch"));
            }
        }
    }

    u32::try_from(series_count).unwrap_or(u32::MAX)
}

impl Store {
    /// Guarda un lote atómico (1..=64) de lecturas en `telemetry_readings` y
    /// actualiza `telemetry_heads` en la misma transacción.
    ///
    /// Valida la forma del valor por indicador (sin reglas médicas). Omite
    /// (`skipped`) una lectura respecto a la última guardada de su serie salvo
    /// si el valor/unidad cambió y pasaron ≥ `MIN_CHANGE_INTERVAL_MS`, o si
    /// pasaron ≥ `HEARTBEAT_MS`.
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
            let mut update_head_stmt = tx.prepare_cached(
                "UPDATE telemetry_heads
                 SET last_reading_id = ?3,
                     last_hash = ?4,
                     count = count + 1
                 WHERE astronaut_key = ?1 AND indicator = ?2",
            )?;
            let mut insert_head_stmt = tx.prepare_cached(
                "INSERT INTO telemetry_heads (
                     astronaut_key, indicator, last_reading_id, last_hash, count
                 ) VALUES (?1, ?2, ?3, ?4, 1)",
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
                            .is_some_and(|last_value| values_equal(&last_value, &r.value))
                            && last_unit == r.unit;
                        let elapsed_ms = i128::from(observed_at) - i128::from(last_observed_at);
                        let should_store = (!same_value
                            && elapsed_ms >= i128::from(MIN_CHANGE_INTERVAL_MS))
                            || elapsed_ms >= i128::from(HEARTBEAT_MS);
                        if !should_store {
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
                let reading_id = tx.last_insert_rowid();
                let updated = update_head_stmt.execute(params![
                    r.astronaut_key,
                    r.indicator,
                    reading_id,
                    row_hash,
                ])?;
                if updated == 0 {
                    insert_head_stmt.execute(params![
                        r.astronaut_key,
                        r.indicator,
                        reading_id,
                        row_hash,
                    ])?;
                }
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

    /// Lista las lecturas locales de un astronauta por orden de inserción descendente (`reading_id DESC`).
    pub fn list_telemetry(
        &self,
        input: ListTelemetryInput,
    ) -> Result<Vec<TelemetryItem>, StoreError> {
        validate_astronaut_key(&input.astronaut_key, "astronaut_key")?;
        if let Some(indicator) = &input.indicator
            && !ALLOWED_INDICATORS.contains(&indicator.as_str())
        {
            return Err(invalid(
                "indicator",
                "Indicator is not in the allowed telemetry catalog",
            ));
        }

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
        let _ = write!(sql, " ORDER BY reading_id DESC LIMIT ?{next_idx}");
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
    /// recalcula cada `row_hash`, comprueba `prev_hash` y contrasta cada serie
    /// con su cabeza en `telemetry_heads`.
    pub fn verify_telemetry(
        &self,
        astronaut_key: Option<String>,
    ) -> Result<VerifyTelemetryResult, StoreError> {
        if let Some(key) = &astronaut_key {
            validate_astronaut_key(key, "astronaut_key")?;
        }
        let tx = self.conn.unchecked_transaction()?;
        let result = verify_telemetry_in(&tx, astronaut_key)?;
        tx.commit()?;
        Ok(result)
    }
}

/// Verifica la cadena usando una conexión o transacción ya abierta, para que el
/// exportador pueda verificar en el mismo snapshot que usa para leer las filas.
pub(crate) fn verify_telemetry_in(
    tx: &rusqlite::Connection,
    astronaut_key: Option<String>,
) -> Result<VerifyTelemetryResult, StoreError> {
    {
        let heads = load_telemetry_heads(tx, astronaut_key.as_deref())?;

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

        let mut stmt = tx.prepare(sql)?;
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

        let mut actual_series: BTreeMap<(String, String), SeriesActual> = BTreeMap::new();
        let mut checked: u32 = 0;
        let mut first_problem: Option<(Option<i64>, &'static str)> = None;

        for row in rows {
            let (
                (reading_id, astronaut_key, indicator, value_json, unit),
                (observed_at, batch_id, source, app_version, prev_hash, row_hash),
            ) = row?;
            checked = checked.saturating_add(1);

            let key = (astronaut_key.clone(), indicator.clone());
            let expected_prev = actual_series
                .get(&key)
                .map_or(GENESIS_PREV_HASH, |s| s.last_hash.as_str());

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

            if first_problem.is_none() {
                if prev_hash != expected_prev {
                    first_problem = Some((Some(reading_id), "broken_link"));
                } else if source != TELEMETRY_SOURCE
                    || serde_json::from_str::<Value>(&value_json).is_err()
                    || row_hash != expected_hash
                {
                    first_problem = Some((Some(reading_id), "hash_mismatch"));
                }
            }

            actual_series
                .entry(key)
                .and_modify(|s| {
                    s.last_reading_id = reading_id;
                    s.last_hash.clone_from(&row_hash);
                    s.count += 1;
                })
                .or_insert(SeriesActual {
                    first_reading_id: reading_id,
                    last_reading_id: reading_id,
                    last_hash: row_hash,
                    count: 1,
                });
        }

        drop(stmt);
        let checked_through_reading_id = actual_series.values().map(|s| s.last_reading_id).max();
        let series_checked = check_series_against_heads(&heads, &actual_series, &mut first_problem);
        let (first_bad_reading_id, problem) = match first_problem {
            Some((id, kind)) => (id, Some(kind.to_owned())),
            None => (None, None),
        };

        Ok(VerifyTelemetryResult {
            ok: problem.is_none(),
            checked,
            series_checked,
            checked_through_reading_id,
            first_bad_reading_id,
            problem,
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
    fn verify_reports_snapshot_high_water_mark() {
        let mut store = Store::open_in_memory().unwrap();
        assert_eq!(
            store
                .verify_telemetry(None)
                .unwrap()
                .checked_through_reading_id,
            None
        );
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "heart_rate", json!({"bpm": 70}), "bpm"),
                        reading("a2", "hrv", json!({"ms": 60}), "ms"),
                    ],
                },
                t(0),
                V,
            )
            .unwrap();
        let previous = store
            .verify_telemetry(None)
            .unwrap()
            .checked_through_reading_id;
        assert_eq!(previous, Some(2));
        assert_eq!(
            store
                .verify_telemetry(Some("a1".into()))
                .unwrap()
                .checked_through_reading_id,
            Some(1)
        );
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 75}), "bpm")],
                },
                t(MIN_CHANGE_INTERVAL_MS),
                V,
            )
            .unwrap();
        let current = store
            .verify_telemetry(None)
            .unwrap()
            .checked_through_reading_id;
        assert_eq!(current, Some(3));
        assert_ne!(previous, current);
        assert_eq!(
            store
                .verify_telemetry(Some("a2".into()))
                .unwrap()
                .checked_through_reading_id,
            Some(2)
        );
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
        assert!(all_a1.windows(2).all(|w| w[0].reading_id > w[1].reading_id));

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

        // Filtro por indicador desconocido → invalid_value con field: "indicator".
        let bad_filter = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "a1".into(),
                indicator: Some("headache".into()),
                ..Default::default()
            })
            .unwrap_err();
        assert_eq!(bad_filter.code, "invalid_value");
        assert_eq!(bad_filter.field.as_deref(), Some("indicator"));

        // Límite por defecto (200) y máximo (1000).
        for batch in 0_i64..92 {
            let step = f64::from(i32::try_from(batch).unwrap());
            let readings = vec![
                reading("bulk", "heart_rate", json!({"bpm": 60 + batch}), "bpm"),
                reading(
                    "bulk",
                    "oxygen_in_blood",
                    json!({"percentage": 95.0 + step * 0.01}),
                    "%",
                ),
                reading(
                    "bulk",
                    "blood_pressure",
                    json!({"systolic": 110 + batch, "diastolic": 70}),
                    "mmHg",
                ),
                reading("bulk", "glucose", json!({"mg_dl": 80 + batch}), "mg/dL"),
                reading(
                    "bulk",
                    "body_temperature",
                    json!({"celsius": 36.0 + step * 0.01}),
                    "°C",
                ),
                reading("bulk", "sleep", json!({"hours": 6.0 + step * 0.01}), "h"),
                reading("bulk", "nutrition", json!({"kcal": 1800 + batch}), "kcal"),
                reading(
                    "bulk",
                    "space_radiation",
                    json!({"usv_h": 10.0 + step * 0.1}),
                    "µSv/h",
                ),
                reading(
                    "bulk",
                    "distance_from_earth",
                    json!({"km": 400.0 + step}),
                    "km",
                ),
                reading(
                    "bulk",
                    "mood",
                    json!({"label": format!("state-{batch}")}),
                    "",
                ),
                reading("bulk", "hrv", json!({"ms": 50 + batch}), "ms"),
                reading(
                    "bulk",
                    "body_battery",
                    json!({"value": 70 + (batch % 30)}),
                    "",
                ),
            ];
            store
                .record_telemetry(
                    RecordTelemetryInput { readings },
                    t(100_000 + batch * MIN_CHANGE_INTERVAL_MS),
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
    fn frequency_deduplication_10s_change_and_60s_heartbeat_rule() {
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

        // Mismo valor dentro de 60 s (a los 2 s) → se omite.
        let skip_same_2s = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 72}), "bpm")],
                },
                t(2_000),
                V,
            )
            .unwrap();
        assert_eq!((skip_same_2s.stored, skip_same_2s.skipped), (0, 1));

        // Valor distinto pero antes de MIN_CHANGE_INTERVAL_MS (5 s y 9 999 ms) → se omite.
        let skip_change_5s = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 73}), "bpm")],
                },
                t(5_000),
                V,
            )
            .unwrap();
        assert_eq!((skip_change_5s.stored, skip_change_5s.skipped), (0, 1));

        let skip_change_9999 = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 73}), "bpm")],
                },
                t(9_999),
                V,
            )
            .unwrap();
        assert_eq!((skip_change_9999.stored, skip_change_9999.skipped), (0, 1));

        // Valor distinto exactamente a los 10 000 ms → se guarda.
        let store_change_10s = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 73}), "bpm")],
                },
                t(10_000),
                V,
            )
            .unwrap();
        assert_eq!((store_change_10s.stored, store_change_10s.skipped), (1, 0));

        // Igualdad numérica por f64: 73.0 cuenta como igual a 73 (< 60 s desde t(10_000)) → se omite.
        let skip_numeric_eq = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 73.0}), "bpm")],
                },
                t(25_000),
                V,
            )
            .unwrap();
        assert_eq!((skip_numeric_eq.stored, skip_numeric_eq.skipped), (0, 1));

        // Nuevo cambio a los 30 000 ms (20 s desde t(10_000)) → se guarda.
        let store_changed = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 74}), "bpm")],
                },
                t(30_000),
                V,
            )
            .unwrap();
        assert_eq!((store_changed.stored, store_changed.skipped), (1, 0));

        // Mismo valor numérico a los 59 999 ms desde t(30_000) → se omite.
        let skip_before_hb = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 74.0}), "bpm")],
                },
                t(89_999),
                V,
            )
            .unwrap();
        assert_eq!((skip_before_hb.stored, skip_before_hb.skipped), (0, 1));

        // Igual exactamente a los HEARTBEAT_MS (60 000 ms desde t(30_000)) → latido guardado.
        let store_hb = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 74.0}), "bpm")],
                },
                t(90_000),
                V,
            )
            .unwrap();
        assert_eq!((store_hb.stored, store_hb.skipped), (1, 0));

        // Dos lecturas de la misma serie en el mismo lote (elapsed=0 ms) → la segunda se omite.
        let intra_batch = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "heart_rate", json!({"bpm": 80}), "bpm"),
                        reading("a1", "heart_rate", json!({"bpm": 81}), "bpm"),
                    ],
                },
                t(100_000),
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
        assert_eq!(listed.len(), 5);
        assert_eq!(
            store.verify_telemetry(None).unwrap(),
            VerifyTelemetryResult {
                ok: true,
                checked: 5,
                series_checked: 1,
                checked_through_reading_id: Some(5),
                first_bad_reading_id: None,
                problem: None,
            }
        );
    }

    #[test]
    fn value_shape_validation_covers_all_indicators() {
        let mut store = Store::open_in_memory().unwrap();

        let bad_cases = [
            (
                reading("a1", "heart_rate", json!(72), "bpm"),
                "readings[0].value",
            ),
            (
                reading("a1", "heart_rate", json!({}), "bpm"),
                "readings[0].value.bpm",
            ),
            (
                reading("a1", "heart_rate", json!({"bpm": 72, "extra": 1}), "bpm"),
                "readings[0].value",
            ),
            (
                reading("a1", "heart_rate", json!({"bpm": "72"}), "bpm"),
                "readings[0].value.bpm",
            ),
            (
                reading("a1", "oxygen_in_blood", json!({"percentage": true}), "%"),
                "readings[0].value.percentage",
            ),
            (
                reading("a1", "blood_pressure", json!({"systolic": 120}), "mmHg"),
                "readings[0].value.diastolic",
            ),
            (
                reading("a1", "space_radiation", json!({"value": 14.1}), "µSv/h"),
                "readings[0].value.usv_h",
            ),
            (
                reading("a1", "space_radiation", json!({"usv_h": "14.1"}), "µSv/h"),
                "readings[0].value.usv_h",
            ),
            (
                reading("a1", "distance_from_earth", json!({"value": 418.2}), "km"),
                "readings[0].value.km",
            ),
            (
                reading(
                    "a1",
                    "distance_from_earth",
                    json!({"km": 418.2, "extra": 1}),
                    "km",
                ),
                "readings[0].value",
            ),
            (
                reading("a1", "mood", json!({"label": ""}), ""),
                "readings[0].value.label",
            ),
            (
                reading("a1", "mood", json!({"label": "   "}), ""),
                "readings[0].value.label",
            ),
            (
                reading("a1", "mood", json!({"label": "x".repeat(65)}), ""),
                "readings[0].value.label",
            ),
            (
                reading("a1", "mood", json!({"label": "calm\n"}), ""),
                "readings[0].value.label",
            ),
            (
                reading("a1", "mood", json!({"label": 123}), ""),
                "readings[0].value.label",
            ),
            (
                reading("a1", "mood", json!({"label": "calm", "extra": 1}), ""),
                "readings[0].value",
            ),
        ];

        for (bad, expected_field) in bad_cases {
            let err = store
                .record_telemetry(
                    RecordTelemetryInput {
                        readings: vec![bad],
                    },
                    t(0),
                    V,
                )
                .unwrap_err();
            assert_eq!(err.code, "invalid_value");
            assert_eq!(err.field.as_deref(), Some(expected_field));
        }

        // Texto de 64 caracteres válido para mood y sin rangos médicos restrictivos.
        let ok_res = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "mood", json!({"label": "x".repeat(64)}), ""),
                        reading("a1", "heart_rate", json!({"bpm": -5.5}), "bpm"),
                    ],
                },
                t(0),
                V,
            )
            .unwrap();
        assert_eq!(ok_res.stored, 2);
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
    fn append_only_triggers_and_replace_protection() {
        let mut store = Store::open_in_memory().unwrap();
        let rec_triggers: i64 = store
            .conn
            .pragma_query_value(None, "recursive_triggers", |r| r.get(0))
            .unwrap();
        assert_eq!(rec_triggers, 1);

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

        // Regresión Hallazgo 2: INSERT OR REPLACE y REPLACE INTO deben abortar.
        let insert_replace_err = store
            .conn
            .execute(
                "INSERT OR REPLACE INTO telemetry_readings (
                     reading_id, astronaut_key, astronaut_name, indicator, value_json, unit,
                     observed_at, batch_id, source, app_version, prev_hash, row_hash
                 ) VALUES (1, 'a1', 'Tampered', 'heart_rate', '{\"bpm\":99}', 'bpm', 1, 'b', 'live_api', '0.1.0', '0', '0')",
                [],
            )
            .unwrap_err();
        assert!(
            insert_replace_err
                .to_string()
                .contains("telemetry_readings is append-only"),
            "unexpected insert or replace error: {insert_replace_err}"
        );

        let replace_into_err = store
            .conn
            .execute(
                "REPLACE INTO telemetry_readings (
                     reading_id, astronaut_key, astronaut_name, indicator, value_json, unit,
                     observed_at, batch_id, source, app_version, prev_hash, row_hash
                 ) VALUES (1, 'a1', 'Tampered', 'heart_rate', '{\"bpm\":99}', 'bpm', 1, 'b', 'live_api', '0.1.0', '0', '0')",
                [],
            )
            .unwrap_err();
        assert!(
            replace_into_err
                .to_string()
                .contains("telemetry_readings is append-only"),
            "unexpected replace into error: {replace_into_err}"
        );

        // La fila original debe seguir intacta.
        let items = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "a1".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].value, json!({"bpm": 72}));

        // Protección de telemetry_heads: sin DELETE ni UPDATE hacia atrás o igual.
        let head_del_err = store
            .conn
            .execute("DELETE FROM telemetry_heads WHERE astronaut_key = 'a1'", [])
            .unwrap_err();
        assert!(
            head_del_err
                .to_string()
                .contains("telemetry_heads cannot be deleted")
        );

        let head_replace_err = store
            .conn
            .execute(
                "INSERT OR REPLACE INTO telemetry_heads (
                     astronaut_key, indicator, last_reading_id, last_hash, count
                 ) VALUES ('a1', 'heart_rate', 99, 'abc', 99)",
                [],
            )
            .unwrap_err();
        assert!(
            head_replace_err
                .to_string()
                .contains("telemetry_heads cannot be replaced")
        );

        let head_backwards_err = store
            .conn
            .execute(
                "UPDATE telemetry_heads SET count = 1, last_reading_id = 2 WHERE astronaut_key = 'a1'",
                [],
            )
            .unwrap_err();
        assert!(
            head_backwards_err
                .to_string()
                .contains("telemetry_heads can only advance forward")
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
    fn verify_detects_tampering_truncation_and_missing_series() {
        // 1. Caso sano con dos series.
        let mut store = Store::open_in_memory().unwrap();
        for (idx, bpm) in [70, 72, 74].into_iter().enumerate() {
            let mut readings = vec![reading("a1", "heart_rate", json!({"bpm": bpm}), "bpm")];
            if idx < 2 {
                readings.push(reading(
                    "a1",
                    "hrv",
                    json!({"ms": 55 + i64::try_from(idx).unwrap()}),
                    "ms",
                ));
            }
            store
                .record_telemetry(
                    RecordTelemetryInput { readings },
                    t(i64::try_from(idx).unwrap() * MIN_CHANGE_INTERVAL_MS),
                    V,
                )
                .unwrap();
        }

        let ok = store.verify_telemetry(None).unwrap();
        assert_eq!(
            ok,
            VerifyTelemetryResult {
                ok: true,
                checked: 5,
                series_checked: 2,
                checked_through_reading_id: Some(5),
                first_bad_reading_id: None,
                problem: None,
            }
        );

        // 2. Alterar un valor → hash_mismatch.
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
                checked: 5,
                series_checked: 2,
                checked_through_reading_id: Some(5),
                first_bad_reading_id: Some(2),
                problem: Some("hash_mismatch".into()),
            }
        );

        // 3. Romper el enlace eliminando la primera fila → broken_link.
        let mut store_link = Store::open_in_memory().unwrap();
        for idx in 0_i64..2 {
            store_link
                .record_telemetry(
                    RecordTelemetryInput {
                        readings: vec![reading(
                            "a1",
                            "heart_rate",
                            json!({"bpm": 70 + idx}),
                            "bpm",
                        )],
                    },
                    t(idx * MIN_CHANGE_INTERVAL_MS),
                    V,
                )
                .unwrap();
        }
        store_link
            .conn
            .execute_batch("DROP TRIGGER telemetry_readings_no_delete;")
            .unwrap();
        store_link
            .conn
            .execute("DELETE FROM telemetry_readings WHERE reading_id = 1", [])
            .unwrap();
        let broken = store_link.verify_telemetry(None).unwrap();
        assert_eq!(
            broken,
            VerifyTelemetryResult {
                ok: false,
                checked: 1,
                series_checked: 1,
                checked_through_reading_id: Some(2),
                first_bad_reading_id: Some(2),
                problem: Some("broken_link".into()),
            }
        );

        // 4. Truncar la cola → missing_tail.
        let mut store_tail = Store::open_in_memory().unwrap();
        for idx in 0_i64..3 {
            store_tail
                .record_telemetry(
                    RecordTelemetryInput {
                        readings: vec![reading(
                            "a1",
                            "heart_rate",
                            json!({"bpm": 70 + idx}),
                            "bpm",
                        )],
                    },
                    t(idx * MIN_CHANGE_INTERVAL_MS),
                    V,
                )
                .unwrap();
        }
        store_tail
            .conn
            .execute_batch("DROP TRIGGER telemetry_readings_no_delete;")
            .unwrap();
        store_tail
            .conn
            .execute("DELETE FROM telemetry_readings WHERE reading_id = 3", [])
            .unwrap();
        let truncated = store_tail.verify_telemetry(None).unwrap();
        assert_eq!(
            truncated,
            VerifyTelemetryResult {
                ok: false,
                checked: 2,
                series_checked: 1,
                checked_through_reading_id: Some(2),
                first_bad_reading_id: Some(3),
                problem: Some("missing_tail".into()),
            }
        );

        // 5. Borrar una serie entera → missing_series.
        let mut store_series = Store::open_in_memory().unwrap();
        store_series
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "heart_rate", json!({"bpm": 70}), "bpm"),
                        reading("a1", "hrv", json!({"ms": 60}), "ms"),
                    ],
                },
                t(0),
                V,
            )
            .unwrap();
        store_series
            .conn
            .execute_batch("DROP TRIGGER telemetry_readings_no_delete;")
            .unwrap();
        store_series
            .conn
            .execute(
                "DELETE FROM telemetry_readings WHERE astronaut_key = 'a1' AND indicator = 'hrv'",
                [],
            )
            .unwrap();
        let missing = store_series.verify_telemetry(Some("a1".into())).unwrap();
        assert_eq!(
            missing,
            VerifyTelemetryResult {
                ok: false,
                checked: 1,
                series_checked: 2,
                checked_through_reading_id: Some(1),
                first_bad_reading_id: None,
                problem: Some("missing_series".into()),
            }
        );

        // 6. Discrepancia de conteo en la cabeza → count_mismatch.
        let mut store_count = Store::open_in_memory().unwrap();
        store_count
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a1", "heart_rate", json!({"bpm": 70}), "bpm")],
                },
                t(0),
                V,
            )
            .unwrap();
        store_count
            .conn
            .execute_batch("DROP TRIGGER telemetry_heads_forward_only;")
            .unwrap();
        store_count
            .conn
            .execute(
                "UPDATE telemetry_heads SET count = 2 WHERE astronaut_key = 'a1' AND indicator = 'heart_rate'",
                [],
            )
            .unwrap();
        let count_bad = store_count.verify_telemetry(None).unwrap();
        assert_eq!(
            count_bad,
            VerifyTelemetryResult {
                ok: false,
                checked: 1,
                series_checked: 1,
                checked_through_reading_id: Some(1),
                first_bad_reading_id: None,
                problem: Some("count_mismatch".into()),
            }
        );
    }

    #[test]
    fn record_telemetry_rolls_back_rows_and_heads_on_sql_error() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .conn
            .execute_batch(
                "CREATE TRIGGER test_abort_on_999
                 BEFORE INSERT ON telemetry_readings
                 WHEN NEW.value_json LIKE '%999%'
                 BEGIN
                     SELECT RAISE(ABORT, 'simulated sql failure');
                 END;",
            )
            .unwrap();

        // Lote con primera inserción válida y segunda abortada por SQL → rollback total.
        let err = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a1", "heart_rate", json!({"bpm": 72}), "bpm"),
                        reading("a1", "oxygen_in_blood", json!({"percentage": 999}), "%"),
                    ],
                },
                t(0),
                V,
            )
            .unwrap_err();
        assert_eq!(err.code, "storage_error");

        let rows: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM telemetry_readings", [], |r| r.get(0))
            .unwrap();
        let heads: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM telemetry_heads", [], |r| r.get(0))
            .unwrap();
        assert_eq!((rows, heads), (0, 0));

        // Con una lectura previa válida, un lote que falla en la segunda inserción no altera ni filas ni cabeza.
        store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![reading("a2", "heart_rate", json!({"bpm": 70}), "bpm")],
                },
                t(0),
                V,
            )
            .unwrap();
        let err2 = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: vec![
                        reading("a2", "heart_rate", json!({"bpm": 75}), "bpm"),
                        reading("a2", "oxygen_in_blood", json!({"percentage": 999}), "%"),
                    ],
                },
                t(MIN_CHANGE_INTERVAL_MS),
                V,
            )
            .unwrap_err();
        assert_eq!(err2.code, "storage_error");

        let a2_items = store
            .list_telemetry(ListTelemetryInput {
                astronaut_key: "a2".into(),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(a2_items.len(), 1);
        assert_eq!(a2_items[0].value, json!({"bpm": 70}));

        let head_state: (i64, i64) = store
            .conn
            .query_row(
                "SELECT last_reading_id, count FROM telemetry_heads WHERE astronaut_key = 'a2' AND indicator = 'heart_rate'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(head_state, (a2_items[0].reading_id, 1));
        assert!(store.verify_telemetry(None).unwrap().ok);
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

    struct TempDbPath(std::path::PathBuf);

    impl TempDbPath {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "merken-telemetry-{name}-{}-{:?}.sqlite3",
                std::process::id(),
                std::thread::current().id()
            ));
            let helper = Self(path);
            helper.cleanup();
            helper
        }

        fn cleanup(&self) {
            let _ = std::fs::remove_file(&self.0);
            let wal = format!("{}-wal", self.0.display());
            let shm = format!("{}-shm", self.0.display());
            let _ = std::fs::remove_file(wal);
            let _ = std::fs::remove_file(shm);
        }
    }

    impl Drop for TempDbPath {
        fn drop(&mut self) {
            self.cleanup();
        }
    }

    fn seed_v3_base(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE profile (id INTEGER PRIMARY KEY CHECK(id=1), astronaut_json TEXT NOT NULL, medical_profile_json TEXT NOT NULL, medical_conditions_json TEXT NOT NULL, profile_version INTEGER NOT NULL, is_demo INTEGER NOT NULL, updated_at INTEGER NOT NULL);
             CREATE TABLE records (local_order INTEGER PRIMARY KEY AUTOINCREMENT, record_id TEXT NOT NULL UNIQUE, indicator TEXT NOT NULL, body_zone TEXT, value_json TEXT NOT NULL, unit TEXT NOT NULL, measured_at INTEGER NOT NULL, logged_at INTEGER NOT NULL, source TEXT NOT NULL, interpreted_by_json TEXT, replaces TEXT, root_id TEXT NOT NULL, note TEXT, seq INTEGER, received_at INTEGER, is_demo INTEGER NOT NULL);
             CREATE TABLE events (event_id TEXT PRIMARY KEY, kind TEXT NOT NULL, certainty TEXT NOT NULL, message TEXT, body_zone TEXT, priority TEXT NOT NULL, occurred_at INTEGER NOT NULL, logged_at INTEGER NOT NULL, seq INTEGER, received_at INTEGER, is_demo INTEGER NOT NULL);
             CREATE TABLE evaluations (evaluation_id TEXT PRIMARY KEY, series_key TEXT NOT NULL, payload_json TEXT NOT NULL, evaluated_at INTEGER NOT NULL, is_demo INTEGER NOT NULL);
             CREATE TABLE outbox (outbox_id INTEGER PRIMARY KEY AUTOINCREMENT, entity TEXT NOT NULL, entity_id TEXT NOT NULL, payload_json TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('pending','synced','failed')), attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT, created_at INTEGER NOT NULL, UNIQUE(entity, entity_id));
             CREATE TABLE sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE pulled_changes (seq INTEGER PRIMARY KEY, entity TEXT NOT NULL, entity_id TEXT NOT NULL, payload_json TEXT NOT NULL, applied_at INTEGER NOT NULL);
             PRAGMA user_version=3;",
        )
        .unwrap();
    }

    fn seed_old_v4_schema(conn: &Connection) {
        seed_v3_base(conn);
        conn.execute_batch(
            "CREATE TABLE telemetry_readings (
                 reading_id INTEGER PRIMARY KEY AUTOINCREMENT,
                 astronaut_key TEXT NOT NULL,
                 astronaut_name TEXT NOT NULL,
                 indicator TEXT NOT NULL,
                 value_json TEXT NOT NULL,
                 unit TEXT NOT NULL,
                 observed_at INTEGER NOT NULL,
                 batch_id TEXT NOT NULL,
                 source TEXT NOT NULL,
                 app_version TEXT NOT NULL,
                 prev_hash TEXT NOT NULL,
                 row_hash TEXT NOT NULL
             );
             CREATE INDEX telemetry_readings_series
                 ON telemetry_readings(astronaut_key, indicator, observed_at);
             CREATE INDEX telemetry_readings_astronaut_time
                 ON telemetry_readings(astronaut_key, observed_at, reading_id);
             CREATE TRIGGER telemetry_readings_no_update
                 BEFORE UPDATE ON telemetry_readings
                 BEGIN
                     SELECT RAISE(ABORT, 'telemetry_readings is append-only');
                 END;
             CREATE TRIGGER telemetry_readings_no_delete
                 BEFORE DELETE ON telemetry_readings
                 BEGIN
                     SELECT RAISE(ABORT, 'telemetry_readings is append-only');
                 END;
             PRAGMA user_version=4;",
        )
        .unwrap();
    }

    fn seed_expanded_v4_schema(conn: &Connection) {
        seed_old_v4_schema(conn);
        conn.execute_batch(
            "CREATE INDEX telemetry_readings_chain
                 ON telemetry_readings(astronaut_key, indicator, reading_id);
             CREATE INDEX telemetry_readings_astronaut_reading
                 ON telemetry_readings(astronaut_key, reading_id);
             CREATE TRIGGER telemetry_readings_no_replace
                 BEFORE INSERT ON telemetry_readings
                 WHEN NEW.reading_id IS NOT NULL
                  AND EXISTS (SELECT 1 FROM telemetry_readings WHERE reading_id = NEW.reading_id)
                 BEGIN
                     SELECT RAISE(ABORT, 'telemetry_readings is append-only');
                 END;
             CREATE TABLE telemetry_heads (
                 astronaut_key TEXT NOT NULL,
                 indicator TEXT NOT NULL,
                 last_reading_id INTEGER NOT NULL,
                 last_hash TEXT NOT NULL,
                 count INTEGER NOT NULL,
                 PRIMARY KEY(astronaut_key, indicator)
             );
             CREATE TRIGGER telemetry_heads_no_delete
                 BEFORE DELETE ON telemetry_heads
                 BEGIN
                     SELECT RAISE(ABORT, 'telemetry_heads cannot be deleted');
                 END;
             CREATE TRIGGER telemetry_heads_forward_only
                 BEFORE UPDATE ON telemetry_heads
                 WHEN NEW.count <= OLD.count
                   OR NEW.last_reading_id <= OLD.last_reading_id
                   OR NEW.astronaut_key != OLD.astronaut_key
                   OR NEW.indicator != OLD.indicator
                 BEGIN
                     SELECT RAISE(ABORT, 'telemetry_heads can only advance forward');
                 END;
             PRAGMA user_version=4;",
        )
        .unwrap();
    }

    struct RawTelemetryRow<'a> {
        astronaut_key: &'a str,
        indicator: &'a str,
        value_json: &'a str,
        unit: &'a str,
        observed_at: i64,
        batch_id: &'a str,
        prev_hash: &'a str,
    }

    fn insert_raw_telemetry_row(conn: &Connection, row: RawTelemetryRow<'_>) -> (i64, String) {
        let row_hash = compute_row_hash(&HashFields {
            astronaut_key: row.astronaut_key,
            indicator: row.indicator,
            value_json: row.value_json,
            unit: row.unit,
            observed_at: row.observed_at,
            batch_id: row.batch_id,
            source: TELEMETRY_SOURCE,
            app_version: V,
            prev_hash: row.prev_hash,
        });
        conn.execute(
            "INSERT INTO telemetry_readings (
                 astronaut_key, astronaut_name, indicator, value_json, unit,
                 observed_at, batch_id, source, app_version, prev_hash, row_hash
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                row.astronaut_key,
                format!("Astronaut {}", row.astronaut_key),
                row.indicator,
                row.value_json,
                row.unit,
                row.observed_at,
                row.batch_id,
                TELEMETRY_SOURCE,
                V,
                row.prev_hash,
                row_hash,
            ],
        )
        .unwrap();
        (conn.last_insert_rowid(), row_hash)
    }

    #[test]
    fn migration_v3_to_v5_preserves_data_and_reopen_is_idempotent() {
        let db = TempDbPath::new("v3-v5");

        // Construir una base en versión 3 con datos existentes.
        {
            let conn = Connection::open(&db.0).unwrap();
            seed_v3_base(&conn);
            conn.execute_batch(
                "INSERT INTO records(record_id, indicator, body_zone, value_json, unit, measured_at, logged_at, source, root_id, is_demo)
                 VALUES ('r-v3', 'heart_rate', NULL, '{\"bpm\":70}', 'bpm', 1800000000000, 1800000000000, 'form', 'r-v3', 0);
                 INSERT INTO outbox(entity, entity_id, payload_json, state, created_at)
                 VALUES ('record', 'r-v3', '{\"record_id\":\"r-v3\"}', 'pending', 1800000000000);
                 INSERT INTO pulled_changes(seq, entity, entity_id, payload_json, applied_at)
                 VALUES (1, 'analysis', 'an-1', '{\"seq\":1}', 1800000000000);",
            )
            .unwrap();
        }

        // Primera apertura: sube a v5 y conserva los datos v3.
        {
            let mut store = Store::open(&db.0).unwrap();
            let ver: i64 = store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(ver, 5);
            let rec_triggers: i64 = store
                .conn
                .pragma_query_value(None, "recursive_triggers", |r| r.get(0))
                .unwrap();
            assert_eq!(rec_triggers, 1);
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

        // Segunda apertura: idempotente y mantiene tanto v3 como v5.
        {
            let store = Store::open(&db.0).unwrap();
            let ver: i64 = store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(ver, 5);
            let rec_triggers: i64 = store
                .conn
                .pragma_query_value(None, "recursive_triggers", |r| r.get(0))
                .unwrap();
            assert_eq!(rec_triggers, 1);
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

        // Versión futura (6) rechazada con invalid_schema.
        {
            let conn = Connection::open(&db.0).unwrap();
            conn.pragma_update(None, "user_version", 6).unwrap();
        }
        let err = match Store::open(&db.0) {
            Ok(_) => panic!("expected unsupported schema version error"),
            Err(err) => err,
        };
        assert_eq!(err.code, "invalid_schema");
    }

    #[test]
    fn migration_old_v4_to_v5_backfills_heads_and_is_idempotent() {
        let db = TempDbPath::new("old-v4-v5");
        let (hr_id_2, hr_hash_2);
        let (rad_id, rad_hash);
        let (a2_id, a2_hash);

        {
            let conn = Connection::open(&db.0).unwrap();
            seed_old_v4_schema(&conn);
            let (_hr_id_1, hr_hash_1) = insert_raw_telemetry_row(
                &conn,
                RawTelemetryRow {
                    astronaut_key: "a1",
                    indicator: "heart_rate",
                    value_json: r#"{"bpm":70}"#,
                    unit: "bpm",
                    observed_at: t(0).millis(),
                    batch_id: "b-old-1",
                    prev_hash: GENESIS_PREV_HASH,
                },
            );
            (hr_id_2, hr_hash_2) = insert_raw_telemetry_row(
                &conn,
                RawTelemetryRow {
                    astronaut_key: "a1",
                    indicator: "heart_rate",
                    value_json: r#"{"bpm":74}"#,
                    unit: "bpm",
                    observed_at: t(MIN_CHANGE_INTERVAL_MS).millis(),
                    batch_id: "b-old-2",
                    prev_hash: &hr_hash_1,
                },
            );
            // Lectura heredada con JSON antiguo: no debe reescribirse ni romper hashes.
            (rad_id, rad_hash) = insert_raw_telemetry_row(
                &conn,
                RawTelemetryRow {
                    astronaut_key: "a1",
                    indicator: "space_radiation",
                    value_json: r#"{"value":14.1}"#,
                    unit: "µSv/h",
                    observed_at: t(0).millis(),
                    batch_id: "b-old-1",
                    prev_hash: GENESIS_PREV_HASH,
                },
            );
            (a2_id, a2_hash) = insert_raw_telemetry_row(
                &conn,
                RawTelemetryRow {
                    astronaut_key: "a2",
                    indicator: "heart_rate",
                    value_json: r#"{"bpm":64}"#,
                    unit: "bpm",
                    observed_at: t(0).millis(),
                    batch_id: "b-old-1",
                    prev_hash: GENESIS_PREV_HASH,
                },
            );
        }

        // Primera apertura: migra v4 antigua -> v5 y rellena telemetry_heads.
        {
            let mut store = Store::open(&db.0).unwrap();
            let ver: i64 = store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(ver, 5);

            let heads = load_telemetry_heads(&store.conn, None).unwrap();
            assert_eq!(heads.len(), 3);
            let h1 = &heads[&("a1".to_owned(), "heart_rate".to_owned())];
            assert_eq!(
                (h1.last_reading_id, &h1.last_hash, h1.count),
                (hr_id_2, &hr_hash_2, 2)
            );
            let h_rad = &heads[&("a1".to_owned(), "space_radiation".to_owned())];
            assert_eq!(
                (h_rad.last_reading_id, &h_rad.last_hash, h_rad.count),
                (rad_id, &rad_hash, 1)
            );
            let h2 = &heads[&("a2".to_owned(), "heart_rate".to_owned())];
            assert_eq!(
                (h2.last_reading_id, &h2.last_hash, h2.count),
                (a2_id, &a2_hash, 1)
            );

            // JSON y hash originales conservados intactos.
            let (stored_json, stored_hash): (String, String) = store
                .conn
                .query_row(
                    "SELECT value_json, row_hash FROM telemetry_readings WHERE reading_id = ?1",
                    [rad_id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(stored_json, r#"{"value":14.1}"#);
            assert_eq!(stored_hash, rad_hash);

            assert_eq!(
                store.verify_telemetry(None).unwrap(),
                VerifyTelemetryResult {
                    ok: true,
                    checked: 4,
                    series_checked: 3,
                    checked_through_reading_id: Some(4),
                    first_bad_reading_id: None,
                    problem: None,
                }
            );

            // Siguientes inserciones encadenan y avanzan telemetry_heads.
            let rec = store
                .record_telemetry(
                    RecordTelemetryInput {
                        readings: vec![
                            reading("a1", "heart_rate", json!({"bpm": 78}), "bpm"),
                            reading("a1", "space_radiation", json!({"usv_h": 15.2}), "µSv/h"),
                            reading("a2", "hrv", json!({"ms": 60}), "ms"),
                        ],
                    },
                    t(2 * MIN_CHANGE_INTERVAL_MS),
                    V,
                )
                .unwrap();
            assert_eq!((rec.stored, rec.skipped), (3, 0));
            assert_eq!(
                store.verify_telemetry(None).unwrap(),
                VerifyTelemetryResult {
                    ok: true,
                    checked: 7,
                    series_checked: 4,
                    checked_through_reading_id: Some(7),
                    first_bad_reading_id: None,
                    problem: None,
                }
            );
        }

        // Segunda apertura: completamente idempotente.
        {
            let store = Store::open(&db.0).unwrap();
            let ver: i64 = store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(ver, 5);
            assert_eq!(
                store.verify_telemetry(None).unwrap(),
                VerifyTelemetryResult {
                    ok: true,
                    checked: 7,
                    series_checked: 4,
                    checked_through_reading_id: Some(7),
                    first_bad_reading_id: None,
                    problem: None,
                }
            );
        }
    }

    #[test]
    fn migration_expanded_v4_to_v5_is_idempotent_and_protects_heads() {
        let db = TempDbPath::new("expanded-v4-v5");
        {
            let conn = Connection::open(&db.0).unwrap();
            seed_expanded_v4_schema(&conn);
            let (id1, hash1) = insert_raw_telemetry_row(
                &conn,
                RawTelemetryRow {
                    astronaut_key: "a1",
                    indicator: "heart_rate",
                    value_json: r#"{"bpm":70}"#,
                    unit: "bpm",
                    observed_at: t(0).millis(),
                    batch_id: "b-exp-1",
                    prev_hash: GENESIS_PREV_HASH,
                },
            );
            conn.execute(
                "INSERT INTO telemetry_heads (astronaut_key, indicator, last_reading_id, last_hash, count)
                 VALUES ('a1', 'heart_rate', ?1, ?2, 1)",
                params![id1, hash1],
            )
            .unwrap();
        }

        {
            let mut store = Store::open(&db.0).unwrap();
            let ver: i64 = store
                .conn
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(ver, 5);
            assert!(store.verify_telemetry(None).unwrap().ok);

            // Nuevo trigger BEFORE INSERT bloquea REPLACE incluso con recursive_triggers = 0.
            store
                .conn
                .pragma_update(None, "recursive_triggers", 0)
                .unwrap();
            let err = store
                .conn
                .execute(
                    "REPLACE INTO telemetry_heads (astronaut_key, indicator, last_reading_id, last_hash, count)
                     VALUES ('a1', 'heart_rate', 99, 'tampered', 99)",
                    [],
                )
                .unwrap_err();
            assert!(
                err.to_string()
                    .contains("telemetry_heads cannot be replaced")
            );

            let rec = store
                .record_telemetry(
                    RecordTelemetryInput {
                        readings: vec![reading("a1", "heart_rate", json!({"bpm": 75}), "bpm")],
                    },
                    t(MIN_CHANGE_INTERVAL_MS),
                    V,
                )
                .unwrap();
            assert_eq!(rec.stored, 1);
            assert_eq!(
                store.verify_telemetry(None).unwrap(),
                VerifyTelemetryResult {
                    ok: true,
                    checked: 2,
                    series_checked: 1,
                    checked_through_reading_id: Some(2),
                    first_bad_reading_id: None,
                    problem: None,
                }
            );
        }

        let store = Store::open(&db.0).unwrap();
        assert_eq!(
            store.verify_telemetry(None).unwrap(),
            VerifyTelemetryResult {
                ok: true,
                checked: 2,
                series_checked: 1,
                checked_through_reading_id: Some(2),
                first_bad_reading_id: None,
                problem: None,
            }
        );
    }

    #[test]
    fn migration_old_v4_with_incoherent_chain_migrates_and_verify_reports() {
        let db = TempDbPath::new("old-v4-bad-chain");
        let (bad_id, bad_hash);

        {
            let conn = Connection::open(&db.0).unwrap();
            seed_old_v4_schema(&conn);
            let (_id1, _hash1) = insert_raw_telemetry_row(
                &conn,
                RawTelemetryRow {
                    astronaut_key: "a1",
                    indicator: "heart_rate",
                    value_json: r#"{"bpm":70}"#,
                    unit: "bpm",
                    observed_at: t(0).millis(),
                    batch_id: "b-bad-1",
                    prev_hash: GENESIS_PREV_HASH,
                },
            );
            (bad_id, bad_hash) = insert_raw_telemetry_row(
                &conn,
                RawTelemetryRow {
                    astronaut_key: "a1",
                    indicator: "heart_rate",
                    value_json: r#"{"bpm":72}"#,
                    unit: "bpm",
                    observed_at: t(MIN_CHANGE_INTERVAL_MS).millis(),
                    batch_id: "b-bad-2",
                    prev_hash: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                },
            );
        }

        // La migración NO debe fallar por cadena rota; debe rellenar cabezas y verify lo reporta.
        let store = Store::open(&db.0).unwrap();
        let ver: i64 = store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(ver, 5);

        let heads = load_telemetry_heads(&store.conn, None).unwrap();
        let head = &heads[&("a1".to_owned(), "heart_rate".to_owned())];
        assert_eq!(
            (head.last_reading_id, &head.last_hash, head.count),
            (bad_id, &bad_hash, 2)
        );

        assert_eq!(
            store.verify_telemetry(None).unwrap(),
            VerifyTelemetryResult {
                ok: false,
                checked: 2,
                series_checked: 1,
                checked_through_reading_id: Some(bad_id),
                first_bad_reading_id: Some(bad_id),
                problem: Some("broken_link".into()),
            }
        );
    }

    #[test]
    fn fixture_batch_records_all_twelve_readings_without_skips() {
        #[derive(Deserialize)]
        struct BatchFixture {
            readings: Vec<TelemetryReadingInput>,
        }

        const FIXTURE_JSON: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/api/fixtures/telemetry-record-batch.json"
        ));
        let fixture: BatchFixture = serde_json::from_str(FIXTURE_JSON).unwrap();
        assert_eq!(fixture.readings.len(), 12);

        let mut store = Store::open_in_memory().unwrap();
        let res = store
            .record_telemetry(
                RecordTelemetryInput {
                    readings: fixture.readings,
                },
                t(0),
                V,
            )
            .unwrap();
        assert_eq!((res.stored, res.skipped), (12, 0));
        assert_eq!(
            store.verify_telemetry(None).unwrap(),
            VerifyTelemetryResult {
                ok: true,
                checked: 12,
                series_checked: 12,
                checked_through_reading_id: Some(12),
                first_bad_reading_id: None,
                problem: None,
            }
        );
    }

    #[test]
    fn append_only_matrix_with_recursive_triggers_off_and_on() {
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

        for (step, mode) in [0_i64, 1_i64].into_iter().enumerate() {
            store
                .conn
                .pragma_update(None, "recursive_triggers", mode)
                .unwrap();
            let active_mode: i64 = store
                .conn
                .pragma_query_value(None, "recursive_triggers", |r| r.get(0))
                .unwrap();
            assert_eq!(active_mode, mode);

            let readings_cases = [
                (
                    "REPLACE INTO telemetry_readings",
                    "REPLACE INTO telemetry_readings (
                         reading_id, astronaut_key, astronaut_name, indicator, value_json, unit,
                         observed_at, batch_id, source, app_version, prev_hash, row_hash
                     ) VALUES (1, 'a1', 'Tampered', 'heart_rate', '{\"bpm\":99}', 'bpm', 1, 'b', 'live_api', '0.1.0', '0', '0')",
                    "telemetry_readings is append-only",
                ),
                (
                    "INSERT OR REPLACE INTO telemetry_readings",
                    "INSERT OR REPLACE INTO telemetry_readings (
                         reading_id, astronaut_key, astronaut_name, indicator, value_json, unit,
                         observed_at, batch_id, source, app_version, prev_hash, row_hash
                     ) VALUES (1, 'a1', 'Tampered', 'heart_rate', '{\"bpm\":99}', 'bpm', 1, 'b', 'live_api', '0.1.0', '0', '0')",
                    "telemetry_readings is append-only",
                ),
                (
                    "UPDATE telemetry_readings",
                    "UPDATE telemetry_readings SET value_json = '{\"bpm\":99}' WHERE reading_id = 1",
                    "telemetry_readings is append-only",
                ),
                (
                    "DELETE FROM telemetry_readings",
                    "DELETE FROM telemetry_readings WHERE reading_id = 1",
                    "telemetry_readings is append-only",
                ),
            ];

            let heads_cases = [
                (
                    "REPLACE INTO telemetry_heads",
                    "REPLACE INTO telemetry_heads (
                         astronaut_key, indicator, last_reading_id, last_hash, count
                     ) VALUES ('a1', 'heart_rate', 99, 'tampered', 99)",
                    "telemetry_heads cannot be replaced",
                ),
                (
                    "INSERT OR REPLACE INTO telemetry_heads",
                    "INSERT OR REPLACE INTO telemetry_heads (
                         astronaut_key, indicator, last_reading_id, last_hash, count
                     ) VALUES ('a1', 'heart_rate', 99, 'tampered', 99)",
                    "telemetry_heads cannot be replaced",
                ),
                (
                    "UPDATE telemetry_heads",
                    "UPDATE telemetry_heads SET last_hash = 'tampered' WHERE astronaut_key = 'a1' AND indicator = 'heart_rate'",
                    "telemetry_heads can only advance forward",
                ),
                (
                    "DELETE FROM telemetry_heads",
                    "DELETE FROM telemetry_heads WHERE astronaut_key = 'a1' AND indicator = 'heart_rate'",
                    "telemetry_heads cannot be deleted",
                ),
            ];

            for (label, sql, expected_msg) in readings_cases.into_iter().chain(heads_cases) {
                let err = store.conn.execute(sql, []).unwrap_err();
                assert!(
                    err.to_string().contains(expected_msg),
                    "recursive_triggers={mode}, {label}: expected '{expected_msg}', got '{err}'"
                );
            }

            let step_i64 = i64::try_from(step).unwrap();
            let res = store
                .record_telemetry(
                    RecordTelemetryInput {
                        readings: vec![reading(
                            "a1",
                            "heart_rate",
                            json!({"bpm": 74 + step_i64}),
                            "bpm",
                        )],
                    },
                    t((step_i64 + 1) * MIN_CHANGE_INTERVAL_MS),
                    V,
                )
                .unwrap();
            assert_eq!(res.stored, 1);

            let (head_count, head_last_id): (i64, i64) = store
                .conn
                .query_row(
                    "SELECT count, last_reading_id FROM telemetry_heads WHERE astronaut_key = 'a1' AND indicator = 'heart_rate'",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(head_count, step_i64 + 2);
            assert_eq!(head_last_id, step_i64 + 2);
            assert!(store.verify_telemetry(None).unwrap().ok);
        }
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
                t(MIN_CHANGE_INTERVAL_MS),
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
        assert_eq!(
            v2,
            VerifyTelemetryResult {
                ok: true,
                checked: 1,
                series_checked: 1,
                checked_through_reading_id: Some(a2[0].reading_id),
                first_bad_reading_id: None,
                problem: None,
            }
        );

        let v1 = store.verify_telemetry(Some("astro-1".into())).unwrap();
        assert_eq!(
            v1,
            VerifyTelemetryResult {
                ok: false,
                checked: 2,
                series_checked: 1,
                checked_through_reading_id: Some(a1[0].reading_id),
                first_bad_reading_id: Some(a1[0].reading_id),
                problem: Some("hash_mismatch".into()),
            }
        );
    }

    #[test]
    fn explain_query_plan_uses_indexes_without_temp_btree() {
        let store = Store::open_in_memory().unwrap();
        let cols = "reading_id, astronaut_key, astronaut_name, indicator, value_json, unit, observed_at, batch_id, source, app_version, prev_hash, row_hash";
        let queries: Vec<(&str, String, Vec<rusqlite::types::Value>, &str)> = vec![
            (
                "list_all",
                format!(
                    "SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 ORDER BY reading_id DESC LIMIT ?2"
                ),
                vec!["a1".to_owned().into(), 200_i64.into()],
                "telemetry_readings_astronaut_reading",
            ),
            (
                "list_indicator",
                format!(
                    "SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 AND indicator = ?2 ORDER BY reading_id DESC LIMIT ?3"
                ),
                vec![
                    "a1".to_owned().into(),
                    "heart_rate".to_owned().into(),
                    200_i64.into(),
                ],
                "telemetry_readings_chain",
            ),
            (
                "verify_astronaut",
                format!(
                    "SELECT {cols} FROM telemetry_readings WHERE astronaut_key = ?1 ORDER BY reading_id ASC"
                ),
                vec!["a1".to_owned().into()],
                "telemetry_readings_astronaut_reading",
            ),
        ];

        for (name, sql, params, expected_index) in queries {
            let eqp = format!("EXPLAIN QUERY PLAN {sql}");
            let mut stmt = store.conn.prepare(&eqp).unwrap();
            let details: Vec<String> = stmt
                .query_map(params_from_iter(params), |r| r.get::<_, String>(3))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            let joined = details.join("\n");
            assert!(
                !joined.contains("TEMP B-TREE"),
                "{name} unexpectedly used a temp B-tree: {joined}"
            );
            assert!(
                joined.contains(expected_index),
                "{name} expected index {expected_index}, got: {joined}"
            );
        }
    }
}
