# Informe: telemetría local con trazabilidad (parte Rust)

## Qué se hizo

1. **Dependencia `sha2` en el workspace**:
   - Se añadió `sha2 = "0.10"` en `[workspace.dependencies]` de `Cargo.toml` y `sha2.workspace = true` en `crates/store/Cargo.toml`, reutilizando `sha2 0.10.9` ya presente en `Cargo.lock` (actualizando la lista de dependencias de `merken-store` en `Cargo.lock`).

2. **Migración v4 en `crates/store/src/lib.rs`**:
   - Se subió el límite de versión soportada de `version > 3` a `version > 4`.
   - Se añadió el bloque `if version <= 3` que crea `telemetry_readings`, los índices (`telemetry_readings_series`, `telemetry_readings_astronaut_time`, `telemetry_readings_chain`), los dos triggers append-only (`BEFORE UPDATE` y `BEFORE DELETE` con `RAISE(ABORT, 'telemetry_readings is append-only')`) y actualiza `PRAGMA user_version=4`.
   - Se mantuvo `telemetry_readings` completamente aislada de `records`, `events`, `evaluations`, `outbox` y `clear_demo_in`.

3. **Módulo `crates/store/src/telemetry.rs`**:
   - Tipos públicos con `serde` en `snake_case`: `TelemetryReadingInput`, `RecordTelemetryInput`, `RecordTelemetryResult`, `ListTelemetryInput`, `TelemetryItem`, `VerifyTelemetryResult` (reexportados también desde `crates/store/src/lib.rs`).
   - `Store::record_telemetry`: valida de forma atómica lotes de 1 a 64 lecturas sin reglas médicas ni modificación de valores (`astronaut_key` 1–64 caracteres sin controles ni blanco, `astronaut_name` ≤ 100 caracteres sin controles, `indicator` en la lista cerrada de 12 indicadores usando `merken_core::catalog::Indicator::as_str()` en los existentes + `"hrv"` y `"body_battery"`, `unit` ≤ 16 caracteres sin controles, `value` JSON no nulo ≤ 2048 bytes). Si cualquier lectura `i` falla, rechaza todo el lote con `StoreError { code: "invalid_value", field: Some("readings[i].<campo>"), ... }`.
   - Deduplicación por frecuencia dentro de la transacción: consulta la última lectura guardada de la serie `(astronaut_key, indicator)` (`ORDER BY reading_id DESC LIMIT 1`); omite (`skipped`) si coincide valor (y unidad) y han pasado `< 60 000` ms, o inserta (`stored`) con `source = "live_api"`, `batch_id` UUIDv7 (`Store::new_id`) y cadena SHA-256 (`prev_hash` de 64 ceros para la primera lectura de la serie o el `row_hash` anterior).
   - `Store::list_telemetry`: filtros `astronaut_key` (obligatorio), `indicator`, `from`, `to` y `limit` (por defecto 200, acotado en `1..=1000`), ordenado del más reciente al más antiguo (`ORDER BY observed_at DESC, reading_id DESC`).
   - `Store::verify_telemetry`: recorre las lecturas por `reading_id ASC` (todas o filtradas por `astronaut_key`), verifica continuidad de `prev_hash`, `source == "live_api"`, validez del JSON y recalcula `row_hash`, devolviendo `VerifyTelemetryResult { ok, checked, first_bad_reading_id }`.
   - 9 tests unitarios en `telemetry::tests` cubriendo todos los casos obligatorios (inserción/listado/filtros/límites, regla de 60 s e intra-lote, atomicidad con índice de error, triggers `UPDATE`/`DELETE`, independencia de `load_demo`/`clear_demo`/escrituras reales, verificación tras desactivar triggers y alterar/borrar filas, ausencia de filas en `outbox`, migración v3 → v4 e idempotencia al reabrir + rechazo v5, y cadenas independientes entre astronautas).

4. **Comandos Tauri en `src-tauri/src/lib.rs`**:
   - Añadidos y registrados en `generate_handler!`: `record_telemetry`, `list_telemetry` y `verify_telemetry` (usando `with_store`, `now()` y `APP_VERSION`), además del test `telemetry_round_trip_and_verify_input_parsing`.

