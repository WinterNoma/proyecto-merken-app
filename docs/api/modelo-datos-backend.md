# Pulsys — Modelo de datos y API para el equipo de backend

Versión 0.3.1 · 14-nov-2026 · Para: equipo de backend (MySQL + DynamoDB + servidor)
Revisado por Sol: [v0.1](../../reviews/modelo-datos-backend.md) y [v0.2](../../reviews/modelo-datos-backend-v0.2.md); la v0.3 aplica sus correcciones exactas. Los nombres de campos, tablas y endpoints van en **inglés**, igual que los textos que ve el usuario.

## 0. Resumen

| Dónde | Qué se guarda |
|---|---|
| **MySQL** | Identidad: astronautas, usuarios de Tierra, sesiones, misión, perfil médico verificado y antecedentes |
| **DynamoDB** | Historial clínico generado por la app, por Tierra y por Garmin, más un **diario de cambios** para sincronizar |
| **Móvil (SQLite)** | Copia local del perfil y del historial de su astronauta; funciona sin conexión |

**Cambios en v0.3.1:** tablas de DynamoDB renombradas: `health_records`, `health_events`, `health_evaluations`, `health_recommendations` (antes `pulsys_suggestions`), `recommendation_reviews` (antes `pulsys_suggestion_reviews`), `health_analyses`, `sync_changes` y `sync_heads`. Solo cambian los nombres de las tablas: los valores de `entity` en el diario (`record`, `suggestion`, `suggestion_review`…) y los campos (`suggestion_ids`…) siguen igual.

**Cambios respecto a v0.1:**
- La escritura condicional usa `attribute_not_exists(#pk)`, porque la v0.1 permitía sobrescribir.
- Un único diario `sync_changes` con número de secuencia sustituye a los cinco cursores por fecha.
- Se elimina `health_status` de MySQL: el estado actual se **calcula** a partir de los registros.
- La revisión de una recomendación pasa a ser una entidad propia (`suggestion_review`).
- Se añaden roles y permisos, tokens con rotación, límites de tamaño, paginación, misión y MET.
- Los rangos del catálogo dejan de rechazar valores extremos reales.
- Se quitan `private` y `read_at`.

## 1. Reglas comunes

1. **El historial clínico es inmutable**: registros, eventos, evaluaciones, recomendaciones, revisiones y análisis. Nada se edita ni se borra. Una corrección es un registro nuevo con `replaces`.
2. **El perfil y las proyecciones sí se actualizan**: perfil médico, antecedentes, sesiones y estado actual calculado.
3. **IDs:** UUID en texto.
   - Lo que nace en el móvil: **UUIDv7** generado en el móvil.
   - Garmin y los POST de Tierra: **UUIDv5 determinista** generado por el backend (§4), para que un reintento no duplique nada.
4. **Fechas:** ISO 8601 en UTC con milisegundos (`"2026-11-14T21:05:33.120Z"`).
   - `measured_at` y `occurred_at` vienen del reloj del móvil o del dispositivo de medida.
   - `received_at` y `seq` los asigna **el servidor**. Ningún reloj del cliente controla la sincronización.
5. **Ausencia de un valor:** campo `null` o ausente. No se usa `0` ni `""` para decir "no hay dato". El cero sí es un valor válido (p. ej., `0` horas de sueño o ánimo `0`).
6. **La API habla JSON normal.** Las letras `S`, `N`, `M`, `L` y `BOOL` de este documento solo describen cómo se guarda en DynamoDB.
7. **Ninguna escritura se salta el recorrido de §4**, venga del móvil, de Tierra, de Garmin o de un script de carga.

## 2. MySQL — identidad y perfil

### `missions`
| Columna | Tipo | Nulo | Notas |
|---|---|---|---|
| `mission_id` | `CHAR(36)` PK | no | |
| `name` | `VARCHAR(100)` | no | |
| `epoch_utc` | `DATETIME(3)` | no | Instante cero del MET. `MET = instante_utc − epoch_utc` |
| `created_at` | `DATETIME(3)` | no | |

