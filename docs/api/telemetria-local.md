# Telemetría local con trazabilidad (SQLite + Tauri)

Referencia de implementación: [`crates/store/src/telemetry.rs`](../../crates/store/src/telemetry.rs), [`crates/store/src/lib.rs`](../../crates/store/src/lib.rs) (migraciones v4 y v5) y [`src-tauri/src/lib.rs`](../../src-tauri/src/lib.rs). Contrato TypeScript: [`ui/src/core-api.ts`](../../ui/src/core-api.ts).

## Propósito y aislamiento

La interfaz recibe cada 2 s el estado actual de la tripulación desde la API de desarrollo (`/api/astronauts`, tabla MySQL `astronauts`, sin historial ni hora de medición). Este módulo guarda ese historial en SQLite en el dispositivo con trazabilidad por astronauta e indicador.

- Vive en dos tablas separadas: `telemetry_readings` y `telemetry_heads`.
- **No** escribe en `records`, `events`, `evaluations` ni `outbox`. El contrato del backend prohíbe publicar datos externos desde el móvil y esta telemetría es exclusivamente local.
- **No** se borra con `clear_demo`, `load_demo` ni al guardar mediciones reales (`clear_demo_in`).
- **Sin reglas médicas ni correcciones:** se persiste el valor recibido tal cual tras validar únicamente la forma de los campos.

## Esquema SQLite (migración v5, `PRAGMA user_version = 5`)

`Store::open` y `Store::open_in_memory` activan `PRAGMA foreign_keys = ON` y `PRAGMA recursive_triggers = ON` en cada conexión.

La migración **v5** es idempotente desde v3 (v3→v4→v5), desde una v4 antigua (sin `telemetry_heads`, sin `telemetry_readings_no_replace` ni los índices de cadena) y desde una v4 ampliada:

- Usa `CREATE TABLE/INDEX/TRIGGER IF NOT EXISTS` para garantizar todas las tablas, índices y triggers.
- Rellena `telemetry_heads` desde `telemetry_readings` con `INSERT OR IGNORE` (`max(reading_id)`, `row_hash` del último registro y `count(*)`) agrupado por `(astronaut_key, indicator)`, sin reescribir `row_hash` ni `value_json`, y sin fallar si alguna serie existente tiene la cadena dañada (en ese caso `verify_telemetry` lo reportará después).
- Añade el trigger `telemetry_heads_no_replace` (`BEFORE INSERT`) una vez completado el relleno inicial.

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

### Tabla `telemetry_heads`

Registra la cabeza más reciente de cada serie `(astronaut_key, indicator)` y se actualiza en la **misma transacción** que cada inserción: primero intenta `UPDATE telemetry_heads` sobre la cabeza existente y, si no existe (`0` filas afectadas), ejecuta `INSERT INTO telemetry_heads` con `count = 1`:

| Columna | Tipo | Notas |
|---|---|---|
| `astronaut_key` | `TEXT NOT NULL` | Parte de la clave primaria compuesta |
| `indicator` | `TEXT NOT NULL` | Parte de la clave primaria compuesta |
| `last_reading_id` | `INTEGER NOT NULL` | Último `reading_id` insertado en la serie |
| `last_hash` | `TEXT NOT NULL` | Último `row_hash` insertado en la serie |
| `count` | `INTEGER NOT NULL` | Número acumulado de filas guardadas en la serie |

### Índices y protección append-only

- `telemetry_readings_series (astronaut_key, indicator, observed_at)`
- `telemetry_readings_astronaut_time (astronaut_key, observed_at, reading_id)`
- `telemetry_readings_chain (astronaut_key, indicator, reading_id)` — cubre `record_telemetry` y `list_telemetry` filtrando por `indicator` en orden `reading_id DESC` sin tabla temporal.
- `telemetry_readings_astronaut_reading (astronaut_key, reading_id)` — cubre `list_telemetry` sin `indicator` (`reading_id DESC`) y `verify_telemetry` por astronauta (`reading_id ASC`) sin tabla temporal.
- Triggers sobre `telemetry_readings`:
  - `telemetry_readings_no_replace` (`BEFORE INSERT`): aborta si `NEW.reading_id` no es nulo y ya existe en la tabla, impidiendo sustituciones con `INSERT OR REPLACE` / `REPLACE INTO` incluso con `PRAGMA recursive_triggers = 0`.
  - `telemetry_readings_no_update` (`BEFORE UPDATE`) y `telemetry_readings_no_delete` (`BEFORE DELETE`): ejecutan `RAISE(ABORT, 'telemetry_readings is append-only')`.