5. **Documentación en `docs/api/telemetria-local.md`**:
   - Documentación técnica del esquema v4, reglas de validación/deduplicación, formato exacto de hash y contrato de comandos.

## Archivos tocados

- `Cargo.toml` (modificado: dependencia `sha2 = "0.10"` en `[workspace.dependencies]`)
- `Cargo.lock` (modificado: `"sha2"` añadido a dependencias de `merken-store`)
- `crates/store/Cargo.toml` (modificado: `sha2.workspace = true`)
- `crates/store/src/lib.rs` (modificado: `pub mod telemetry`, reexports, límite `version > 4` y migración v4)
- `crates/store/src/telemetry.rs` (nuevo)
- `src-tauri/src/lib.rs` (modificado: 3 comandos, `VerifyTelemetryInput`, registro en `generate_handler!` y test)
- `docs/api/telemetria-local.md` (nuevo)
- `reviews/telemetria-local-rust-informe.md` (este informe)

## Decisiones fuera de la especificación literal

- **Formato del hash `row_hash`**: se eligió prefijo de dominio `"merken-telemetry-v1"` y prefijo de longitud de 8 bytes big-endian (`u64::to_be_bytes`) delante de cada uno de los 9 campos especificados (`astronaut_key`, `indicator`, `value_json`, `unit`, `observed_at` en `i64::to_be_bytes`, `batch_id`, `source`, `app_version`, `prev_hash`) para eliminar cualquier colisión incluso si algún campo contuviera delimitadores.
- **Cláusulas `IF NOT EXISTS` en la migración v4**: el test existente `flow_tests::migration_v3_parks_pending_headache_outbox` abre una base ya migrada, retrocede manualmente `PRAGMA user_version = 2` y vuelve a llamar `Store::initialize`. Usar `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS` y `CREATE TRIGGER IF NOT EXISTS` en v4 evita fallo cuando las estructuras v4 ya existen tras ese retroceso artificial de versión.
- **Deserialización de `verify_telemetry` en Tauri (`VerifyTelemetryInput`)**: alineado con `ui/src/core-api.ts` (`invoke("verify_telemetry", { input })` con `{ astronaut_key?: string }`), el comando recibe `input: Option<VerifyTelemetryInput>` mientras `Store::verify_telemetry` mantiene exactamente la firma `verify_telemetry(&self, astronaut_key: Option<String>)`.
- **Rechazo de `Value::Null` y control de `astronaut_name`/`unit`**: se rechaza `value: null` (en el modelo de datos `null` significa ausencia de dato) y caracteres de control en `astronaut_name`/`unit`, pero se permiten cadenas vacías en `astronaut_name` y `unit` para no descartar lecturas válidas de indicadores como `mood` o `body_battery` cuando lleguen sin unidad o sin nombre.
- **Índices adicionales en `telemetry_readings`**: además del índice `(astronaut_key, indicator, observed_at)`, se añadieron `(astronaut_key, observed_at, reading_id)` y `(astronaut_key, indicator, reading_id)` para acelerar `list_telemetry` sin filtro de indicador y la búsqueda del último eslabón por `reading_id DESC`.
- **Alcance de `verify_telemetry` al detectar corrupción**: recorre todas las filas del filtro en orden `reading_id ASC`, mantiene en `checked` el total inspeccionado y reporta en `first_bad_reading_id` el menor `reading_id` cuya fila o enlace falla.

## Qué no se pudo hacer

- **`cargo fmt --check` global en verde**: falla exclusivamente por diferencias previas en `crates/core/src/telemetry.rs`, archivo que las reglas prohíben modificar. En los paquetes tocados (`cargo fmt --check -p merken-store -p merken`), la salida es limpia (código 0).
- **Ejecución dentro del sandbox de `cargo test --workspace` y `cargo clippy ... -p merken`**: en Windows, el sandbox deniega escritura en `target/` (`Access is denied`, necesario para `.cargo-build-lock` y el test previo `migration_can_be_reopened`) y bloquea la macro `tauri::generate_context!()` de `src-tauri` (`unable to access current working directory: Access is denied`). Ambos comandos se ejecutaron fuera del sandbox (`BypassSandbox: true`).
- **Tests de integración en vivo contra red**: los 2 tests previos `crates/sync/tests/live_simulation.rs` permanecen ignorados (`necesita red`).