### `astronauts`
| Columna | Tipo | Nulo | Notas |
|---|---|---|---|
| `astronaut_id` | `CHAR(36)` PK | no | |
| `username` | `VARCHAR(64)` UNIQUE | no | |
| `password_hash` | `VARCHAR(255)` | no | argon2id |
| `display_name` | `VARCHAR(100)` | no | |
| `mission_id` | `CHAR(36)` FK | sí | |
| `status` | `ENUM('active','inactive')` | no | |
| `created_at`, `updated_at` | `DATETIME(3)` | no | |

### `medical_profiles` (1 fila por astronauta, la crea el equipo con datos verificados)
| Columna | Tipo | Nulo | Notas |
|---|---|---|---|
| `astronaut_id` | `CHAR(36)` PK, FK | no | |
| `height_cm` | `DECIMAL(5,1)` | sí | |
| `weight_kg` | `DECIMAL(5,1)` | sí | |
| `blood_type` | `ENUM('A+','A-','B+','B-','AB+','AB-','O+','O-')` | sí | |
| `sex` | `ENUM('female','male','other')` | sí | |
| `date_of_birth` | `DATE` | sí | |
| `notes` | `TEXT` | sí | |
| `verified_by` | `VARCHAR(100)` | sí | |
| `verified_at` | `DATETIME(3)` | sí | |
| `profile_version` | `INT` | no | Sube +1 **en la misma transacción** cada vez que cambia el perfil o un antecedente |
| `updated_at` | `DATETIME(3)` | no | |

### `medical_conditions` (antecedentes, 0..N por astronauta)
| Columna | Tipo | Nulo | Notas |
|---|---|---|---|
| `condition_id` | `CHAR(36)` PK | no | |
| `astronaut_id` | `CHAR(36)` FK | no | |
| `name` | `VARCHAR(200)` | no | En inglés, p. ej. `"Asthma"` |
| `icd10_code` | `VARCHAR(10)` | sí | |
| `status` | `ENUM('active','resolved')` | no | |
| `since` | `DATE` | sí | |
| `notes` | `TEXT` | sí | |

### `ground_users` (personal de Tierra y servicios)
| Columna | Tipo | Nulo | Notas |
|---|---|---|---|
| `ground_user_id` | `CHAR(36)` PK | no | |
| `username` | `VARCHAR(64)` UNIQUE | no | |
| `password_hash` | `VARCHAR(255)` | no | argon2id (los servicios pueden usar en su lugar una clave de API guardada como hash) |
| `display_name` | `VARCHAR(100)` | no | |
| `role` | `ENUM('ground_reviewer','ground_service')` | no | Ver §8 |
| `status` | `ENUM('active','inactive')` | no | |

### `sessions` y `refresh_tokens`
| `sessions` | Tipo | Nulo | Notas |
|---|---|---|---|
| `session_id` | `CHAR(36)` PK | no | |
| `principal_type` | `ENUM('astronaut','ground_user')` | no | |
| `principal_id` | `CHAR(36)` | no | |
| `device_id` | `CHAR(36)` | sí | Lo genera la app; **no autentica**, solo identifica |
| `device_name` | `VARCHAR(100)` | sí | |
| `access_token_hash` | `CHAR(64)` | no | SHA-256 del token |
| `access_expires_at` | `DATETIME(3)` | no | 15 minutos |
| `created_at`, `last_seen_at` | `DATETIME(3)` | no | |
| `revoked_at` | `DATETIME(3)` | sí | |

| `refresh_tokens` | Tipo | Nulo | Notas |
|---|---|---|---|
| `token_hash` | `CHAR(64)` PK | no | SHA-256 |
| `session_id` | `CHAR(36)` FK | no | |
| `expires_at` | `DATETIME(3)` | no | 30 días |
| `used_at` | `DATETIME(3)` | sí | Si llega uno ya usado → **revocar la sesión** (detección de reutilización) |