- Triggers sobre `telemetry_heads`:
  - `telemetry_heads_no_replace` (`BEFORE INSERT`): aborta si ya existe una fila para `(astronaut_key, indicator)`, impidiendo sustituciones con `INSERT OR REPLACE` / `REPLACE INTO` incluso con `PRAGMA recursive_triggers = 0`.
  - `telemetry_heads_no_delete` (`BEFORE DELETE`): aborta borrados.
  - `telemetry_heads_forward_only` (`BEFORE UPDATE`): aborta si `NEW.count <= OLD.count`, `NEW.last_reading_id <= OLD.last_reading_id` o si se intenta cambiar `astronaut_key` o `indicator`.

## Validación y deduplicación

### Validación por lote (atómica)

- El lote debe tener entre `1` y `64` lecturas; si no, falla con `invalid_value` en `field: "readings"`.
- Si cualquier lectura `i` falla validación, se rechaza todo el lote sin escribir ninguna fila ni cabeza y `StoreError.field` señala `"readings[i].<campo>"`:
  - `astronaut_key`: 1–64 caracteres, no vacío/blanco y sin caracteres de control.
  - `astronaut_name`: ≤ 100 caracteres sin caracteres de control.
  - `indicator`: lista cerrada de 12 indicadores:
    `heart_rate`, `oxygen_in_blood`, `blood_pressure`, `glucose`, `body_temperature`, `sleep`, `nutrition`, `space_radiation`, `distance_from_earth`, `mood`, `hrv`, `body_battery`.
  - `unit`: ≤ 16 caracteres sin caracteres de control.
  - `value`: objeto JSON no nulo cuyo `serde_json::to_string` ocupa ≤ 2048 bytes y con **exactamente** las claves esperadas por indicador (error en `readings[i].value` o `readings[i].value.<clave>`, sin rangos médicos; para números solo se exige que sean finitos):
    - `heart_rate`: `{ bpm }`
    - `oxygen_in_blood`: `{ percentage }`
    - `blood_pressure`: `{ systolic, diastolic }`
    - `glucose`: `{ mg_dl }`
    - `body_temperature`: `{ celsius }`
    - `sleep`: `{ hours }`
    - `nutrition`: `{ kcal }`
    - `space_radiation`: `{ usv_h }`
    - `distance_from_earth`: `{ km }`
    - `mood`: `{ label }` (cadena no vacía/blanca de 1–64 caracteres sin controles)
    - `hrv`: `{ ms }`
    - `body_battery`: `{ value }`

### Regla de frecuencia y comparación de valores

Constantes públicas en `crates/store/src/telemetry.rs`:

- `MIN_CHANGE_INTERVAL_MS = 10_000` (10 s)
- `HEARTBEAT_MS = 60_000` (60 s)

Dentro de la misma transacción que inserta cada lectura se consulta la última fila guardada de su serie `(astronaut_key, indicator)`:

- Si no existe ninguna, se guarda con `prev_hash = "0000...0000"` (64 ceros) y se inicializa su fila en `telemetry_heads` (`count = 1`).
- Si existe una lectura previa, dos valores se consideran iguales si coinciden tanto `unit` como el JSON de `value` bajo **igualdad numérica por `f64`** (por ejemplo, `72` y `72.0` son iguales) y estructural para cadenas y objetos.
- Se **guarda** (`stored += 1`, encadenando `prev_hash = last_row_hash` y avanzando `telemetry_heads`) únicamente si se cumple alguna de estas dos condiciones:
  1. El valor (o la unidad) cambió **y** `observed_at - prev_observed_at >= MIN_CHANGE_INTERVAL_MS` (`10_000` ms).
  2. Han pasado `observed_at - prev_observed_at >= HEARTBEAT_MS` (`60_000` ms), aunque el valor no haya cambiado (latido periódico).
- En cualquier otro caso, se omite (`skipped += 1`) sin tocar `telemetry_readings` ni `telemetry_heads`.

### Tamaño esperado por día y astronauta (cálculo analítico)

> Nota: las cifras siguientes son un **cálculo analítico** a partir de las constantes y del esquema, no una medición empírica en dispositivo. **Archivar y podar históricos queda fuera de esta entrega.**

