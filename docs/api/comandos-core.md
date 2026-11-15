# Comandos del núcleo (Rust → interfaz)

Para: Rodrigo (`ui/`) · Versión 1 · 14-nov-2026
Tipos TypeScript: [`ui/src/core-api.ts`](../../ui/src/core-api.ts). Diseño: [decisión 003](../decisiones/003-nucleo-rust.md). Umbrales y fuentes: [umbrales-core.md](umbrales-core.md).

## Cómo se usan

```ts
import { core, isCoreError } from "./core-api";

const status = await core.getStatus();
```

- Todo funciona **sin conexión**: se guarda en SQLite en el móvil y queda en cola para subir a Tierra.
- Todo el JSON va en **inglés** y los textos `message` y `summary` se pueden mostrar tal cual.
- Las fechas son ISO 8601 en UTC con milisegundos (`"2026-11-14T21:05:33.120Z"`).
- **La interfaz no decide nada de salud.** Estados, colores, rangos, tendencias y "need help" los calcula el núcleo. Si un dato no viene, se muestra `No data` o `Not evaluated`, nunca un valor inventado.
- Si un comando falla, la promesa se rechaza con un `CoreError` `{ code, field, message }`. `invalid_value` e `invalid_schema` llevan en `field` el campo que hay que corregir.

## Comandos

| Comando | Para qué | Devuelve |
|---|---|---|
| `getProfile()` | Nombre, misión (para el MET), perfil médico y antecedentes | `{ profile \| null, is_demo, core_version, rules_version }` |
| `logRecord(input)` | Guardar una medición o un síntoma | `needs_confirmation` con avisos, o `saved` con el registro y su evaluación |
| `correctRecord(input)` | Corregir un registro (se guarda uno nuevo con `replaces`; nada se borra) | Igual que `logRecord` |
| `listRecords(input)` | Historial para gráficas y listas, del más reciente al más antiguo | `{ records }` |
| `getStatus()` | Estado de cada indicador, figura humana, alertas y sincronización | `StatusResponse` |
| `raiseEvent(input)` | Botón **Need help** (`need_help`) y **How I feel** (`feeling_report`) | `{ event, evaluation \| null }` |
| `listEvents({ limit })` | Historial de eventos | `{ events }` |
| `loadDemo(scenario)` | Cargar datos **DEMO** (`routine`, `persistent_change`, `alarm_event`) | `StatusResponse` |
| `clearDemo()` | Borrar solo los datos DEMO | — |
| `simListAstronauts(token?)` | Astronautas de la **API de simulación** (datos inventados) | `{ astronauts: [{ astronaut_id, display_name }] }` |
| `simImport({ astronaut_id, days?, token? })` | Descargar los registros de un astronauta simulado y guardarlos como DEMO | `{ astronaut_id, display_name, imported, rejected, first_measured_at, last_measured_at }` |

## Registrar un dato (con confirmación de valores raros)

```ts
let r = await core.logRecord({ indicator: "heart_rate", value: { bpm: 210 } });
if (r.status === "needs_confirmation") {
  // Mostrar r.warnings[i].message y preguntar "Save anyway?"
  r = await core.logRecord({ indicator: "heart_rate", value: { bpm: 210 }, confirm_warnings: true });
}
// r.status === "saved": r.record, r.evaluation
```

Formato de `value` por indicador (unidad fija del catálogo, no hace falta enviarla):

| `indicator` | `value` | Notas |
|---|---|---|
| `heart_rate` | `{ bpm: 72 }` | > 0 |
| `blood_pressure` | `{ systolic: 118, diastolic: 76 }` | mmHg, > 0 |
| `body_temperature` | `{ celsius: 36.7 }` | > 0 |
| `glucose` | `{ mg_dl: 95 }` | > 0 |
| `sleep` | `{ hours: 5.5, quality: "poor", window_start: "…Z", window_end: "…Z" }` | `quality`: `poor`, `fair`, `good` o `null`. **Ventana obligatoria** (contrato §5): fin posterior al inicio, ≤ 24 h, y `hours` no mayor que la ventana. Pide "Went to bed" / "Woke up" |
| `mood` | `{ score: 6 }` | Entero 0–10. **0 = very bad, 10 = very good** |
| `oxygen_in_blood` | `{ percentage: 98.2 }` | SpO₂ en %, mayor que 0 y ≤ 100. Pedir medir **en el dedo y en reposo** |
| `space_radiation` | `{ usv_h: 12.5 }` | µSv/h, ≥ 0. **Contexto**: no se evalúa |
| `distance_from_earth` | `{ km: 384400 }` | km desde la superficie de la Tierra (en órbita baja, la altitud: ISS ≈ 400), ≥ 0. **Contexto**: no se evalúa |
| `nutrition` | `{ kcal: 2400 }` | kcal, ≥ 0. **Contexto**: no se evalúa |
| `symptom` | `{ body_zone: "left_foot", score: 4, description: "My foot hurts" }` | Entero 0–10. **0 = none, 10 = worst**; `description` ≤ 2000 caracteres |