### `external_accounts` (vínculo con Garmin)
| Columna | Tipo | Nulo | Notas |
|---|---|---|---|
| `provider` | `ENUM('garmin')` | no | PK compuesta |
| `external_user_id` | `VARCHAR(128)` | no | PK compuesta |
| `astronaut_id` | `CHAR(36)` FK | no | |

## 3. DynamoDB — historial y diario

### Atributos comunes de las tablas de entidades
| Atributo | Tipo | Quién lo pone | Notas |
|---|---|---|---|
| `astronaut_id` | S | — | **Clave de partición (PK)** en todas las tablas |
| `<entidad>_id` | S | origen | **Clave de ordenación (SK)**: `record_id`, `event_id`… |
| `seq` | N | servidor | Posición en el diario (§4) |
| `received_at` | S | servidor | |
| `created_by` | M | servidor | `{ "type": "astronaut" o "ground_reviewer" o "ground_service", "id": "…" }`, tomado del token |
| `schema_version` | N | origen | `1` |
| `app_version` | S | móvil | Nulo si no viene del móvil |

### `health_records` — mediciones y síntomas
| Atributo | Tipo | Nulo | Notas |
|---|---|---|---|
| `record_id` (SK) | S | no | |
| `indicator` | S | no | Catálogo §5 |
| `value` | M | no | Formato según el indicador |
| `unit` | S | no | |
| `measured_at` | S | no | |
| `logged_at` | S | no | Cuándo se anotó |
| `source` | S | no | `form`, `text`, `voice` (móvil) · `earth` (Tierra) · `garmin` |
| `interpreted_by` | M | sí | `{ "kind": "rules", "version": "0.1.0" }` o `{ "kind": "granite", "model_version": "…" }` |
| `replaces` | S | sí | `record_id` corregido: mismo astronauta y mismo indicador |
| `external_id` | S | sí | `"garmin:<id>"` |
| `note` | S | sí | Máximo 2000 caracteres |

### `health_events` — "Need help", alarmas y "How I feel"
| Atributo | Tipo | Nulo | Notas |
|---|---|---|---|
| `event_id` (SK) | S | no | |
| `kind` | S | no | `need_help`, `syncope`, `visual_field_loss`, `suspected_clot`, `feeling_report` |
| `certainty` | S | no | `symptom` o `suspicion`. El móvil **no puede** enviar `confirmed` |
| `message` | S | sí | Texto libre ("How I feel…"), máximo 2000 caracteres |
| `body_zone` | S | sí | Enum de zonas, §5 |
| `priority` | S | no | `urgent` o `normal` |
| `occurred_at`, `logged_at` | S | no | |

### `health_evaluations` — lo que decidió el núcleo de la app
| Atributo | Tipo | Nulo | Notas |
|---|---|---|---|
| `evaluation_id` (SK) | S | no | |
| `indicator` | S | no | |
| `status` | S | no | `not_evaluated`, `stable`, `change`, `alarm` |
| `reason` | S | sí | Si `not_evaluated`: `no_data`, `too_few_data`, `stale` |
| `direction` | S | sí | `up` o `down` |
| `persistent` | BOOL | no | |
| `escalation` | S | no | `continue`, `self_care`, `flag_for_ground`, `urgent` |
| `rule_id`, `rule_version` | S | no | |
| `source_kind` | S | no | `published`, `approved_protocol`, `demo_heuristic` |
| `source_ref` | S | sí | Cita de la fuente |
| `input_record_ids`, `input_event_ids` | L | no | Pueden ir vacías |
| `core_version`, `evaluated_at` | S | no | |

Como no hay referencia previa al vuelo, `change` significa "cambio respecto al historial reciente de la propia persona en la app". El último dato conocido **no** es una referencia ni prueba de normalidad, y la ausencia de datos nunca se convierte en `stable`.