## Salida resumida de verificación

- `rustfmt --edition 2024 --check crates\store\src\lib.rs crates\store\src\telemetry.rs src-tauri\src\lib.rs`: código `0`, sin diferencias.
- `cargo fmt --check -p merken-store -p merken`: código `0`, sin diferencias.
- `cargo fmt --check`: código `1`, únicamente por formato previo en `crates/core/src/telemetry.rs`.
- `cargo clippy --locked --offline -p merken-store -p merken --all-targets -- -D warnings`: código `0`, sin avisos ni errores.
- `cargo test --locked --offline --workspace`: código `0`, **75 pasados, 0 fallidos, 2 ignorados** (los 2 de red en `crates/sync/tests/live_simulation.rs`):
  - `merken` (`merken_lib`): `4 passed; 0 failed` (incluye `telemetry_round_trip_and_verify_input_parsing`).
  - `merken` (`main.rs`): `0 passed; 0 failed`.
  - `merken-core`: `24 passed; 0 failed`.
  - `merken-seed`: `2 passed; 0 failed`.
  - `merken-slm`: `0 passed; 0 failed`.
  - `merken-store`: `39 passed; 0 failed` (incluye los 9 tests de `telemetry::tests`, `flow_tests::migration_v3_parks_pending_headache_outbox` y `tests::migration_can_be_reopened`).
  - `merken-sync`: `6 passed; 0 failed` (y `2 ignored` en `live_simulation`).


## Correcciones tras la revisión de Sol

### Resumen de cambios implementados

1. **Hallazgo 2 (`INSERT OR REPLACE` / `REPLACE INTO`):**
   - Se activó `PRAGMA recursive_triggers = ON` tanto en `Store::open` como en `Store::open_in_memory` en `crates/store/src/lib.rs`.
   - En el bloque `v4` se añadió el trigger `telemetry_readings_no_replace` (`BEFORE INSERT WHEN NEW.reading_id IS NOT NULL AND EXISTS (SELECT 1 FROM telemetry_readings WHERE reading_id = NEW.reading_id)`) que ejecuta `RAISE(ABORT, 'telemetry_readings is append-only')`.
   - **Nota sobre versión de esquema:** Como `v4` todavía no se ha publicado ni ejecutado en ninguna base de datos real, se editó directamente el bloque `if version <= 3` manteniendo `PRAGMA user_version=4` en lugar de crear una migración `v5`.
   - Se añadió regresión con `INSERT OR REPLACE` y `REPLACE INTO` en `append_only_triggers_and_replace_protection` (`crates/store/src/telemetry.rs`), además de validar `PRAGMA recursive_triggers = 1` tanto en memoria como en apertura sobre disco (`migration_v3_to_v4_preserves_data_and_reopen_is_idempotent`).

2. **Hallazgo 3 (truncamiento y borrado de series con `telemetry_heads`):**
   - Se añadió la tabla `telemetry_heads(astronaut_key TEXT NOT NULL, indicator TEXT NOT NULL, last_reading_id INTEGER NOT NULL, last_hash TEXT NOT NULL, count INTEGER NOT NULL, PRIMARY KEY(astronaut_key, indicator))` al bloque `v4`, junto con los triggers `telemetry_heads_no_delete` (`BEFORE DELETE` aborta) y `telemetry_heads_forward_only` (`BEFORE UPDATE` aborta si `NEW.count <= OLD.count`, `NEW.last_reading_id <= OLD.last_reading_id`, o si cambian `astronaut_key` o `indicator`).
   - `Store::record_telemetry` actualiza `telemetry_heads` en la misma transacción mediante `INSERT ... ON CONFLICT(astronaut_key, indicator) DO UPDATE`.
   - Se amplió `VerifyTelemetryResult` a `{ ok, checked, series_checked, first_bad_reading_id, problem }`, donde `problem` toma valores en `{ "hash_mismatch", "broken_link", "missing_tail", "missing_series", "count_mismatch" }`.
   - `Store::verify_telemetry` contrasta cada serie (o solo las del `astronaut_key` indicado) contra su cabeza (`last_reading_id`, `last_hash` y `count`), devolviendo `missing_series` si existe una cabeza sin filas en `telemetry_readings`.
   - Se documentó con honestidad en `docs/api/telemetria-local.md` que el mecanismo detecta ediciones accidentales, truncamientos de cola y borrados parciales o de series completas, pero no protege frente a alguien con acceso de escritura completo al archivo SQLite (requeriría exportar/anclar `last_hash` fuera del dispositivo).
   - Tests en `verify_detects_tampering_truncation_and_missing_series`: caso sano, alteración de valor (`hash_mismatch`), ruptura de enlace (`broken_link`), truncamiento de cola (`missing_tail`), borrado de serie completa (`missing_series`) y discrepancia de conteo (`count_mismatch`).