`headache` ya no existe (el catálogo lo quitó el 14-nov). Un dolor de cabeza se registra como `symptom` con `body_zone: "head"`.

**Indicadores de contexto** (`space_radiation`, `distance_from_earth`, `nutrition`): se guardan y se suben igual que los demás, pero no tienen estado de salud. En `get_status` salen dentro de `indicators` solo si hay datos, con `status: "not_evaluated"`, `reason: "not_applicable"` y `latest_value`; nunca cambian `overall`. Píntalos como dato, sin color de estado.

Opcionales: `measured_at` (por defecto, ahora), `note` (≤ 2000 caracteres), `source` (`form` por defecto; `text`/`voice` para el intérprete).

Se **rechaza** solo lo que no tiene sentido (texto en vez de número, 0 o negativo, puntuación 11 o 4.5, zona inexistente…). Los valores extremos reales **no** se rechazan: piden confirmación.

## Estados y cómo pintarlos

| `status` | Texto sugerido | Notas |
|---|---|---|
| `not_evaluated` | `Not evaluated` | `reason`: `no_data` → "No data"; `too_few_data` → "Need {history_needed} previous readings ({history_count}/{history_needed})" (`history_count` no cuenta la última); `stale` → "Last reading too old"; `not_applicable` → sin estado, mostrar solo el valor |
| `stable` | `Stable` | Dentro de su historial reciente |
| `change` | `Change` | Respecto a su historial reciente o fuera del rango habitual. **No es una alarma clínica.** `direction` ↑/↓ y `persistent` |
| `alarm` | `Alarm` | Siempre con icono y texto, no solo color |

`escalation` dice qué hacer: `continue` (nada), `self_care` (sugerencias), `flag_for_ground` ("Flagged for ground review"), `urgent` (abrir el flujo de ayuda y avisar a Tierra).
Muestra siempre `source_kind`: si es `demo_heuristic`, añade "Demo rule, not medical advice".

En la figura humana usa `status.body_zones` (las 14 zonas, con el peor estado de sus síntomas). La cabecera puede usar `status.overall`: solo es `stable` si **todas** las series están evaluadas y estables; si falta alguna, es `not_evaluated` y `worst` dice cuáles, y el pie `status.met` y `status.sync` (`3 pending`, `1 not synced`).

## Ayuda y "How I feel"

```ts
await core.raiseEvent({ kind: "need_help", message: "Chest pain" }); // pide confirmación en la interfaz antes de llamarlo
await core.raiseEvent({ kind: "feeling_report", message: "Tired but OK" });
```

`need_help` crea un evento urgente y una alarma en `status.alerts` que **no caduca** (aún no hay forma de marcarla como resuelta; está pendiente con backend). Se guarda aunque no haya conexión; `event.sync` dice si está `pending` o `synced`.

## Datos DEMO

`loadDemo` solo funciona si en el móvil **no** hay datos reales (si los hay, falla con `real_data_present`). Todo lo DEMO lleva `is_demo: true` y **nunca se sube a Tierra**. **El primer dato real (un registro, un `need_help`, el perfil descargado) borra todo lo DEMO**, para que nunca se mezclen: avisa al usuario antes ("Logging real data will remove the demo data"). Muestra un distintivo `DEMO` mientras `is_demo` sea `true`.

## Datos de la API de simulación

La API de simulación (`http://98.95.196.77:8001`, aparte del backend real) tiene 7 astronautas con datos inventados de pulso (cada 15 min), temperatura (cada hora) y sueño (cada noche). Alex Morgan tiene un escenario de pulso alto, y Jordan Lee uno de temperatura y sueño.

```ts
const { astronauts } = await core.simListAstronauts(token);
const r = await core.simImport({ astronaut_id: astronauts[0].astronaut_id, token }); // últimos 14 días
const status = await core.getStatus(); // el núcleo ya los evalúa
```