### `health_recommendations` — recomendaciones del v5 en el móvil
| Atributo | Tipo | Nulo | Notas |
|---|---|---|---|
| `suggestion_id` (SK) | S | no | |
| `trigger_text` | S | sí | Lo que dijo el astronauta, p. ej. `"My foot hurts"` |
| `record_ids`, `event_ids` | L | no | Datos en los que se basó (pueden ir vacías) |
| `evaluation_id` | S | sí | |
| `cards` | L | no | `[{ "card_id": "…", "version": "…" }]` |
| `text` | S | no | La recomendación mostrada, en inglés, máximo 4000 caracteres |
| `model` | M | no | `{ "name": "space-health-v5", "version": "…", "lora_sha256": "…" }` |
| `generated_at` | S | no | |

### `recommendation_reviews` — Tierra aprueba, corrige o rechaza
| Atributo | Tipo | Nulo | Notas |
|---|---|---|---|
| `review_id` (SK) | S | no | Lo genera el servidor |
| `suggestion_id` | S | no | Del mismo astronauta |
| `status` | S | no | `approved`, `corrected`, `rejected` |
| `corrected_text` | S | sí | **Obligatorio** si `status = corrected` |
| `note` | S | sí | |
| `reviewed_at` | S | no | Lo pone el servidor |
| `replaces` | S | sí | `review_id` anterior. La revisión vigente es la de mayor `seq` |

### `health_analyses` — análisis y mensajes de Tierra
| Atributo | Tipo | Nulo | Notas |
|---|---|---|---|
| `analysis_id` (SK) | S | no | Lo genera el servidor |
| `author_type` | S | no | `model` (rol `ground_service`) o `doctor` (rol `ground_reviewer`); lo deduce el servidor |
| `model_version` | S | sí | |
| `about` | M | no | `{ "record_ids": [], "event_ids": [], "evaluation_ids": [], "suggestion_ids": [] }` |
| `summary` | S | no | En inglés, máximo 4000 caracteres |
| `recommendations` | L | no | Máximo 10 textos de 500 caracteres |
| `attention_level` | S | no | `routine`, `soon`, `urgent` |
| `message` | S | sí | Mensaje directo del médico |
| `created_at` | S | no | Lo pone el servidor |

**Regla:** un análisis se muestra **junto** al estado local y **nunca rebaja una alarma** de la app.

### `sync_changes` — diario de cambios (lo que el móvil descarga)
| Atributo | Tipo | Notas |
|---|---|---|
| `astronaut_id` | S | PK |
| `seq` | N | SK: 1, 2, 3… sin huecos por astronauta |
| `entity` | S | `record`, `event`, `evaluation`, `suggestion`, `suggestion_review`, `analysis` |
| `entity_id` | S | |
| `payload` | M | Copia completa de la entidad, incluidos `seq`, `received_at` y `created_by` |
| `received_at` | S | |

### `sync_heads`
| Atributo | Tipo | Notas |
|---|---|---|
| `astronaut_id` | S | PK |
| `last_seq` | N | Último `seq` publicado |

### Índice opcional para el panel
Si el panel necesita gráficas por indicador, añadir en `health_records` el GSI `by_indicator`:
- PK `indicator_pk` = `astronaut_id + "#" + indicator` (atributo que se escribe al guardar)
- SK `measurement_key` = `measured_at + "#" + record_id`
- Proyección: `ALL`

Los GSI son de consistencia eventual: sirven al panel, **no** a la sincronización.

## 4. Cómo se publica una entidad (idéntico para móvil, Tierra y Garmin)

