# Decisión 003 — Núcleo en Rust: registro, evaluación y almacenamiento local

Fecha: 14-nov-2026 · Estado: **aceptada para la entrega del 15-nov**
Contrato de referencia: [`docs/api/modelo-datos-backend.md`](../api/modelo-datos-backend.md) v0.3 (§1, §3, §5, §6). Si algo de aquí choca con el contrato, manda el contrato.
Umbrales médicos y sus fuentes: [`docs/api/umbrales-core.md`](../api/umbrales-core.md).

## 1. Reparto entre crates

| Crate | Hace | No hace |
|---|---|---|
| `pulsys-core` | Tipos del catálogo (§5), validación dura, avisos de plausibilidad, evaluación (estados + escalado), registro efectivo (§6), DTO del contrato con `serde` | E/S, reloj, azar. El instante "ahora" entra como argumento |
| `pulsys-store` | SQLite: perfil, registros (solo inserción), eventos, evaluaciones, outbox, cursor. Genera UUIDv7 | Decidir salud. Llama a `core` |
| `pulsys-seed` | Perfil y escenarios **DEMO** deterministas | Datos de la persona real (nunca) |
| `src-tauri` | Comandos Tauri. Lee el reloj del sistema y se lo pasa a `store`/`core` | Lógica de salud |

## 2. Tipos de `core` (resumen)

- `Timestamp(i64)`: milisegundos UTC desde la época Unix. `Timestamp::parse_rfc3339(&str)` y `to_rfc3339()` → `"2026-11-14T21:05:33.120Z"` (siempre `Z` y 3 decimales). Crate `time` solo para parsear/formatear (sin `now()`).
- Unidades con `newtype` validados al construirlos (`TryFrom<f64>` / `TryFrom<u8>`): `Bpm`, `MmHg`, `Celsius`, `MgDl` (> 0), `Hours` (0–24), `Score` (0–10), `Percent` (SpO₂: > 0 y ≤ 100), `MicroSvPerHour`, `Km`, `Kcal` (≥ 0).
- `Indicator` (enum, `snake_case`): `heart_rate`, `blood_pressure`, `body_temperature`, `glucose`, `sleep`, `mood`, `oxygen_in_blood`, `space_radiation`, `distance_from_earth`, `nutrition`, `symptom` (catálogo del 14-nov: se quitó `headache`). `Indicator::is_evaluated()` es `false` para los tres de contexto (radiación, distancia, nutrición): se validan y guardan, pero no se evalúan y no cuentan para `overall` (`reason: not_applicable`). La radiación sigue RF-12: va como dosis acumulada, no como estado de salud.
- `BodyZone` (enum §5, 14 valores). `SleepQuality`: `poor`, `fair`, `good`.
- `Measurement` (enum por indicador con sus tipos). Se construye **solo** desde `RawValue` (el JSON `value` del contrato) con `Measurement::validate(indicator, &serde_json::Value, unit: Option<&str>) -> Result<Measurement, ValidationError>`. No hay `Deserialize` derivado que se salte la validación (RNF-02): al leer de SQLite o de JSON se vuelve a validar.
- `Measurement::to_value()` → JSON del contrato y `unit()` → unidad del catálogo.
- `ValidationError { code: "invalid_value" | "invalid_schema", field: String, message: String }` (mensajes en inglés).

### Escalas de puntuación (0–10, enteros)
- `mood`: 0 = muy mal, 10 = muy bien (más alto es mejor).
- `symptom`: 0 = nada, 10 = el peor imaginable (más alto es peor).

## 3. Validación dura (§5) — se rechaza

Exactamente lo del §5 y nada más: tipo incorrecto, no finito, unidad distinta, `bpm`/`systolic`/`diastolic`/`mg_dl`/`celsius` ≤ 0, puntuación no entera o fuera de 0–10, sueño con ventana inválida (`window_end` ≤ `window_start`, > 24 h, `hours` fuera de 0..duración), `quality` fuera del enum, `body_zone` fuera del enum. Además: `note` y `description` ≤ 2000 caracteres, campos desconocidos en `value` → `invalid_schema`. En sueño, `window_start`/`window_end` son **obligatorios** (revisión de Sol: sin ellos no se puede comprobar `hours`).

**Valores extremos reales no se rechazan** (pulso 300, glucosa 2375, temperatura 14 °C).

## 4. Avisos de plausibilidad — se pide confirmar