3. **Hallazgo 6 (crecimiento de BD y plan de consulta):**
   - Se sustituyó la regla de deduplicación por las constantes públicas documentadas `MIN_CHANGE_INTERVAL_MS = 10_000` y `HEARTBEAT_MS = 60_000`: solo se guarda si cambia valor/unidad y pasaron ≥ `10_000` ms, o si pasaron ≥ `60_000` ms desde la última lectura guardada de esa serie.
   - Se actualizó `Store::list_telemetry` para ordenar por `reading_id DESC` (monótono por inserción) en lugar de `observed_at DESC`.
   - Se añadió el índice `telemetry_readings_astronaut_reading ON telemetry_readings(astronaut_key, reading_id)` en `v4` (complementando a `telemetry_readings_chain (astronaut_key, indicator, reading_id)`), y se añadió el test de regresión `explain_query_plan_uses_indexes_without_temp_btree`.
   - Se documentó en `docs/api/telemetria-local.md` el cálculo analítico de crecimiento diario esperado por astronauta (~17 280 filas/día ≈ ~11 MB/día en escenario estable; cota máxima ~103 680 filas/día ≈ ~66 MB/día frente a las ~518 400 filas/día anteriores) y que archivar/podar queda fuera de esta entrega.

   **Planes de consulta (`EXPLAIN QUERY PLAN`) antes y después:**

   - **Antes** (sin `telemetry_readings_astronaut_reading`):
     - `WHERE astronaut_key = ?1 ORDER BY reading_id DESC LIMIT ?2`:
       `SEARCH telemetry_readings USING INDEX telemetry_readings_astronaut_time (astronaut_key=?)` + `USE TEMP B-TREE FOR ORDER BY`
     - `WHERE astronaut_key = ?1 AND indicator = ?2 ORDER BY reading_id DESC LIMIT ?3`:
       `SEARCH telemetry_readings USING INDEX telemetry_readings_chain (astronaut_key=? AND indicator=?)` (sin tabla temporal)
     - `WHERE astronaut_key = ?1 ORDER BY reading_id ASC` (`verify_telemetry`):
       `SEARCH telemetry_readings USING INDEX telemetry_readings_astronaut_time (astronaut_key=?)` + `USE TEMP B-TREE FOR ORDER BY`
     - Con filtro de rango `observed_at >= ? AND observed_at <= ?` y `ORDER BY reading_id DESC`:
       `SEARCH telemetry_readings USING INDEX telemetry_readings_astronaut_time / telemetry_readings_series` + `USE TEMP B-TREE FOR ORDER BY`
   - **Después** (con `telemetry_readings_astronaut_reading (astronaut_key, reading_id)`):
     - `WHERE astronaut_key = ?1 ORDER BY reading_id DESC LIMIT ?2`:
       `SEARCH telemetry_readings USING INDEX telemetry_readings_astronaut_reading (astronaut_key=?)` (**sin `TEMP B-TREE`**)
     - `WHERE astronaut_key = ?1 AND indicator = ?2 ORDER BY reading_id DESC LIMIT ?3`:
       `SEARCH telemetry_readings USING INDEX telemetry_readings_chain (astronaut_key=? AND indicator=?)` (**sin `TEMP B-TREE`**)
     - `WHERE astronaut_key = ?1 ORDER BY reading_id ASC` (`verify_telemetry` con `astronaut_key`):
       `SEARCH telemetry_readings USING INDEX telemetry_readings_astronaut_reading (astronaut_key=?)` (**sin `TEMP B-TREE`**)
     - `verify_telemetry(None)` (`ORDER BY reading_id ASC`):
       `SCAN telemetry_readings` (orden nativo por PK `reading_id`)
     - Con filtro explícito `from`/`to` sobre `observed_at`, SQLite mantiene el uso de los índices de tiempo (`telemetry_readings_astronaut_time` / `telemetry_readings_series`) para acotar la ventana antes de ordenar por `reading_id DESC`.

