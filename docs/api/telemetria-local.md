# Telemetría local con trazabilidad (SQLite + Tauri)

Referencia de implementación: [`crates/store/src/telemetry.rs`](../../crates/store/src/telemetry.rs), [`crates/store/src/lib.rs`](../../crates/store/src/lib.rs) (migración v4) y [`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs). Contrato TypeScript: [`ui/src/core-api.ts`](../../ui/src/core-api.ts).

## Propósito y aislamiento

La interfaz recibe cada 2 s el estado actual de la tripulación desde la API de desarrollo (`/api/astronauts`, tabla MySQL `astronauts`, sin historial ni hora de medición). Este módulo guarda ese historial en SQLite en el dispositivo con trazabilidad por astronauta e indicador.

- Vive en una tabla nueva y separada: `telemetry_readings`.
- **No** escribe en `records`, `events`, `evaluations` ni `outbox`. El contrato del backend prohíbe publicar datos externos desde el móvil y esta telemetría es exclusivamente local.
- **No** se borra con `clear_demo`, `load_demo` ni al guardar mediciones reales (`clear_demo_in`).
- **Sin reglas médicas ni correcciones:** se persiste el valor recibido tal cual tras validar únicamente la forma de los campos.

## Esquema SQLite (migración v4, `PRAGMA user_version = 4`)

### Tabla `telemetry_readings`

| Columna | Tipo | Notas |
|---|---|---|
| `reading_id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | Orden de inserción de la cadena |
| `astronaut_key` | `TEXT NOT NULL` | Identificador textual de la API (1–64 caracteres sin controles) |
| `astronaut_name` | `TEXT NOT NULL` | Nombre legible (≤ 100 caracteres sin controles) |
| `indicator` | `TEXT NOT NULL` | Indicador dentro de la lista cerrada |
| `value_json` | `TEXT NOT NULL` | JSON serializado del valor recibido (≤ 2048 bytes UTF-8) |
| `unit` | `TEXT NOT NULL` | Unidad textual (≤ 16 caracteres sin controles) |
| `observed_at` | `INTEGER NOT NULL` | Instante UTC en ms proporcionado por el reloj de la app (`src-tauri`) |
| `batch_id` | `TEXT NOT NULL` | UUIDv7 generado con `Store::new_id(now)` por lote |
| `source` | `TEXT NOT NULL` | Siempre `'live_api'` |
| `app_version` | `TEXT NOT NULL` | Versión de la app (`CARGO_PKG_VERSION`) |
| `prev_hash` | `TEXT NOT NULL` | `row_hash` anterior de la serie `(astronaut_key, indicator)`, o 64 ceros |
| `row_hash` | `TEXT NOT NULL` | SHA-256 en hex minúsculas (64 caracteres) |

### Índices y protección append-only

- `telemetry_readings_series (astronaut_key, indicator, observed_at)`
- `telemetry_readings_astronaut_time (astronaut_key, observed_at, reading_id)`
- `telemetry_readings_chain (astronaut_key, indicator, reading_id)`
- Triggers `BEFORE UPDATE` (`telemetry_readings_no_update`) y `BEFORE DELETE` (`telemetry_readings_no_delete`): ejecutan `RAISE(ABORT, 'telemetry_readings is append-only')`.

## Validación y deduplicación

### Validación por lote (atómica)

- El lote debe tener entre `1` y `64` lecturas; si no, falla con `invalid_value` en `field: "readings"`.
- Si cualquier lectura `i` falla validación, se rechaza todo el lote sin escribir ninguna fila y `StoreError.field` señala `"readings[i].<campo>"`:
  - `astronaut_key`: 1–64 caracteres, no vacío/blanco y sin caracteres de control.
  - `astronaut_name`: ≤ 100 caracteres sin caracteres de control.
  - `indicator`: lista cerrada de 12 indicadores:
    `heart_rate`, `oxygen_in_blood`, `blood_pressure`, `glucose`, `body_temperature`, `sleep`, `nutrition`, `space_radiation`, `distance_from_earth`, `mood`, `hrv`, `body_battery`.
  - `unit`: ≤ 16 caracteres sin caracteres de control.
  - `value`: JSON no nulo cuyo `serde_json::to_string` ocupa ≤ 2048 bytes.

### Regla de frecuencia (60 s)

Dentro de la misma transacción que inserta cada lectura se consulta la última fila guardada de su serie `(astronaut_key, indicator)`:

- Si no existe ninguna, se guarda con `prev_hash = "0000...0000"` (64 ceros).
- Si existe y el valor (y unidad) coincide y `observed_at - prev_observed_at < 60_000` ms, se omite (`skipped += 1`).
- Si cambia el valor (o unidad), o si `observed_at - prev_observed_at >= 60_000` ms, se inserta encadenando `prev_hash = last_row_hash` (`stored += 1`).

## Cadena de hashes SHA-256

Cada serie `(astronaut_key, indicator)` mantiene una cadena independiente. El `row_hash` (64 hex minúsculas) se calcula sobre el prefijo de dominio `"merken-telemetry-v1"` seguido de los 9 campos, cada uno prefijado con su longitud en bytes como entero sin signo de 64 bits big-endian (`u64::to_be_bytes`):

1. `astronaut_key` (UTF-8)
2. `indicator` (UTF-8)
3. `value_json` (UTF-8 exacto guardado en SQLite)
4. `unit` (UTF-8)
5. `observed_at` (`i64::to_be_bytes`, 8 bytes)
6. `batch_id` (UTF-8)
7. `source` (UTF-8, `'live_api'`)
8. `app_version` (UTF-8)
9. `prev_hash` (UTF-8)

## Comandos Tauri y métodos de `Store`

| Comando Tauri | Entrada (`{ input }`) | Salida |
|---|---|---|
| `record_telemetry` | `{ readings: TelemetryReadingInput[] }` | `{ stored, skipped, batch_id }` |
| `list_telemetry` | `{ astronaut_key, indicator?, from?, to?, limit? }` | `TelemetryItem[]` (orden `observed_at DESC, reading_id DESC`; `limit` por defecto `200`, máximo `1000`) |
| `verify_telemetry` | `{ astronaut_key? }` (opcional) | `{ ok, checked, first_bad_reading_id }` |