Con 12 indicadores recibidos cada 2 s, el tope sin estrangulamiento sería `12 × (86_400 / 2) = 518_400` filas/día/astronauta. Con `MIN_CHANGE_INTERVAL_MS = 10_000` y `HEARTBEAT_MS = 60_000`:

- **Escenario estable (solo latido cada 60 s):** `86_400 / 60 = 1_440` filas/día por indicador → **`17_280` filas/día/astronauta** (reducción de 30×).
- **Peor caso continuo (los 12 indicadores cambiando cada ≥ 10 s):** `86_400 / 10 = 8_640` filas/día por indicador → **`103_680` filas/día/astronauta** como cota máxima dura (reducción de 5×).
- **Coste aproximado por fila (tabla + 4 índices):** una fila típica ocupa ~240–290 B útiles (~320 B con cabecera de registro y holgura B-tree en `telemetry_readings`), más ~320 B repartidos entre los 4 índices secundarios, es decir **~640 B/fila** (~0,63 KiB/fila). `telemetry_heads` aporta a lo sumo 12 filas por astronauta (despreciable).
- **Crecimiento diario estimado por astronauta:**
  - Estable (`17_280` filas/día): `17_280 × 640 B` ≈ **~11 MB/día** (~330 MB/mes).
  - Peor caso (`103_680` filas/día): `103_680 × 640 B` ≈ **~66 MB/día** (~2,0 GB/mes).

## Cadena de hashes SHA-256 y alcance de la verificación

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

`verify_telemetry` recorre las filas en `ORDER BY reading_id ASC` (opcionalmente filtradas por `astronaut_key`), verifica `prev_hash` y `row_hash` y contrasta cada serie con su entrada en `telemetry_heads` (`last_reading_id`, `last_hash` y `count`). Las cabezas y las lecturas se consultan en una misma transacción de lectura. `checked_through_reading_id` es el mayor `reading_id` de las lecturas comprobadas en ese snapshot, o `null` si no hay lecturas; con `astronaut_key` solo considera ese astronauta. El valor identifica el alcance de esa comprobación y puede quedar desactualizado al insertar nuevas lecturas. En caso de fallo devuelve `ok: false` y `problem` con uno de estos valores:

- `broken_link`: ruptura de `prev_hash` respecto a la fila anterior (o al génesis). `first_bad_reading_id` indica la fila afectada.
- `hash_mismatch`: contenido alterado, fuente inválida o discrepancia entre `row_hash` y el hash recalculado o `telemetry_heads.last_hash`. `first_bad_reading_id` indica la fila afectada.
- `missing_tail`: faltan filas al final de la serie (`actual.last_reading_id < head.last_reading_id`). `first_bad_reading_id` indica `head.last_reading_id`.
- `missing_series`: existe una cabeza en `telemetry_heads` sin filas en `telemetry_readings`. `first_bad_reading_id` es `None`.
- `count_mismatch`: el número de filas de la serie no coincide con `telemetry_heads.count` (o hay filas sin cabeza o más allá de `head.last_reading_id`).

**Alcance y modelo de amenazas:** esta verificación detecta ediciones accidentales, truncamientos de cola, borrados parciales y desaparición de series completas dentro de la base local. **No** protege frente a alguien que tenga acceso de escritura a todo el archivo SQLite, ya que podría desactivar o eliminar los triggers y reescribir de forma coherente tanto `telemetry_readings` como `telemetry_heads`; para proteger frente a ese adversario habría que exportar o anclar `last_hash` fuera del dispositivo.

## Comandos Tauri y métodos de `Store`

| Comando Tauri | Entrada (`{ input }`) | Salida |
|---|---|---|
| `record_telemetry` | `{ readings: TelemetryReadingInput[] }` | `{ stored, skipped, batch_id }` |
| `list_telemetry` | `{ astronaut_key, indicator?, from?, to?, limit? }` | `TelemetryItem[]` (orden `reading_id DESC`; `limit` por defecto `200`, máximo `1000`; `indicator` desconocido falla con `invalid_value` en `field: "indicator"`) |
| `verify_telemetry` | `{ astronaut_key? }` (opcional; admite omisión de `input` o `{}`) | `{ ok, checked, series_checked, checked_through_reading_id, first_bad_reading_id, problem }` |