`core::plausibility::check(&Measurement, measured_at, now) -> Vec<PlausibilityWarning>`; cada aviso tiene `code`, `message` (inglés) y `rule_id`. Si hay avisos y el usuario no confirmó (`confirm_warnings: false`), **no se guarda** y se devuelven los avisos. Umbrales en [`umbrales-core.md`](../api/umbrales-core.md) §1. Además, siempre:
- `future_timestamp`: `measured_at > now + 10 min`.
- `old_measurement`: `measured_at < now − 7 días`.
- `diastolic_not_below_systolic`: `diastolic ≥ systolic`.

## 5. Registro efectivo (§6, versión local)

`core::effective::EffectiveSet::build(records: &[RecordView])` con:
1. Excluir los registros con `measured_at > reference + 10 min`, donde `reference = received_at` si existe y si no `logged_at` (en local aún no hay `received_at`). Quedan marcados `future_timestamp`.
2. Agrupar por **registro raíz** (siguiendo `replaces`). En cada grupo es efectivo el de mayor clave de orden: `(seq, local_order)` con `seq = None` considerado **mayor** que cualquier `seq` (lo local sin subir es posterior a lo ya sincronizado, porque solo se puede corregir lo que ya existe).
3. Estado actual por indicador (en `symptom`, por `(indicator, body_zone)`): el efectivo de mayor `(measured_at, orden)`.

Ciclos o raíces que no existen (dato corrupto): el registro forma su propio grupo; nunca panic.

## 6. Evaluación

`core::evaluate::evaluate_series(key: SeriesKey, series: &[&RecordView], now: Timestamp) -> Evaluation` (pura; reglas en `rules.rs`). `series` = registros efectivos de ese indicador/zona, ordenados por `measured_at`.

**Estados** (contrato §3): `not_evaluated` (con `reason`: `no_data`, `too_few_data`, `stale`; y solo en local `not_applicable` para contexto), `stable`, `change` (con `direction`: `up`/`down`, `persistent`), `alarm`.

Algoritmo (en este orden):
0. Indicador de contexto (`is_evaluated() == false`) → siempre `not_evaluated / not_applicable`, con `latest_value` si hay datos, `source_kind: approved_protocol` y `history_needed: 0`. No cuenta para `overall` y no se guarda ni se sube como evaluación (el contrato §3 no admite ese motivo); sus registros sí se suben.
1. Sin registros efectivos → `not_evaluated / no_data`.
2. El último tiene más antigüedad que `stale_after` del indicador → `not_evaluated / stale`, **salvo** que disparara una regla de alarma: entonces sigue en `alarm` hasta que haya otra lectura (Hydrogen).
3. Se aplican **todas** las reglas del indicador sobre el último valor; cada una puede devolver `alarm`, `change(dir)` o nada:
   - **Reglas de rango absoluto** (`<ind>.alarm_low`, `<ind>.alarm_high`, `<ind>.attention_low`, `<ind>.attention_high`): umbrales con fuente en `umbrales-core.md` §2.
   - **Regla de historial personal** (`<ind>.personal_change`, `demo_heuristic`): `prior` = efectivos anteriores al último dentro de 14 días, del más nuevo al más viejo.
     - Si `prior.len() ≥ 5`: `baseline = median(prior[2..])` y se miran el último, `prior[0]` y `prior[1]`. Cambio si `|último − baseline| ≥ delta` del indicador. `persistent = true` si los tres se desvían en la misma dirección.
     - Si `3 ≤ prior.len() < 5`: `baseline = median(prior)`, `persistent = false`.
     - Si `prior.len() < 3`: la regla no se puede aplicar.
     - Solo cuentan las direcciones relevantes (`mood`, sueño y `oxygen_in_blood` solo `down`; `symptom` solo `up`).
   - Para las reglas de rango absoluto, `persistent = true` si el último y los dos efectivos anteriores (dentro de 14 días) disparan la misma regla.
4. Resultado = la regla más grave (`alarm` > `change` > nada). Empates: la de `rule_id` menor en orden alfabético (determinista).
5. Si ninguna regla se dispara:
   - si la regla de historial **se pudo aplicar** → `stable` (`rule_id = <ind>.personal_change`);
   - si no → `not_evaluated / too_few_data`, con `history_count` y `history_needed = 3`. **La ausencia de historial nunca da `stable`.**

**Escalado** (RF-32): `stable` → `continue`; `change` → `self_care`; `change` persistente → `flag_for_ground`; `alarm` → `urgent`; `not_evaluated` → `continue`. Una regla puede subir el escalado (`escalation_floor`), nunca bajarlo.

