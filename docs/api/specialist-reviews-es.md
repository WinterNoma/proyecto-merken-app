# Pulsys: revisiones de especialista

**Tabla:** `specialist_reviews` · **Base de datos:** Amazon DynamoDB
**Contrato de referencia:** `docs/api/modelo-datos-backend.md` v0.3 (repo de la app). Los nombres de tablas, campos y valores van en inglés, igual que en el contrato.

Guarda notas y avisos que un especialista de Tierra escribe sobre un astronauta. Una revisión puede apuntar a una lectura de telemetría.

## Qué cambió respecto a la versión anterior

1. **Índice para listar por fecha (GSI `by_created`).** `review_id` es un UUIDv5 y no lleva tiempo, así que consultar por la clave principal devolvía las revisiones en un orden que parecía aleatorio. El listado ahora usa un índice ordenado por `created_at`, de la más nueva a la más antigua.
2. **Entidad en el diario y transacción completa.** Se fija `entity = "specialist_review"` en `sync_changes`. La escritura hace las tres operaciones del contrato §4: la revisión, el diario y `sync_heads`.
3. **Validación de todo `about`.** Antes solo se comprobaba `record_ids`. Ahora cualquier ID de `about` debe existir y ser del mismo astronauta; si no, se rechaza. Sin esto se podía enlazar datos de otro astronauta.
4. **`attention_level` cerrado en la API.** Se aceptan `routine` y `soon`; `urgent` se rechaza de forma explícita hasta que se acuerde. Antes dependía de lo que mandara el formulario.
5. **Idempotencia descrita.** Se indica qué responde la API si se repite la misma `Idempotency-Key`: `duplicate` o `id_conflict`.
6. **Límites de la API.** `limit` máximo de 50, y hasta 10 recomendaciones de 500 caracteres (contrato §3 y §9).
7. **Ejemplo corregido.** El `record_id` del ejemplo era un UUIDv4. Los registros del móvil son UUIDv7 y los de Tierra o Garmin, UUIDv5.
8. **Queda una decisión pendiente**, al final del documento: esta tabla se solapa con `health_analyses` del contrato.

## Claves

| Clave | Atributo | Tipo |
|---|---|---|
| Clave de partición | `astronaut_id` | String (S) |
| Clave de ordenación | `review_id` | String (S) |

Con las dos claves se lee una revisión concreta. Para listar se usa el índice `by_created`.

### Índice secundario global `by_created` (nuevo)

| Clave | Atributo | Tipo |
|---|---|---|
| Clave de partición | `astronaut_id` | String (S) |
| Clave de ordenación | `created_key` | String (S): `created_at + "#" + review_id` |

- Proyección: `ALL`.
- `created_key` lo escribe el backend al guardar. El `#review_id` evita empates si dos revisiones tienen el mismo `created_at`.
- Los GSI tienen consistencia eventual: sirven para el panel, no para la sincronización con el móvil, que usa `sync_changes`.

Al crear la tabla solo se declaran los atributos de clave (`astronaut_id`, `review_id`, `created_key`). El resto lo valida el backend al escribir.

## Atributos

| Atributo | Tipo | Obligatorio | Origen y validación |
|---|---|---|---|
| `astronaut_id` | S | Sí | Backend. UUID del astronauta, sacado de la ruta autorizada. |
| `review_id` | S | Sí | Backend. UUIDv5 determinista a partir de un namespace fijo + principal + astronauta + operación + `Idempotency-Key` (contrato §4). |
| `created_key` | S | Sí | Backend. `created_at#review_id`, para el índice `by_created`. |
| `summary` | S | Sí | Valoración o aviso del especialista. Sin espacios al inicio ni al final, no vacío, máximo 4000 caracteres. |
| `attention_level` | S | Sí | `routine` (nota) o `soon` (aviso). La API rechaza cualquier otro valor con `invalid_value`, incluido `urgent`, que está pendiente de acordar. |
| `recommendations` | L de S | Sí | Puede ir vacía. Máximo 10 textos de hasta 500 caracteres. El formulario actual manda 0 o 1. |
| `about` | M | Sí | Contiene las cuatro listas siguientes. Todas obligatorias, pueden ir vacías. |
| `about.record_ids` | L de S | Sí | Vacía para una revisión general, o el ID de la lectura del gráfico. |
| `about.event_ids` | L de S | Sí | El formulario actual manda `[]`. |
| `about.evaluation_ids` | L de S | Sí | El formulario actual manda `[]`. |
| `about.suggestion_ids` | L de S | Sí | El formulario actual manda `[]`. |
| `created_by` | M | Sí | Backend. `{ "type": "ground_reviewer", "id": "<UUID del usuario de Tierra autenticado>" }`. |
| `created_at` | S | Sí | Backend. UTC en ISO 8601 con milisegundos. |
| `received_at` | S | Sí | Backend. UTC en ISO 8601 con milisegundos. |
| `seq` | N | Sí | Backend. Posición en el diario de cambios del astronauta. |
| `schema_version` | N | Sí | Backend. Valor inicial `1`. |