1. Validar el payload (§5) y los permisos (§8). Las referencias (`replaces`, `suggestion_id`, `record_ids`…) deben existir y ser **del mismo astronauta**. Si no existen todavía, se rechaza como reintentable (`unknown_reference`). El móvil sube en orden (cola FIFO), así que lo referenciado llega antes.
2. Leer `sync_heads` con `ConsistentRead: true` → `n` (`0` si no existe).
3. `TransactWriteItems` con tres operaciones:
   - `Put` de la entidad con `ConditionExpression: attribute_not_exists(#pk)` y `ExpressionAttributeNames: {"#pk": "astronaut_id"}`. Pedir `ReturnValuesOnConditionCheckFailure: ALL_OLD`.
   - `Put` en `sync_changes` con `seq = n + 1`.
   - `Update` de `sync_heads`: calcular `next = n + 1` en el backend y usar `UpdateExpression: SET #last = :next` con `#last = last_seq`. Condición: `#last = :expected` si el contador existe, o `attribute_not_exists(#pk)` si no existe (`#pk = astronaut_id`). Los números van en `ExpressionAttributeValues`. **No** añadir un `ConditionCheck` aparte sobre el mismo contador: una transacción no admite dos acciones sobre el mismo elemento.
4. Si la transacción se cancela, distinguir la causa: condición fallida, conflicto concurrente, falta de capacidad o error de validación.
   - **La entidad ya existía:** comparar **solo el DTO de entrada permitido**, normalizado (claves ordenadas), excluyendo todos los campos que genera o deriva el servidor (`seq`, `received_at`, `created_by`, `reviewed_at`, `created_at`, `indicator_pk`…). Si es igual → `duplicate`; si es distinto → `id_conflict`. Se conservan los campos de servidor del primer guardado.
   - **Cambió el contador** (otro escritor publicó a la vez): repetir desde el paso 2, hasta 5 veces. Después → `internal_error` reintentable.
   - **Fallo transitorio** (capacidad, red): reintentar con espera creciente. Si el resultado es incierto, leer la entidad con `ConsistentRead` antes de decidir.

**IDs deterministas en el backend:**
- **Garmin:** `record_id` = UUIDv5 de `"garmin:<id de Garmin>"`. Recibir dos veces la misma lectura da `duplicate`.
- **POST de Tierra:** la petición lleva la cabecera `Idempotency-Key`, estable por operación. El servidor deriva el ID con UUIDv5 sobre un namespace fijo + principal + astronauta + operación + clave. Mismo ID y mismo DTO → `duplicate`; contenido distinto → `id_conflict`.

La idempotencia la dan la clave y la condición, **no** el `ClientRequestToken` de DynamoDB, que solo dura 10 minutos.

## 5. Catálogo de indicadores y validación

| `indicator` | `value` | `unit` |
|---|---|---|
| `heart_rate` | `{ "bpm": 72 }` | `bpm` |
| `blood_pressure` | `{ "systolic": 118, "diastolic": 76 }` | `mmHg` |
| `body_temperature` | `{ "celsius": 36.7 }` | `°C` |
| `glucose` | `{ "mg_dl": 95 }` | `mg/dL` |
| `sleep` | `{ "hours": 5.5, "quality": "poor", "window_start": "…Z", "window_end": "…Z" }` | `h` |
| `mood` | `{ "score": 6 }` | `score_0_10` |
| `oxygen_in_blood` | `{ "percentage": 98.2 }` | `%` |
| `space_radiation` | `{ "usv_h": 12.5 }` | `µSv/h` |
| `distance_from_earth` | `{ "km": 384400.0 }` | `km` |
| `nutrition` | `{ "kcal": 2400.0 }` | `kcal` |
| `symptom` | `{ "body_zone": "left_foot", "score": 4, "description": "My foot hurts" }` | `score_0_10` |

**El backend rechaza solo lo que no tiene sentido** (`invalid_value`):
- Tipo incorrecto, número no finito (`NaN`, `Infinity`) o unidad distinta de la del catálogo.
- Valores negativos o cero donde no pueden darse: `bpm`, `systolic`, `diastolic`, `mg_dl` y `celsius` deben ser mayores que 0.
- `percentage` (SpO₂) debe ser mayor que 0 y como máximo 100. `usv_h`, `km` y `kcal` deben ser 0 o más. `km` es la distancia desde la superficie de la Tierra (en órbita baja, la altitud). `space_radiation`, `distance_from_earth` y `nutrition` son de contexto: el núcleo no les da estado de salud.
- Puntuaciones que no son enteros de 0 a 10.
- Sueño: `window_end` debe ser posterior a `window_start`, la ventana no puede pasar de 24 h y `hours` debe estar entre 0 y la duración de la ventana. `quality` admite `poor`, `fair`, `good` o `null`.
- `body_zone` fuera del enum: `head`, `neck`, `chest`, `abdomen`, `back`, `left_arm`, `right_arm`, `left_hand`, `right_hand`, `left_leg`, `right_leg`, `left_foot`, `right_foot`, `other`.

