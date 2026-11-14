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

## Lo que aún no hay

- Login y descarga del perfil real: llegan con la sincronización (`crates/sync`), cuando el backend tenga URL. Hasta entonces `getProfile` devuelve `profile: null` salvo con DEMO.
- Subida y descarga con Tierra: los datos quedan en la cola (`sync.pending`).