4. **Dudas de contrato:**
   - Se validó la forma exacta de `value` para los 12 indicadores (`heart_rate {bpm}`, `oxygen_in_blood {percentage}`, `blood_pressure {systolic, diastolic}`, `glucose {mg_dl}`, `body_temperature {celsius}`, `sleep {hours}`, `nutrition {kcal}`, `space_radiation {value}`, `distance_from_earth {value}`, `mood {label}` con 1–64 caracteres no blancos y sin controles, `hrv {ms}`, `body_battery {value}`), sin rangos médicos (solo números finitos) y señalando `readings[i].value` o `readings[i].value.<clave>`. Se corrigieron también los fixtures de prueba que usaban `usv_h`/`km` para adaptarlos a `{value}`.
   - Se implementó igualdad numérica por `f64` (`72 == 72.0`) y estructural para cadenas/objetos en `values_equal`.
   - `Store::list_telemetry` valida `input.indicator` contra `ALLOWED_INDICATORS` y devuelve `invalid_value` con `field: "indicator"` si es desconocido.

5. **Hallazgo 7 (tests adicionales):**
   - `record_telemetry_rolls_back_rows_and_heads_on_sql_error` en `crates/store/src/telemetry.rs` usa un trigger temporal que aborta la segunda inserción de un lote y comprueba que no quedan ni la primera fila ni su entrada en `telemetry_heads` (ni altera una serie previa existente).
   - `telemetry_round_trip_and_verify_command_paths` en `src-tauri/src/lib.rs` extrae y prueba `handle_verify_telemetry` sin `input` (`None`), con `input: {}` y con `astronaut_key`, además de comprobar la serialización JSON completa de `VerifyTelemetryResult`.

### Verificación y bloqueos de permisos observados

- **Bloqueos dentro del sandbox de terminal (`BypassSandbox: false`):**
  - `cargo test` sin `--target-dir` falló al abrir `target\debug\.cargo-build-lock` (`Access is denied. (os error 5)`).
  - En `src-tauri`, la macro `tauri::generate_context!()` falló dentro del sandbox con `unable to access current working directory: Access is denied. (os error 5)`.
  - `tests::migration_can_be_reopened` (que crea su archivo en `../../target/merken-store-tests`) falló dentro del sandbox con `storage_error`, y 2 tests de `merken-sync` que abren sockets loopback (`127.0.0.1`) fallaron con error `"network"` por el aislamiento de red del sandbox.
  - `rustfmt` en modo escritura devolvió `Access is denied (os error 5)` dentro del sandbox al intentar sobrescribir archivos; se aplicó el formato mediante `edit_file`.
- **Resultados tras el último cambio:**
  - `cargo test --offline --target-dir <scratch>\target --workspace` (ejecutado con aprobación fuera del sandbox): **en verde** (`merken_lib`: 4 passed; `merken_core`: 24 passed; `merken_seed`: 2 passed; `merken_slm`: 0; `merken_store`: 42 passed; `merken_sync`: 6 passed + 2 `#[ignore]` de red en `tests/live_simulation.rs`).
  - `cargo clippy --offline --target-dir <scratch>\target --workspace --all-targets -- -D warnings` (fuera del sandbox): **limpio** (código 0, sin warnings).
  - `rustfmt --edition 2024 --check crates/store/src/telemetry.rs crates/store/src/lib.rs src-tauri/src/lib.rs`: **limpio** (código 0). *(Nota: `cargo fmt --check` a nivel global reporta formato previo sin tocar en `crates/core/src/telemetry.rs`, archivo fuera del alcance permitido.)*


## Ronda 3

### Cambios realizados