**No rechaza valores extremos.** Pulso de 300, glucosa de 2375 mg/dL o temperatura de 14 °C están documentados en personas reales. El móvil pide confirmar los valores inusuales antes de guardarlos; esos umbrales viven en el núcleo de la app y se revisan aparte.
Para añadir un indicador no hace falta migrar tablas, pero sí actualizar el catálogo, los validadores y la app.

## 6. Estado actual (calculado, no guardado)

- Una corrección conserva astronauta, indicador y, en síntomas, `value.body_zone`. `replaces` apunta a un registro que ya existe.
- Para calcular el estado:
  1. Excluir los registros con `measured_at > received_at + 10 minutos`. Se quedan en el historial con el aviso `future_timestamp`.
  2. Agrupar los demás por su **registro raíz**, siguiendo la cadena de `replaces` hasta el original. En cada grupo solo es efectivo el de mayor `seq`.
  3. Entre los efectivos, el estado actual de cada indicador (en `symptom`, de cada `(indicator, body_zone)`) es el de mayor `(measured_at, seq)`.
- Las mediciones que el equipo verifica (p. ej., glucosa de laboratorio) se publican como registros con `source: "earth"`; no van a una tabla aparte.

## 7. API (REST + JSON, **HTTPS obligatorio**, `Authorization: Bearer <access_token>`)

### Autenticación
| Método | Ruta | Cuerpo → Respuesta |
|---|---|---|
| `POST` | `/v1/auth/login` | `{ principal_type, username, password, device_id, device_name }` → `{ access_token, refresh_token, access_expires_at, refresh_expires_at, principal_type, principal_id, role }` |
| `POST` | `/v1/auth/refresh` | `{ refresh_token }` → tokens nuevos; el refresh anterior queda usado |
| `POST` | `/v1/auth/logout` | Revoca la sesión |

- Tokens **opacos y aleatorios** (256 bits); en el servidor solo se guarda su hash.
- Login limitado por intentos. Ante un error, siempre el mensaje genérico `invalid_credentials`.
- `principal_type` es obligatorio: `astronaut` (la app móvil) o `ground_user` (panel y servicios). Así no hay ambigüedad si un mismo `username` existe en las dos tablas. El rol de un astronauta es siempre `astronaut`.

### Perfil
`GET /v1/me` → DTO explícito, **sin** `password_hash` ni datos de sesión:
```json
{
  "astronaut": { "astronaut_id": "…", "username": "…", "display_name": "…",
                 "mission": { "mission_id": "…", "name": "…", "epoch_utc": "…Z" } },
  "medical_profile": { "height_cm": 178.0, "weight_kg": 74.5, "blood_type": "O+", "profile_version": 3, "…": "…" },
  "medical_conditions": [ { "condition_id": "…", "name": "Asthma", "status": "resolved" } ],
  "current_status": [ { "indicator": "glucose", "record_id": "…", "value": { "mg_dl": 95 }, "measured_at": "…Z", "source": "earth" } ],
  "head_seq": 128
}
```
El móvil llama a `/v1/me` cada vez que recupera la conexión, con independencia del cursor, y actualiza el perfil si cambió `profile_version`.