- Se guardan como **DEMO**: mismas reglas que `loadDemo` (fallan con `real_data_present` si hay datos reales, nunca se suben y el primer dato real los borra). Cada importación **sustituye** la DEMO anterior: no se mezclan astronautas.
- El perfil queda como `SIMULATION <nombre>`, con `is_demo: true`.
- **Token:** nunca en el código. En el PC, la variable de entorno `MERKEN_SIM_TOKEN` antes de `npm run tauri dev`; en el móvil, un campo en la pantalla que lo pase como `token`. La URL se puede cambiar con `MERKEN_SIM_URL`.
- La petición sale desde Rust, no desde la WebView, así que no hay CORS. La API es HTTP sin cifrar: solo para la demo.
- `rejected` cuenta los registros descartados: duplicados, de otro astronauta, con fecha posterior a ahora + 10 min, con `schema_version` distinto de 1, sin `record_id` o con textos demasiado largos, y los que no pasan la validación del núcleo.
- Todo o nada: si la descarga falla o pasa de 20 000 registros, la DEMO anterior queda intacta.
- Errores nuevos: `missing_token`, `unauthorized` (token rechazado), `network`, `not_found` (astronauta), `http_error`, `bad_response` (respuesta incoherente), `too_much_data` (más de 20 000 registros o respuestas demasiado grandes: importar menos días).
- Tarda unos segundos: unas 18 páginas de 100 registros para 14 días.

## Lo que aún no hay

- Login y descarga del perfil real: llegan con la sincronización (`crates/sync`), cuando el backend tenga URL. Hasta entonces `getProfile` devuelve `profile: null` salvo con DEMO.
- Subida y descarga con Tierra: los datos quedan en la cola (`sync.pending`).


## Interpreter and explainer (SLM)

`interpretText` returns a proposal and a layer-2 status; it does not save anything. Show fields from `proposal` and `layer2_proposal` alongside each literal `quote`, labeling layer-2 fields **interpreted by AI**. Keep the original input. The user confirms or corrects every field. Put both `unresolved` and `proposal.uninterpreted` fragments and ambiguous times in the form. Once the user confirms each record, use `logRecord` with `source: "text"` or `"voice"`, including the usual warning confirmation flow. Negated mentions are not records.

Both `interpret_text` and `explain_indicator` require `input.request_id`: a caller-generated string of 1–64 ASCII characters from `[A-Za-z0-9_-]`. Use a fresh ID for every UI request, including retries. A duplicate ID still admitted to the service returns `busy`; malformed IDs return `invalid_value` with field `request_id`. Input `source` and `body_zone` may be omitted or null.

`explainIndicator` obtains the evaluation from the local store in Rust. Acquiring `Mutex<Store>` and calling `Store::status` both happen in `spawn_blocking`; the guard is released before waiting for the model. Like `get_status`, this can persist deterministic evaluations and enqueue them for synchronization. It never saves SLM proposals or unconfirmed measurements. Always show its deterministic facts. When a note exists, show its label **AI-generated suggestion** and its curated statements. Selection is restricted to statements applicable to the existing state and cannot change that state. There are currently no curated fichas, so the note is null with `no_fichas`. Fact IDs are the closed set `indicator`, `status`, `reason`, `direction`, `latest_reading`, `baseline`, `history`, `escalation`, `source`.

Layer 2 can be `not_needed`, `unavailable`, `used`, `model_unavailable`, `cancelled`, `timeout`, `discarded` or `slm_error`. The rules proposal is unchanged. `layer2_proposal` is separate and contains only checked SLM records for indicators absent from the rules proposal, with each full quote inside `unresolved` or `proposal.uninterpreted`; it is null if none survive. The filtered proposal is checked against the original input. `note_status` can be `available`, `no_fichas`, `model_unavailable`, `cancelled`, `timeout`, `discarded` or `slm_error`. Failures do not expose engine output.

Residual coverage uses complete Unicode alphanumeric words (including combining marks and decimal separators inside numeric tokens) with UTF-8 byte offsets into the original input. Citation matching compares complete word sequences and their exact original spacing and punctuation; it never covers substrings of another word or number. For example, `72` cannot cover any part of `172`. Punctuation has no content; the connector list is `y, e, pero, también, tambien, and, but, also` (case-insensitive).

Inside a record citation, a word is covered only by an unambiguous occurrence of a field citation, its ambiguous time citation, a mention of that record's indicator, or the function-word list below. Indicator mentions are tested with the core's `mentions_indicator` on single words and contiguous pairs/triples, preserving punctuation such as the slash in `mg/dL`. Only minimal mentions cover their words: a phrase containing a shorter indicator mention cannot cover arbitrary neighbours. Mention coverage is confined to the record citation. Repeated field/time evidence within a broad citation is conservatively left pending. The record's evidence citation itself is never shortened or treated as full semantic coverage.

