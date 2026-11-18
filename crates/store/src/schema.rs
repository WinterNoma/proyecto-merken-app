use crate::{SCHEMA_VERSION, Store, StoreError};
use rusqlite::{Connection, Transaction, TransactionBehavior};

pub(super) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let version = supported_schema_version(&tx)?;
    Store::migrate_to_v5(&tx, version)?;
    if version < SCHEMA_VERSION {
        migrate_to_v6(&tx)?;
    }
    tx.commit()?;
    Ok(())
}

impl Store {
    pub(super) fn migrate_to_v5(tx: &Transaction<'_>, version: i64) -> Result<(), StoreError> {
        if version == 0 {
            tx.execute_batch("CREATE TABLE profile (id INTEGER PRIMARY KEY CHECK(id=1), astronaut_json TEXT NOT NULL, medical_profile_json TEXT NOT NULL, medical_conditions_json TEXT NOT NULL, profile_version INTEGER NOT NULL, is_demo INTEGER NOT NULL, updated_at INTEGER NOT NULL);
CREATE TABLE records (local_order INTEGER PRIMARY KEY AUTOINCREMENT, record_id TEXT NOT NULL UNIQUE, indicator TEXT NOT NULL, body_zone TEXT, value_json TEXT NOT NULL, unit TEXT NOT NULL, measured_at INTEGER NOT NULL, logged_at INTEGER NOT NULL, source TEXT NOT NULL, interpreted_by_json TEXT, replaces TEXT, root_id TEXT NOT NULL, note TEXT, seq INTEGER, received_at INTEGER, is_demo INTEGER NOT NULL);
CREATE TABLE events (event_id TEXT PRIMARY KEY, kind TEXT NOT NULL, certainty TEXT NOT NULL, message TEXT, body_zone TEXT, priority TEXT NOT NULL, occurred_at INTEGER NOT NULL, logged_at INTEGER NOT NULL, seq INTEGER, received_at INTEGER, is_demo INTEGER NOT NULL);
CREATE TABLE evaluations (evaluation_id TEXT PRIMARY KEY, series_key TEXT NOT NULL, payload_json TEXT NOT NULL, evaluated_at INTEGER NOT NULL, is_demo INTEGER NOT NULL);
CREATE TABLE outbox (outbox_id INTEGER PRIMARY KEY AUTOINCREMENT, entity TEXT NOT NULL, entity_id TEXT NOT NULL, payload_json TEXT NOT NULL, state TEXT NOT NULL CHECK(state IN ('pending','synced','failed')), attempts INTEGER NOT NULL DEFAULT 0, last_error TEXT, created_at INTEGER NOT NULL, UNIQUE(entity, entity_id));
CREATE TABLE sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE INDEX records_measurement ON records(measured_at, local_order);
CREATE INDEX evaluations_series ON evaluations(series_key, evaluated_at);
PRAGMA user_version=1;")?;
        }
        if version <= 1 {
            // v2 (revisión de Sol, B4): todo lo que llega en `pull` se conserva
            // íntegro, aunque la app aún no sepa mostrar esa entidad.
            tx.execute_batch(
                "CREATE TABLE pulled_changes (seq INTEGER PRIMARY KEY, entity TEXT NOT NULL,
                     entity_id TEXT NOT NULL, payload_json TEXT NOT NULL, applied_at INTEGER NOT NULL);
                 PRAGMA user_version=2;",
            )?;
        }
        if version <= 2 {
            // v3 (revisión de Sol, I1): `headache` salió del catálogo el 14-nov.
            // Lo pendiente de subir con ese indicador se conserva, pero no se envía.
            tx.execute_batch(
                "UPDATE outbox SET state = 'failed',
                     last_error = 'retired_indicator: headache (catalog 2026-11-14); kept locally, not sent'
                 WHERE state = 'pending' AND entity IN ('record', 'evaluation')
                   AND json_extract(payload_json, '$.indicator') = 'headache';
                 PRAGMA user_version=3;",
            )?;
        }
        if version <= 3 {
            // v4: telemetría local append-only (no entra en outbox ni se limpia con DEMO).
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS telemetry_readings (
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
                 CREATE INDEX IF NOT EXISTS telemetry_readings_series
                     ON telemetry_readings(astronaut_key, indicator, observed_at);
                 CREATE INDEX IF NOT EXISTS telemetry_readings_astronaut_time
                     ON telemetry_readings(astronaut_key, observed_at, reading_id);
                 CREATE INDEX IF NOT EXISTS telemetry_readings_chain
                     ON telemetry_readings(astronaut_key, indicator, reading_id);
                 CREATE INDEX IF NOT EXISTS telemetry_readings_astronaut_reading
                     ON telemetry_readings(astronaut_key, reading_id);
                 CREATE TRIGGER IF NOT EXISTS telemetry_readings_no_replace
                     BEFORE INSERT ON telemetry_readings
                     WHEN NEW.reading_id IS NOT NULL
                      AND EXISTS (SELECT 1 FROM telemetry_readings WHERE reading_id = NEW.reading_id)
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_readings is append-only');
                     END;
                 CREATE TRIGGER IF NOT EXISTS telemetry_readings_no_update
                     BEFORE UPDATE ON telemetry_readings
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_readings is append-only');
                     END;
                 CREATE TRIGGER IF NOT EXISTS telemetry_readings_no_delete
                     BEFORE DELETE ON telemetry_readings
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_readings is append-only');
                     END;
                 CREATE TABLE IF NOT EXISTS telemetry_heads (
                     astronaut_key TEXT NOT NULL,
                     indicator TEXT NOT NULL,
                     last_reading_id INTEGER NOT NULL,
                     last_hash TEXT NOT NULL,
                     count INTEGER NOT NULL,
                     PRIMARY KEY(astronaut_key, indicator)
                 );
                 CREATE TRIGGER IF NOT EXISTS telemetry_heads_no_delete
                     BEFORE DELETE ON telemetry_heads
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_heads cannot be deleted');
                     END;
                 CREATE TRIGGER IF NOT EXISTS telemetry_heads_forward_only
                     BEFORE UPDATE ON telemetry_heads
                     WHEN NEW.count <= OLD.count
                       OR NEW.last_reading_id <= OLD.last_reading_id
                       OR NEW.astronaut_key != OLD.astronaut_key
                       OR NEW.indicator != OLD.indicator
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_heads can only advance forward');
                     END;
                 PRAGMA user_version=4;",
            )?;
        }
        if version <= 4 {
            // v5 (ronda 3, N2/N4): asegura esquema completo en bases v4 antiguas,
            // rellena `telemetry_heads` sin reescribir hashes ni JSON y bloquea
            // sustituciones (`REPLACE`) en `telemetry_heads` aunque `recursive_triggers=0`.
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS telemetry_readings (
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
                 CREATE INDEX IF NOT EXISTS telemetry_readings_series
                     ON telemetry_readings(astronaut_key, indicator, observed_at);
                 CREATE INDEX IF NOT EXISTS telemetry_readings_astronaut_time
                     ON telemetry_readings(astronaut_key, observed_at, reading_id);
                 CREATE INDEX IF NOT EXISTS telemetry_readings_chain
                     ON telemetry_readings(astronaut_key, indicator, reading_id);
                 CREATE INDEX IF NOT EXISTS telemetry_readings_astronaut_reading
                     ON telemetry_readings(astronaut_key, reading_id);
                 CREATE TRIGGER IF NOT EXISTS telemetry_readings_no_replace
                     BEFORE INSERT ON telemetry_readings
                     WHEN NEW.reading_id IS NOT NULL
                      AND EXISTS (SELECT 1 FROM telemetry_readings WHERE reading_id = NEW.reading_id)
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_readings is append-only');
                     END;
                 CREATE TRIGGER IF NOT EXISTS telemetry_readings_no_update
                     BEFORE UPDATE ON telemetry_readings
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_readings is append-only');
                     END;
                 CREATE TRIGGER IF NOT EXISTS telemetry_readings_no_delete
                     BEFORE DELETE ON telemetry_readings
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_readings is append-only');
                     END;
                 CREATE TABLE IF NOT EXISTS telemetry_heads (
                     astronaut_key TEXT NOT NULL,
                     indicator TEXT NOT NULL,
                     last_reading_id INTEGER NOT NULL,
                     last_hash TEXT NOT NULL,
                     count INTEGER NOT NULL,
                     PRIMARY KEY(astronaut_key, indicator)
                 );
                 CREATE TRIGGER IF NOT EXISTS telemetry_heads_no_delete
                     BEFORE DELETE ON telemetry_heads
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_heads cannot be deleted');
                     END;
                 CREATE TRIGGER IF NOT EXISTS telemetry_heads_forward_only
                     BEFORE UPDATE ON telemetry_heads
                     WHEN NEW.count <= OLD.count
                       OR NEW.last_reading_id <= OLD.last_reading_id
                       OR NEW.astronaut_key != OLD.astronaut_key
                       OR NEW.indicator != OLD.indicator
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_heads can only advance forward');
                     END;
                 INSERT OR IGNORE INTO telemetry_heads (
                     astronaut_key, indicator, last_reading_id, last_hash, count
                 )
                 SELECT
                     agg.astronaut_key,
                     agg.indicator,
                     agg.last_reading_id,
                     last_row.row_hash,
                     agg.row_count
                 FROM (
                     SELECT
                         astronaut_key,
                         indicator,
                         MAX(reading_id) AS last_reading_id,
                         COUNT(*) AS row_count
                     FROM telemetry_readings
                     GROUP BY astronaut_key, indicator
                 ) agg
                 JOIN telemetry_readings last_row
                   ON last_row.reading_id = agg.last_reading_id
                 WHERE NOT EXISTS (
                     SELECT 1
                     FROM telemetry_heads h
                     WHERE h.astronaut_key = agg.astronaut_key
                       AND h.indicator = agg.indicator
                 );
                 CREATE TRIGGER IF NOT EXISTS telemetry_heads_no_replace
                     BEFORE INSERT ON telemetry_heads
                     WHEN EXISTS (
                         SELECT 1 FROM telemetry_heads
                         WHERE astronaut_key = NEW.astronaut_key
                           AND indicator = NEW.indicator
                     )
                     BEGIN
                         SELECT RAISE(ABORT, 'telemetry_heads cannot be replaced');
                     END;
                 PRAGMA user_version=5;",
            )?;
        }
        Ok(())
    }
}