### Subir (móvil → Tierra)
`POST /v1/sync/push` (el `device_id` sale de la sesión):
```json
{ "batch": [
  { "entity": "record",     "id": "0193…", "payload": { "…": "…" } },
  { "entity": "event",      "id": "0193…", "payload": { "…": "…" } },
  { "entity": "evaluation", "id": "0193…", "payload": { "…": "…" } },
  { "entity": "suggestion", "id": "0193…", "payload": { "…": "…" } }
] }
```
→
```json
{ "results": [
  { "entity": "record", "id": "0193…", "status": "accepted", "seq": 129, "received_at": "…Z", "warnings": [] },
  { "entity": "event",  "id": "0193…", "status": "rejected", "code": "invalid_value", "retryable": false }
] }
```
- `status`: `accepted` (guardado de forma duradera; **no** significa revisado), `duplicate`, `id_conflict`, `rejected`.
- `code`: `invalid_schema`, `invalid_value`, `unknown_reference`, `forbidden_field`, `payload_too_large`, `internal_error`.
- Se procesa en orden. Un error en un elemento no anula los demás.
- Qué hace el móvil:
  - `accepted` o `duplicate` → lo marca como sincronizado.
  - Error reintentable → reintenta con espera creciente.
  - Error no reintentable → lo conserva y muestra "Not synced". **Nunca lo borra.**
- Los `event` con `priority: urgent` deben aparecer en el panel de inmediato.

### Bajar (Tierra → móvil)
`GET /v1/sync/pull?cursor=<opaco>&limit=50` →
```json
{ "changes": [ { "seq": 130, "entity": "suggestion_review", "id": "…", "payload": { "…": "…" } } ],
  "next_cursor": "…", "has_more": true }
```
- El cursor es opaco y va **firmado** por el servidor: `{ v, astronaut_id, last_seq, through_seq }`. `through_seq` fija dónde termina el recorrido actual y no cambia durante ese recorrido.
- **Sin cursor:** `last_seq = 0` y `through_seq` = `last_seq` del contador leído con `ConsistentRead` (o `0` si no existe). Así **un móvil nuevo recupera todo el historial**, también lo que subió él mismo.
- **Cursor de un recorrido terminado** (`last_seq = through_seq`): se conserva `last_seq` y se lee un `through_seq` nuevo del contador. Así llegan los cambios posteriores.
- **Consulta:** tabla base `sync_changes`, `ConsistentRead=true`, `ScanIndexForward=true`, partición del astronauta autorizado y `seq BETWEEN :first AND :through`, con `first = last_seq + 1`. Si el intervalo está vacío, no se consulta.
- **Entrega:** un prefijo de hasta 50 cambios cuya respuesta completa no pase de 512 KiB. El cursor avanza **solo hasta el último cambio entregado**, nunca más allá de algo consultado pero no entregado. `has_more = last_seq < through_seq`.
- El móvil aplica cada página y guarda el cursor en **una sola transacción** de SQLite. Ignora los IDs que ya tiene.

### Para el panel de Tierra (lo construye el otro grupo)
| Método | Ruta | Rol | Para qué |
|---|---|---|---|
| `GET` | `/v1/astronauts` | reviewer | Lista |
| `GET` | `/v1/astronauts/{astronaut_id}` | reviewer | Igual que `/v1/me` |
| `GET` | `/v1/astronauts/{astronaut_id}/changes?cursor=&limit=` | reviewer, service | Igual que `pull` |
| `GET` | `/v1/astronauts/{astronaut_id}/records?indicator=&from=&to=&cursor=` | reviewer | Gráficas (usa `by_indicator`) |
| `POST` | `/v1/astronauts/{astronaut_id}/records` | reviewer (`earth`), service (`garmin`) | Mediciones verificadas o de Garmin |
| `POST` | `/v1/astronauts/{astronaut_id}/suggestions/{suggestion_id}/reviews` | reviewer | `{ status, corrected_text, note }` |
| `POST` | `/v1/astronauts/{astronaut_id}/analyses` | reviewer, service | Análisis o mensaje |
| `PUT` | `/v1/astronauts/{astronaut_id}/medical-profile` | reviewer | Sube `profile_version` |