1. **N1 — Contrato de valores (`space_radiation` y `distance_from_earth`)**
   - En `crates/store/src/telemetry.rs` (`expected_numeric_keys`), `space_radiation` ahora exige exactamente la clave `usv_h` y `distance_from_earth` exactamente `km` (números finitos); el resto de indicadores (`hrv {ms}`, `body_battery {value}`, etc.) se mantiene intacto.
   - Se actualizaron los tests existentes (`insert_list_order_filters_and_max_limit` y `value_shape_validation_covers_all_indicators`, verificando ahora el rechazo de `{"value": ...}` con `field` en `readings[0].value.usv_h` y `readings[0].value.km`) y la documentación en `docs/api/telemetria-local.md`.
   - Se añadió el test `telemetry::tests::fixture_batch_records_all_twelve_readings_without_skips`, que carga `docs/api/fixtures/telemetry-record-batch.json` con `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/api/fixtures/telemetry-record-batch.json"))`, deserializa `readings` como `Vec<TelemetryReadingInput>` y los guarda con `record_telemetry`: se guardan las 12 lecturas (`stored = 12`, `skipped = 0`) y `verify_telemetry(None)` devuelve `ok = true`. No se modificó el fixture compartido; sus 12 lecturas coinciden con el contrato de `crates/core` y `crates/store`.

2. **N2 — Migración v5 idempotente (v3→v4→v5, v4 antigua y v4 ampliada)**
   - En `crates/store/src/lib.rs` (`Store::initialize`), el límite de `user_version` sube a `5` (`if version > 5`) y se añade el bloque `if version <= 4` que actualiza a `PRAGMA user_version=5`.
   - La migración usa `CREATE TABLE/INDEX/TRIGGER IF NOT EXISTS` para asegurar `telemetry_readings`, sus cuatro índices (incluidos `telemetry_readings_chain` y `telemetry_readings_astronaut_reading`), los triggers de `telemetry_readings` (incluido `telemetry_readings_no_replace`), `telemetry_heads` y sus triggers (`telemetry_heads_no_delete`, `telemetry_heads_forward_only`).
   - Antes de crear el trigger `telemetry_heads_no_replace`, rellena `telemetry_heads` desde `telemetry_readings` con `INSERT OR IGNORE ... SELECT ... MAX(reading_id), row_hash del último, COUNT(*) ... GROUP BY astronaut_key, indicator` (con `WHERE NOT EXISTS` respecto a `telemetry_heads`), sin reescribir `row_hash` ni `value_json`, y sin fallar si alguna serie previa tiene la cadena rota (lo reporta `verify_telemetry`).
   - Tests añadidos/actualizados en `crates/store/src/telemetry.rs`:
     - `migration_v3_to_v5_preserves_data_and_reopen_is_idempotent`: v3 con datos → v5, reapertura idempotente y rechazo de v6 con `invalid_schema`.
     - `migration_old_v4_to_v5_backfills_heads_and_is_idempotent`: base con el DDL v4 antiguo y lecturas previas (incluido JSON heredado) → v5, relleno correcto de `telemetry_heads` sin reescribir hashes ni JSON, `record_telemetry` y `verify_telemetry` con `ok = true`, y reapertura idempotente.
     - `migration_expanded_v4_to_v5_is_idempotent_and_protects_heads`: v4 ampliada → v5 idempotente con `telemetry_heads_no_replace` activo.
     - `migration_old_v4_with_incoherent_chain_migrates_and_verify_reports`: v4 antigua con `prev_hash` roto migra sin error a v5 y `verify_telemetry` reporta `ok = false` y `problem = "broken_link"`.

3. **N4 — Protección de `telemetry_heads` con `PRAGMA recursive_triggers=0`**
   - En `crates/store/src/telemetry.rs` (`record_telemetry`), se sustituyó el `INSERT ... ON CONFLICT(astronaut_key, indicator) DO UPDATE` sobre `telemetry_heads` por `UPDATE telemetry_heads SET last_reading_id = ?3, last_hash = ?4, count = count + 1 WHERE astronaut_key = ?1 AND indicator = ?2`, ejecutando `INSERT INTO telemetry_heads ... VALUES (?1, ?2, ?3, ?4, 1)` solo cuando el `UPDATE` afecta `0` filas, dentro de la misma transacción.
   - En `crates/store/src/lib.rs` (migración v5), se añade el trigger `telemetry_heads_no_replace` (`BEFORE INSERT ON telemetry_heads`) que ejecuta `RAISE(ABORT, 'telemetry_heads cannot be replaced')` si ya existe `(astronaut_key, indicator)`.
   - Se actualizó `append_only_triggers_and_replace_protection` y se añadió `append_only_matrix_with_recursive_triggers_off_and_on` cubriendo la matriz completa `PRAGMA recursive_triggers` `{0, 1}` × `{telemetry_readings, telemetry_heads}` × `{REPLACE INTO, INSERT OR REPLACE, UPDATE, DELETE}`, comprobando el bloqueo en los 16 casos y el avance legítimo de cabeza mediante `record_telemetry` en ambos modos.