fn migrate_to_v6(tx: &Transaction<'_>) -> Result<(), StoreError> {
    tx.execute_batch(
        "CREATE TABLE pulse_operations (
             key TEXT PRIMARY KEY NOT NULL,
             fingerprint BLOB,
             receipt_json TEXT,
             is_demo INTEGER NOT NULL CHECK(is_demo IN (0, 1)),
             CHECK ((fingerprint IS NOT NULL AND receipt_json IS NOT NULL)
                 OR (is_demo = 1 AND fingerprint IS NULL AND receipt_json IS NULL))
         );
         CREATE INDEX pulse_operations_active_demo ON pulse_operations(is_demo)
             WHERE is_demo = 1 AND receipt_json IS NOT NULL;
         CREATE INDEX records_indicator_history ON records(indicator, measured_at, local_order);
         CREATE INDEX records_replaces ON records(replaces) WHERE replaces IS NOT NULL;
         INSERT INTO sync_state(key, value) VALUES
             ('pulse_store_id', lower(hex(randomblob(16)))),
             ('demo_generation', lower(hex(randomblob(16))));
         PRAGMA user_version=6;",
    )?;
    Ok(())
}

pub(super) fn supported_schema_version(conn: &Connection) -> Result<i64, StoreError> {
    let version = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(StoreError {
            code: "invalid_schema".into(),
            field: None,
            message: "Database version is not supported".into(),
        });
    }
    Ok(version)
}