The closed function-word list is `de, la, el, los, las, un, una, me, mi, my, the, a, of, en, in, con, with, al, del, at, i` (case-insensitive; `a las` is covered as two words). A negated fragment is covered only if it names an indicator (including symptoms) and every content word is either a minimal indicator mention, a function word, or one of these closed negation scaffold particles: `no, not, sin, without, nunca, never, ni, nor, tengo, have, has, siento, feel, hay, any`. Thus `no tengo fiebre` and `no headache` are understood, while `no pegué ojo` remains pending in full. Any unaccounted content inside a negation keeps the entire negated fragment pending.

Unresolved fragments are contiguous original slices from the first to the last uncovered word. Intermediate function words and immediately preceding function words are retained; connectors separate ordinary fragments. This preserves `estoy agotado` in `pulso 72 estoy agotado` and **`I feel exhausted`** in `pulse 72 I feel exhausted` (the chosen English form includes `I`). Text already shown in `proposal.uninterpreted` is not duplicated in `unresolved`; it triggers layer 2 only when it contains content beyond punctuation/connectors. When uncertain, keep content pending rather than silently considering it interpreted.

`slmCancel(requestId)` invokes `slm_cancel({ request_id })`. The service creates the cancellation token when admitting a job, including queued jobs. Cancellation affects exactly that ID. It returns false for an unknown or completed ID and never cancels a successor. Cancellation is cooperative; a queued cancelled job is rejected before model loading. After a command-side timeout its ID is removed and late cancellation returns false, while its cancelled native work may still be unwinding.

The layer-1 proposal, residual text and `render_facts` run on the command's blocking thread, outside the optional model worker. Jobs have a 120-second cooperative model deadline including queue time. The command waits with `recv_timeout` for the remaining deadline plus a 2-second margin, returning the unchanged deterministic result with `timeout` if the wait expires. The token remains cancelled in the background. This bounds the wait for the model, not the preceding database acquisition/evaluation or blocking-pool scheduling. A full queue (two waiting jobs), or native work still alive after a response timeout, returns `busy`.

If a worker channel closes or a panic unwinds, the response keeps its deterministic proposal/residual or facts with `slm_error`, without a model proposal/note. Tokens are cleaned on completion, timeout, admission failure and worker unwind. The next request restarts a finished worker; the service checks thread completion before restarting, so two model engines cannot coexist. A worker still alive and stuck is not replaced. With the workspace release setting `panic = "abort"`, a Rust/native abort ends the entire process; unwind recovery cannot handle that case.

`slmStatus` reports installed, verified, loaded and backend availability without paths. `slmRelease()` invokes `slm_release()` and unloads the single model slot on the worker (RNF-09); it returns true if a model was unloaded, false if already empty. The UI should await it when entering record entry. It does not load an engine, persist health data or confirm records; subsequent inference reloads on demand. Release uses the same bounded queue and command wait; if the worker is still hung, `busy` is returned. A release wait timeout returns `timeout`; a release already queued can still finish later.

Tauri emits `slm-progress` with `{ request_id, phase, tokens, max_tokens }`. Phases are `loading`, `generating`, `validating`, `done`; token counts are zero outside generation. Events contain no input, prompt or generated text. Ignore events for stale IDs. Visible Tauri messages are centralized in `src-tauri/src/strings.rs` in English (RNF-14).

For manual desktop installation, create `<app_data_dir>/models/space-health-v5/` and copy both `space-health-base-Q4_K_M.gguf` and `space-health-lora-f16.gguf` from the v5 package. In a debug build `MERKEN_SLM_MODELS_DIR` selects the parent models directory. The embedded trusted manifest fixes sizes and SHA-256; a disk manifest is not trusted. Build with `slm-llama` to enable the backend. No interpreter model or curated fichas currently ships. Android integration and benchmarking remain Phase 3.

```ts
await core.slmRelease(); // Entering record entry; await model unloading first.
const requestId = crypto.randomUUID();
const { proposal, layer2_proposal, unresolved } = await core.interpretText({
  request_id: requestId, text, source: "voice",
});
// Present both proposals and their quotes; label layer2_proposal fields as interpreted by AI.
// Put unresolved, proposal.uninterpreted and ambiguous times in the form.
// After explicit confirmation and constructing a valid value:
await core.logRecord({ indicator, value, source: "voice" });
// On cancellation of this specific request:
await core.slmCancel(requestId);

const explanation = await core.explainIndicator({
  request_id: crypto.randomUUID(), indicator: "heart_rate", body_zone: null,
});
for (const fact of explanation.facts) show(fact.text);
if (explanation.note) show(explanation.note.label);
```