**Eventos:** `need_help`, `syncope`, `visual_field_loss` y `suspected_clot` dan `alarm` / `urgent` (RF-10, `approved_protocol` del equipo) **sin caducidad** (revisión de Sol, B2), hasta que el contrato tenga una resolución explícita. `feeling_report` no evalúa nada. Su evaluación usa `indicator = <kind>` (p. ej. `"need_help"`) e `input_event_ids`. **Pendiente de confirmar con backend** que acepte esos valores en `indicator`.

Cada `Evaluation` lleva: `evaluation_id` (lo pone `store`), `indicator`, `body_zone` (solo síntomas), `status`, `reason`, `direction`, `persistent`, `escalation`, `rule_id`, `rule_version`, `source_kind`, `source_ref`, `input_record_ids`, `input_event_ids`, `core_version`, `evaluated_at`, y para la interfaz `baseline`, `history_count`, `history_needed`, `latest_value`.

`core::evaluate::overall(&[Evaluation]) -> Overall`: `alarm` > `change`; si no, `not_evaluated` si alguna serie está sin evaluar; `stable` solo si todas lo están (Sol, B3). Escalado = el mayor. `body_zone_status` = peor estado de los síntomas de cada zona (RF-07).

## 7. Almacenamiento (`store`)

Migraciones con `PRAGMA user_version`. Tablas:

| Tabla | Columnas clave |
|---|---|
| `profile` (1 fila) | `astronaut_json`, `medical_profile_json`, `medical_conditions_json`, `profile_version`, `is_demo`, `updated_at` |
| `records` | `record_id` PK, `local_order` (INTEGER autoincremental), `indicator`, `body_zone` (solo síntomas), `value_json`, `unit`, `measured_at` (ms), `logged_at` (ms), `source`, `interpreted_by_json`, `replaces`, `root_id`, `note`, `seq`, `received_at`, `is_demo` |
| `events` | `event_id` PK, `kind`, `certainty`, `message`, `body_zone`, `priority`, `occurred_at`, `logged_at`, `seq`, `received_at`, `is_demo` |
| `evaluations` | `evaluation_id` PK, `series_key`, `payload_json`, `evaluated_at`, `is_demo` |
| `outbox` | `outbox_id` INTEGER PK (FIFO), `entity`, `entity_id`, `payload_json`, `state` (`pending`/`synced`/`failed`), `attempts`, `last_error`, `created_at` |
| `sync_state` | `key` PK, `value` (`pull_cursor`, `last_push_at`, `last_pull_at`) |
| (v3) | Lo pendiente en la outbox con `indicator: headache` pasa a `failed` con `retired_indicator`: se conserva, no se envía. En `pull`, un `headache` histórico bien formado queda en `pulled_changes` sin convertirse en registro vigente (Sol, B1/I1) |
| `pulled_changes` (v2) | `seq` PK, `entity`, `entity_id`, `payload_json`: todo lo recibido en `pull`, íntegro (Sol, B4) |

Reglas:
- Insertar entidad + fila de outbox en **una transacción**. El `payload_json` de la outbox es el DTO exacto del contrato (§3).
- **Lo DEMO nunca entra en la outbox**, y la primera escritura real (registro, evento, perfil o `pull`) borra todo lo DEMO en la misma transacción (Sol, B1).
- Una evaluación solo se guarda (y se encola) si cambió respecto a la última guardada de esa serie (`status`, `reason`, `direction`, `persistent`, `escalation`, `rule_id`, `rule_version`). Así `get_status` no llena la base de datos.
- `correct_record`: el registro corregido debe existir, ser del mismo indicador y, en síntomas, de la misma `body_zone`. `root_id` se hereda del corregido.
- Funciones para `sync` (aún sin HTTP): `pending_outbox(limit)`, `mark_synced(outbox_id, seq, received_at)`, `mark_failed(outbox_id, error, retryable)`, `get_cursor()`, `apply_pull_page(changes, cursor, now)` (una transacción; si un ID ya existe comprueba el contenido y solo añade `seq`/`received_at`; si hay error no avanza el cursor; Sol, B4–B5).
- `store` recibe `now: Timestamp` como argumento; nunca lee el reloj. Los UUIDv7 se generan con `uuid::Uuid::new_v7(ts)` a partir de ese `now`.

## 8. Comandos de Tauri

Referencia para la interfaz: [`docs/api/comandos-core.md`](../api/comandos-core.md); tipos TS en `ui/src/core-api.ts`.