4. **Pruebas pendientes (7a — despachador Tauri)**
   - Se verificó con `cargo tree -p merken --edges features -i tauri` que `tauri v2.12.5` en `src-tauri` solo compila con sus features por defecto y no habilita la feature `"test"`, por lo que `tauri::test` (`mock_builder`, `get_ipc_response`) no es viable sin editar `src-tauri/Cargo.toml` (fuera de los archivos permitidos). No se forzó.
   - En `src-tauri/src/lib.rs`, se amplió `tests::telemetry_round_trip_and_verify_command_paths` para cubrir también `space_radiation {usv_h}`, `distance_from_earth {km}`, filtrado `ListTelemetryInput` con fechas RFC 3339 (`from: "2027-01-15T08:00:05Z"`, `to: "2027-01-15T09:00:15+01:00"`) y las tres formas de entrada en `handle_verify_telemetry` (`None`, `"{}"` y `{"astronaut_key":"astro-1"}`).

### Verificación y notas de entorno

- `cargo test --workspace` sin `--target-dir` falló al abrir `target/debug/.cargo-build-lock` (`Access is denied`).
- Dentro del sandbox estándar, `tauri::generate_context!()` en `src-tauri` falló con `unable to access current working directory: Access is denied. (os error 5)`, `tests::migration_can_be_reopened` falló al escribir en `target/merken-store-tests` y 2 tests loopback de `merken-sync` fallaron por restricción de red del sandbox.
- Con `--target-dir` en scratch y ejecución fuera del sandbox (`BypassSandbox`):
  - `cargo test --workspace --target-dir ...`: verde (`merken_lib`: 4 passed; `merken_core`: 24 passed; `merken_seed`: 2 passed; `merken_slm`: 0 passed; `merken_store`: 47 passed; `merken_sync`: 6 passed + 2 ignored en `live_simulation` por requerir red y `MERKEN_SIM_TOKEN`).
  - `cargo clippy --workspace --all-targets --target-dir ... -- -D warnings`: limpio (0 warnings).
  - `rustfmt --edition 2024 --check crates/store/src/telemetry.rs crates/store/src/lib.rs src-tauri/src/lib.rs`: limpio.
  - Revisión del diff en los archivos tocados: sin `println!`, `dbg!` ni tests temporales (`temp_*`).

## Ronda 4

- Se añadió `checked_through_reading_id`, máximo de los IDs de las lecturas recorridas, con filtro por astronauta y `null` sin lecturas. Se documentó y se agregaron pruebas de vacío, máximo, nueva inserción y alcance por astronauta. Las expectativas existentes se actualizaron.
- Antes, `verify_telemetry` cargaba cabezas y lecturas mediante consultas separadas sin transacción común. Ahora usa `unchecked_transaction` (deferred), termina la consulta y confirma la transacción; los errores revierten al descartarla.
- No se implementó ni ejecutó una prueba real del despachador IPC. El ejecutor de comandos fue rechazado por Windows (`CreateProcessAsUserW`, código 5, Access is denied); un intento alternativo de ejecutar Cargo devolvió `spawnSync cargo EPERM`. Por ello no se pudo determinar si la versión y las features actuales de Tauri exponen `tauri::test`, ni comprobar la fixture o compilar un ensayo. No se añadieron features ni dependencias. La prueba existente sigue siendo de handlers, no del despachador IPC.
- `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` y `rustfmt --edition 2024 --check` no se pudieron iniciar por el mismo rechazo de permisos. No se obtuvo un error de acceso a `target/`: el proceso ni siquiera arrancó. No se puede declarar la ronda terminada ni los cambios verificados.
- Inspección textual de los archivos Rust modificados: no se encontraron funciones de prueba `temp_*`; no se añadieron `println!` ni `dbg!`. No se inició ningún proceso de fondo.
