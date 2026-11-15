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