Si el equipo edita el perfil directamente en MySQL, debe subir `profile_version` en la misma transacción.
Todos los `POST` de Tierra exigen la cabecera `Idempotency-Key` (§4).

## 8. Permisos

El token identifica `principal_type`, `principal_id`, rol y sesión.
- **Astronauta:** la partición (`astronaut_id`) sale **de la sesión**, nunca del cuerpo ni de la ruta.
- **Tierra:** la partición sale de la ruta, después de comprobar que su rol permite esa operación.
- Todas las referencias deben pertenecer al mismo astronauta. Una revisión que reemplaza a otra (`replaces`) debe ser de la misma recomendación.

| Rol | Puede |
|---|---|
| `astronaut` (móvil) | Leer su `/me` y su diario. Publicar solo `record` (con `source` `form`, `text` o `voice`), `event` (sin `confirmed`), `evaluation` y `suggestion`. **Nunca** `source: earth` o `garmin`, revisiones ni análisis (→ `forbidden_field`) |
| `ground_reviewer` (médico o equipo) | Leer los astronautas. Publicar registros `earth`, revisiones y análisis de tipo `doctor`. Editar el perfil |
| `ground_service` (modelo de Tierra, Garmin) | Publicar análisis de tipo `model` y registros `garmin` |

Los campos de autoría (`created_by`, `author_type`, `reviewed_at`) siempre los pone el servidor; si el cliente los envía, se ignoran.

## 9. Límites (propuestos por nosotros, no son límites de AWS)

| Qué | Límite |
|---|---|
| Push | 50 elementos y 1 MiB por petición (si se pasa → HTTP 413) |
| Entidad | 32 KiB de JSON en UTF-8 (→ `payload_too_large`; nunca se trunca) |
| Pull | Hasta 50 cambios y 512 KiB por página |
| Textos | `note`, `message`, `description` y `trigger_text`: 2000 caracteres; `text` y `summary`: 4000 |

Referencia de AWS: 400 KB por elemento y 1 MB por página de `Query` (continuar con `LastEvaluatedKey`).

## 10. Días sin conexión
- La app sigue funcionando y guardando en su cola aunque caduquen los tokens. Sin red **no se cierra la sesión ni se borra nada**.
- Al reconectar: primero `refresh`, después `push`, después `pull` y después `/me`.
- Si el refresh caducó o fue revocado, se pide login **solo para sincronizar**; los datos y las funciones locales siguen disponibles.
- Una revocación remota no puede surtir efecto hasta que el móvil se conecte.
- En el móvil, los tokens se guardan cifrados con una clave de Android Keystore (falta confirmar qué plugin de Tauri lo soporta).

## 11. Seguridad y privacidad
- Las credenciales de MySQL y AWS **solo en el servidor** (`.env`), nunca en la app ni en git.
- Con **datos de una persona real**: consentimiento, nada en git y nada en capturas públicas.
- Privacidad del "How I feel": por ahora **todo se envía a Tierra**. Si más adelante se quiere una nota privada, se diseñará aparte, porque una nota que no se sube tampoco se recupera en otro móvil.

## 12. Prueba mínima de aceptación (backend + app)
1. Cortar la red después de guardar pero antes de responder; reintentar → `duplicate`, sin registros repetidos.
2. Descargar varias páginas con `pull` y confirmar que no se salta ningún `seq`.
3. Iniciar sesión en otro móvil → recupera todo el historial.
4. Tierra corrige una recomendación → el móvil la recibe en el siguiente `pull`.
5. Un token de astronauta que intenta leer a otro astronauta o publicar un análisis → rechazado.

## 13. Preguntas abiertas para backend
1. ¿URL base del servidor y certificado HTTPS para la demo?
2. ¿Cómo llegan los datos de Garmin (Health API push, Connect IQ…) y con qué ID de usuario?
3. ¿El modelo de Tierra se ejecuta en el mismo servidor o como servicio aparte (rol `ground_service`)?
4. ¿Versión del SDK de AWS que vais a usar? (para comprobar transacciones y paginación)