**Validación de `about` (nueva):** cada ID de cualquiera de las cuatro listas debe existir y pertenecer al mismo astronauta. Si un ID no existe todavía, se rechaza con `unknown_reference` (reintentable, contrato §4). Si es de otro astronauta, se rechaza con `invalid_value`. Los campos `created_by`, `created_at`, `received_at`, `seq` y `schema_version` los pone siempre el servidor; si el cliente los manda, se ignoran (contrato §8).

## Ejemplo

La API habla JSON normal.

```json
{
  "astronaut_id": "4859a2f0-7352-4e63-8804-76d333dc14cb",
  "review_id": "ddb2f267-6d37-5e54-b03b-074ffa246638",
  "created_key": "2026-11-14T21:05:33.120Z#ddb2f267-6d37-5e54-b03b-074ffa246638",
  "summary": "Flag this reading for review at the next check-in.",
  "attention_level": "soon",
  "recommendations": ["Compare with the telemetry from the previous check-in."],
  "about": {
    "record_ids": ["0193a5c4-7e21-7b3a-9c4d-2f8e1a6b5c3d"],
    "event_ids": [],
    "evaluation_ids": [],
    "suggestion_ids": []
  },
  "created_by": { "type": "ground_reviewer", "id": "329fb015-dff4-485f-9be9-f90b946ed343" },
  "created_at": "2026-11-14T21:05:33.120Z",
  "received_at": "2026-11-14T21:05:33.120Z",
  "seq": 130,
  "schema_version": 1
}
```

`summary` y `recommendations` van en inglés porque la app los muestra al astronauta.

## API

### Crear: `POST /v1/astronauts/{astronaut_id}/specialist-reviews`
- Requiere un token de `ground_reviewer` y la cabecera `Idempotency-Key`.
- Cuerpo: `{ summary, attention_level, recommendations, about }`. Todo lo demás lo pone el servidor.
- Es inmutable: no hay `PUT` ni `DELETE`. Para corregir una revisión se crea otra.
- Escritura con `TransactWriteItems` (contrato §4), tres operaciones:
  1. `Put` de la revisión con `attribute_not_exists(#pk)`.
  2. `Put` en `sync_changes` con `seq = n + 1`, `entity = "specialist_review"`, `entity_id = review_id` y `payload` = copia completa de la revisión.
  3. `Update` de `sync_heads` (`last_seq = n + 1`, con la condición sobre el valor esperado).
- Si se repite la `Idempotency-Key`:
  - con el mismo cuerpo → `200` con la revisión ya guardada (`duplicate`);
  - con un cuerpo distinto → `409` `id_conflict`.
  - Se compara solo el cuerpo de entrada, sin los campos que pone el servidor.

### Listar: `GET /v1/astronauts/{astronaut_id}/specialist-reviews?limit=50&cursor=…`
- Consulta el índice `by_created` con `ScanIndexForward=false`: **la más nueva primero**.
- `limit` va de 1 a 50 (por defecto 50).
- El cursor es opaco, se genera a partir de `LastEvaluatedKey` y va firmado por el servidor.
- Respuesta: `{ "reviews": [...], "next_cursor": "…" | null }`. Es `null` en la última página.
- Como el GSI es de consistencia eventual, una revisión recién creada puede tardar unos segundos en salir aquí. El móvil no usa este endpoint: recibe las revisiones por `GET /v1/sync/pull`.

## Panel
Un aviso de especialista es una revisión con `attention_level: "soon"`. Sale en **Pulsys reviews**. Para que salga también en **Alert log**, el panel tiene que mezclar estos avisos con las alertas de telemetría.

## En la app del astronauta
- Recibe estas revisiones por la descarga del diario, con `entity = "specialist_review"`. Mientras no sepa mostrarlas, las guarda íntegras y no se pierde nada.
- Una revisión se muestra junto al estado que calcula la app y **nunca rebaja una alarma** de la app (contrato §3).

## Pendiente de acordar
Esta tabla se solapa con `health_analyses` del contrato: mismos campos `summary`, `recommendations`, `attention_level` y `about`, y la ruta `POST /v1/astronauts/{id}/analyses`. Hay que elegir una de dos:
- **A.** Usar `health_analyses` con `author_type: "doctor"` y borrar esta tabla.
- **B.** Quedarse con esta tabla y actualizar el contrato. Entonces hay que decidir:
  - dónde van los análisis del modelo de Tierra (`ground_service`);
  - si se añade `message` (mensaje directo del médico), que aquí no existe.

Además, el nombre "review" se confunde con `suggestion_review` (cuando Tierra revisa una recomendación del SLM). Si se elige B, mejor llamarla `specialist_note`.
