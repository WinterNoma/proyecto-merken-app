# Informe: telemetría local con trazabilidad (parte interfaz)

## Qué se hizo

1. **Contrato IPC en `ui/src/core-api.ts`**:
   - Se añadieron los tipos `TelemetryIndicator`, `TelemetryValue`, `TelemetryReadingInput`, `RecordTelemetryInput`, `RecordTelemetryResult`, `ListTelemetryInput`, `TelemetryItem`, `VerifyTelemetryInput` y `VerifyTelemetryResult` usando `Iso` (RFC 3339) para `observed_at`, `from` y `to`.
   - Se añadieron `recordTelemetry`, `listTelemetry` y `verifyTelemetry` dentro de `core` siguiendo el mismo patrón de `logRecord`.

2. **Tipos de perfil en `ui/src/types.ts`**:
   - Se añadieron los campos opcionales `hrv?: number | null` y `body_battery?: number | null` a `AstronautTelemetry` para soportar esos indicadores cuando aparezcan en `/api/astronauts` sin romper consumidores ni fixtures existentes.

3. **Conversión y persistencia en `ui/src/ui/telemetry.ts`**:
   - Se exportó `TelemetryPersistState` y se añadió `persist?: TelemetryPersistState` en `FeedState`.
   - Se exportó la función pura `crewToReadings(crew)` que transforma `AstronautProfile[]` en `TelemetryReadingInput[]` (con `astronaut_key` como texto, ignorando `null`/no finitos, exigiendo sistólica + diastólica válidas para `blood_pressure` y texto no vacío para `mood`).
   - En `parseCrew`, si la fila incluye `hrv` o `body_battery`, se sanean con `reading(...)`; la firma pública de `parseCrew` y `createTelemetryFeed` no cambia.
   - En `createTelemetryFeed`, tras cada refresco exitoso de `/api/astronauts`, si existe `window.__TAURI_INTERNALS__`, envía las lecturas en bloques secuenciales de máximo 64 (`persistInFlight` impide más de una llamada en vuelo), sin reintentos en bucle y capturando cualquier rechazo en `state.persist` sin romper `live`, `crew` ni `lastSuccess`.

4. **Panel de historial y trazabilidad en `ui/src/ui/telemetry-history.ts` y `ui/src/hybrid.css`**:
   - Se creó `mountTelemetryHistory(host, getCrew)` devolviendo `{ refresh, destroy }`, sin conectarlo a `ui/index.html` ni a `ui/src/main.ts`.
   - Incluye selector de astronauta e indicador (los 12 indicadores permitidos), botón de refresco, estado accesible (`role="status"`, `aria-live="polite"`), línea de trazabilidad (`"N readings · chain verified ✓"` o `"chain broken at #id"`), gráfica SVG de hasta 200 puntos (con `role="img"`, `aria-label` y `<title>`) y lista de las últimas 10 lecturas con hora UTC, valor y unidad.
   - Cubre los estados: cargando (`Loading history…`), vacío (`No readings stored yet`), error y fuera de Tauri (`History is available in the app`).
   - Se añadieron los estilos móviles (~390 px, controles ≥ 44 px) al final de `ui/src/hybrid.css` bajo el prefijo `.tele-hist-` reutilizando las variables de tema existentes.

## Archivos tocados

- `ui/src/core-api.ts` (modificado)
- `ui/src/types.ts` (modificado)
- `ui/src/ui/telemetry.ts` (modificado)
- `ui/src/ui/telemetry-history.ts` (nuevo)
- `ui/src/hybrid.css` (solo bloque `.tele-hist-` añadido al final)
- `reviews/telemetria-local-ui-informe.md` (este informe)

## Decisiones tomadas fuera de la especificación literal

- **`space_radiation` y `distance_from_earth` (`usv_h` y `km` vs `value`)**: siguiendo la instrucción explícita de copiar las claves de valor que ya usa `core-api.ts` para los indicadores existentes, `crewToReadings` emite `{ usv_h }` para `space_radiation` y `{ km }` para `distance_from_earth`, `{ hours }` para `sleep` (sin ventana ni calidad porque el feed MySQL no las trae), `{ label }` para `mood` (texto libre), `{ ms }` para `hrv` y `{ value }` para `body_battery`. En lectura (`telemetry-history.ts`), `extractValue` acepta de forma defensiva todas esas claves incluida `value`, por compatibilidad si Rust serializara cualquiera de las dos.
- **`persist?: TelemetryPersistState` opcional en `FeedState`**: se dejó opcional para no romper `ui/src/ui/visual-preview.ts`, que construye un literal `FeedState` y no debía tocarse.
- **Presión arterial y ánimo en el gráfico SVG**: para `blood_pressure` se dibujan dos polilíneas (sistólica sólida y diastólica discontinua) sobre una escala compartida; para `mood` (valores de texto) no se dibuja línea numérica y se muestra un aviso explicativo manteniendo la lista de las últimas 10 lecturas.
- **Sin runner de tests en el repo**: el proyecto no tiene framework de tests en `ui/`; se dejó `crewToReadings` pura y exportada y se ejecutaron pruebas unitarias con `node --test` (Node v24.14.1) compilando un paquete temporal en el directorio de scratch sin añadir dependencias ni archivos de test al repo.

## Qué no se pudo hacer

- **Integración visual en vivo dentro de Tauri / WebView Android a 390 px**: no se ejecutó `cargo` ni la app nativa de Tauri (el otro agente trabaja en Rust en paralelo), y `mountTelemetryHistory` aún no está montado en `ui/index.html` / `ui/src/main.ts` según lo pedido; por ello la verificación visual en pantalla real a 390 px queda pendiente para la integración final.
- **`npx vite build` dentro del sandbox**: en Windows el sandbox bloqueó la escritura en `node_modules\.vite-temp\` con `EPERM`, por lo que `npx.cmd vite build` se ejecutó fuera del sandbox (`BypassSandbox: true`).

## Salida resumida de la verificación

- `npx.cmd tsc --noEmit`: código de salida `0` (sin errores).
- `npx.cmd vite build`: código de salida `0` (`33 modules transformed`, `dist/assets/index-DbQtUzTP.css 46.02 kB`, `dist/assets/index-BKcpURgv.js 64.83 kB`, `built in 164ms`).
- Pruebas unitarias en Node (`node --test` sobre bundle temporal en scratch con `window`, `fetch`, `document` y `__TAURI_INTERNALS__` simulados): `6/6` pasadas (`pass 6, fail 0, duration_ms 326.776`), cubriendo:
  - `parseCrew` → `crewToReadings` para los 12 indicadores, omisión de nulos/incompletos y `astronaut_key` como texto;
  - `createTelemetryFeed` fuera de Tauri (sin invocaciones IPC) y en Tauri (lotes secuenciales `64, 64, 2`, `maxConcurrent === 1`, captura de error IPC sin romper `live`);
  - envoltorios `core.recordTelemetry`, `core.listTelemetry`, `core.verifyTelemetry`;
  - `mountTelemetryHistory` en estados fuera de Tauri, vacío, error, cadena verificada/rota, lista de 10 lecturas, SVG normal, doble línea en presión arterial, punto único y `mood`.

## Correcciones tras la revisión de Sol

1. **Hallazgo 4 — Persistencia separada del feed en `ui/src/ui/telemetry.ts`**:
   - Tras completar la petición HTTP, `refresh()` actualiza `state`, libera `inFlight = false` y publica inmediatamente `onUpdate(state)` sin esperar al IPC.
   - Se incorporó una cola serial de un solo hueco (`pendingReadings` + `drainPersistQueue`) con política «gana la última»: cualquier refresco nuevo mientras hay un guardado en curso sobrescribe el lote pendiente y descarta los intermedios.
   - Cada llamada IPC (`core.recordTelemetry`) se acota a 10 s mediante `Promise.race` (`runWithIpcTimeout`); al vencer, registra el error en `state.persist`, libera la cola y vuelve a invocar `onUpdate`.
   - `stop()` vacía `pendingReadings`, detiene el temporizador y cancela la espera activa del IPC dejando la cola completamente inerte.

2. **Hallazgo 5 — Normalización única de identidad en `ui/src/ui/telemetry.ts`**:
   - `parseCrew` normaliza `String(row.id).trim()` una sola vez y valida duplicados después de normalizar (rechazando `"a1"` junto a `" a1 "` con `Duplicate crew identity`).
   - `crewToReadings`, el selector del historial y las consultas `listTelemetry` / `verifyTelemetry` reutilizan exactamente esa clave normalizada sin recortes adicionales.

3. **Hallazgo 6 — Coste del panel y nuevo contrato de verificación en `ui/src/core-api.ts`, `ui/src/ui/telemetry-history.ts` y `ui/src/hybrid.css`**:
   - Se añadió `TelemetryProblem` y se actualizó `VerifyTelemetryResult` en `ui/src/core-api.ts` con `series_checked` y `problem`.
   - El refresco habitual (`refresh()`) solo invoca `listTelemetry`. `verifyTelemetry` únicamente se ejecuta al montar, al cambiar de astronauta (como máximo una vez cada 60 s por astronauta, con caché del último resultado para conservar el texto al alternar) y mediante el botón `"Verify chain"` (deshabilitado mientras corre). El bloqueo `verifyInFlight` impide dos verificaciones simultáneas.
   - Los cambios rápidos en los selectores de astronauta e indicador se agrupan con un debounce de 250 ms (`SELECTOR_DEBOUNCE_MS`).
   - Los textos de trazabilidad muestran `"N readings · chain consistent ✓"` o `"chain problem: <texto legible> at #id"` (sin `"at #id"` si `first_bad_reading_id` es `null`) y eliminan la palabra `"verified"`. Se añadió el texto de ayuda `"Detects edits and missing records on this device."` tanto en atributo `title` como en texto visible (`.tele-hist-help`), junto con el bloque `.tele-hist-trace-box` / `.tele-hist-verify` en `ui/src/hybrid.css`.

4. **Hallazgo 8 — Limpieza de `aria-busy` en todas las salidas**:
   - `clearLoadingBusy()` retira `aria-busy` en todas las salidas del estado de carga: tripulación vacía, ejecución fuera de Tauri, error de consulta, respuesta obsoleta (`seq !== requestSeq` sin nueva carga activa) y `destroy()`.

5. **Hallazgo 7 — Pruebas reproducibles en el repositorio (`ui/tests/telemetry.test.mjs` y `package.json`)**:
   - Se añadió `ui/tests/telemetry.test.mjs` usando `node --test` y empaquetando los `.ts` en caliente en un directorio temporal mediante `rolldown` (el empaquetador que ya trae Vite 8 en `node_modules`, dado que Vite 8 no incluye `esbuild`), sin añadir dependencias nuevas.
   - Se añadió el script `"test:ui": "node --test ui/tests/telemetry.test.mjs"` en `package.json`.
   - Verificación tras los cambios:
     - `npx.cmd tsc --noEmit`: código de salida `0` (sin errores).
     - `npx.cmd vite build` (`BypassSandbox: true` por `EPERM` del sandbox en `node_modules\.vite-temp\`): código de salida `0` (`34 modules transformed`, `dist/assets/index-HAWIQ_dX.css 46.92 kB`, `dist/assets/index-ByzAe-i8.js 75.42 kB`, `built in 169ms`).
     - `npm.cmd run test:ui`: `8/8` pruebas pasadas (`pass 8, fail 0`, `duration_ms 192.1483`), cubriendo `parseCrew` (`"a1"` vs `" a1 "`), `crewToReadings`, fuera de Tauri, troceado `64 / 64 / 2`, cola con IPC bloqueado / «gana la última» / timeout 10 s / `stop()`, `aria-busy` en todas las salidas, debounce 250 ms, cooldown 60 s, exclusión mutua de `verifyTelemetry` y textos de trazabilidad/ayuda.

## Ronda 3

### Cambios realizados

- **N1 — Contrato compartido y prueba fuera de Tauri (`ui/tests/telemetry.test.mjs`)**
  - Añadida la prueba `N1 contrato compartido: parseCrew + crewToReadings coincide con el fixture telemetry-record-batch.json` que carga `docs/api/fixtures/telemetry-record-batch.json`, ejecuta `parseCrew([fixture.crew_row])` → `crewToReadings`, ordena ambos arreglos por `indicator` y exige `assert.deepStrictEqual` con `fixture.readings`. No se modificó el fixture; todas las claves (`space_radiation` `{usv_h}`, `distance_from_earth` `{km}`, `glucose` `{mg_dl}`, etc.) y unidades coinciden con el contrato actual.
  - Corregido el test `createTelemetryFeed fuera de Tauri publica el feed sin invocar IPC`: `invokeCount` ahora envuelve `coreMod.core.recordTelemetry` (en el bundle compartido de `rolldown`) con restauración en `finally`, verificando realmente que no se realiza ninguna llamada IPC (`invokeCount === 0`).

- **N3 — Exclusión real en la cola IPC con aviso de lentitud (`ui/src/ui/telemetry.ts`)**
  - Sustituido `runWithIpcTimeout` (`Promise.race`) por `awaitIpcWithSlowWarning`, que espera a la promesa real de `core.recordTelemetry` manteniendo `persistInFlight = true` hasta que dicha promesa se resuelva o rechace.
  - Cuando vence el temporizador de `10_000 ms`, únicamente publica `state.persist` con `{ ok: false, stored, skipped, lastError: "Saving is slow", lastBatchId }` mediante `onUpdate(state)` sin liberar la cola ni permitir un segundo `invoke` simultáneo.
  - Conservada la política «gana la última» (`pendingReadings`) mientras el `invoke` lento sigue en vuelo; al completarse, la cola envía únicamente el último lote pendiente.
  - `stop()` marca `stopped = true`, limpia `pendingReadings`, cancela el intervalo HTTP, aborta el `fetch` activo y limpia el temporizador del IPC lento (`clearActiveIpcTimer`), dejando la cola inerte.
  - Actualizada la prueba en `ui/tests/telemetry.test.mjs` (`N3 cola de guardado...`).

- **Hallazgo 6 — Exclusión de listados en `refresh()` (`ui/src/ui/telemetry-history.ts`)**
  - Implementada exclusión mutua en `refresh()` mediante `listInFlight` y `pendingRefresh`: mientras hay un `listTelemetry` en vuelo, las llamadas adicionales no lanzan otro IPC sino que marcan un único refresco pendiente (`pendingRefresh = true`) y comparten la promesa pendiente hasta que finalice la pasada extra con la selección (`selectedCrewId`, `selectedIndicator`) más reciente.
  - Los atajos síncronos fuera de Tauri (`!isTauriRuntime()`) o sin tripulación (`!selectedCrewId`) siguen resolviendo de inmediato e invalidan respuestas obsoletas vía `requestSeq`.
  - Añadida la prueba `Hallazgo 6: tres refresh() seguidos coalescen a 1 en vuelo + 1 pendiente con la selección más reciente`.

- **N5 — Preservar orden recibido en la lista (`ui/src/ui/telemetry-history.ts`)**
  - Eliminada la reordenación por `observed_at` sobre la lista principal: `buildRecentList` recibe y muestra los elementos en el orden exacto devuelto por `core.listTelemetry` (`reading_id DESC`).
  - `buildLineChart` ordena cronológicamente una copia (`sortChronologicalForChart`, separada por `observed_at` ascendente y `reading_id` ascendente en empates) sin mutar el arreglo original ni afectar a la lista.
  - Añadida la prueba `N5: la lista conserva el orden recibido [2, 1] aunque observed_at venga invertido`.

- **N6 — Etiqueta de integridad con hora UTC, alcance y lecturas nuevas (`ui/src/ui/telemetry-history.ts`)**
  - `buildTraceabilityText` ahora formatea siempre la hora UTC y el alcance de la última comprobación: `Checked HH:MM:SS UTC · N readings in chain · consistent ✓` (o `Checked HH:MM:SS UTC · N readings in chain · chain problem: ...`).
  - Cada astronauta guarda una instantánea (`TraceSnapshot`) con el conteo y el máximo `reading_id` observados por indicador en el momento de verificar. Si un `listTelemetry` posterior devuelve mayor cantidad de lecturas o un `reading_id` superior al comprobado, marca `hasNewerReadings = true` y añade ` · newer readings not yet checked` sin disparar verificación automática; el botón `Verify chain` y el cooldown de 60 s se mantienen intactos.
  - Añadida la prueba con reloj simulado `N6: etiqueta de integridad muestra hora UTC, alcance y marca lecturas nuevas sin re-verificar sola`.

- **Nuevo — Indicador de estado de guardado `setPersistStatus` (`ui/src/ui/telemetry-history.ts`, `ui/src/hybrid.css`)**
  - Exportado `TelemetryPersistStatus` y añadido el método `setPersistStatus(persist: TelemetryPersistStatus | null): void` en `TelemetryHistoryHandle` y en el objeto devuelto por `mountTelemetryHistory`.
  - Renderiza un elemento `<p class="tele-hist-persist caption" role="status" aria-live="polite">` (oculto cuando `null`) con los mensajes `"Saving to device: OK · last batch stored N"` (`data-tone="ok"`) o `"Saving to device failed: <mensaje>"` (`data-tone="alarm"`).
  - Añadida la regla `.tele-hist-persist` en el bloque `.tele-hist-` de `ui/src/hybrid.css` y la prueba unitaria correspondiente.

### Verificación

- `npx tsc --noEmit`: limpio (código de salida 0).
- `npm run test:ui`: 13/13 pruebas en verde (2 suites, 0 fallos, 0 omitidas).
- `npx vite build`: falló en el sandbox estándar por `EPERM` al escribir en `node_modules\.vite-temp` y limpiar `dist\assets` (confirmado con `--configLoader native --outDir <scratch>` que el bundle transformaba y empaquetaba los 34 módulos); ejecutado posteriormente con aprobación fuera del sandbox, completando en `152ms` (código de salida 0, 34 módulos transformados).


## Ronda 4

Correcciones de interfaz en telemetría local según los hallazgos de Sol (R3-1 a R3-4):

- **R3-1 (I) — Validación alineada con Rust (`ui/src/ui/telemetry.ts`):**
  - Se exporta la función pura `validateReading(input)` con las cotas de Rust: `astronaut_key` (1–64 caracteres, sin controles `\p{Cc}`), `astronaut_name` (1–100 caracteres, sin controles), `unit` (≤ 16 caracteres), `mood.label` (1–64 caracteres, sin controles), valores numéricos finitos en `value` y JSON de `value` ≤ 2048 bytes UTF-8.
  - `parseCrew` y `partitionCrewReadings` conservan caracteres de control para que no se saneen en silencio; `partitionCrewReadings` / `crewToReadings` excluyen únicamente las lecturas inválidas del lote sin descartar las válidas restantes.
  - `TelemetryPersistState` añade `rejected: number` y `lastRejection: string | null`, actualizados en `createTelemetryFeed` incluso si todas las lecturas de un lote quedan excluidas.

- **R3-2 (I) — Descarte de respuestas de otro astronauta o indicador (`ui/src/ui/telemetry-history.ts`):**
  - `executeListPass` captura `queryCrewId` y `queryIndicator` al lanzar `core.listTelemetry` y descarta la respuesta (éxito o error) si `seq !== requestSeq` o si la selección actual ya no coincide, sin pintar ni registrar datos ni `readings loaded`.
  - `onCrewChange` y `onIndicatorChange` incrementan `requestSeq` para invalidar la consulta activa en vuelo mientras la pasada pendiente se ejecuta con la selección nueva.

- **R3-3 (I) — Vigencia de integridad con `checked_through_reading_id` (`ui/src/core-api.ts`, `ui/src/ui/telemetry-history.ts`):**
  - `VerifyTelemetryResult` incorpora `checked_through_reading_id: number | null`.
  - `telemetry-history.ts` elimina la comparación con el total global (`checked`) y evalúa `getMaxObservedReadingId(crewId)` frente a `checked_through_reading_id` entre cualquier indicador cargado del astronauta: muestra `newer readings not yet checked` cuando se supera `checked_through_reading_id`, y `scope unknown` cuando `checked_through_reading_id` es `null` con lecturas presentes.

- **R3-4 (M) — Estado lento diferenciado de error (`ui/src/ui/telemetry.ts`, `ui/src/ui/telemetry-history.ts`, `ui/src/hybrid.css`):**
  - `TelemetryPersistState` añade `status: "ok" | "slow" | "error"` manteniendo `ok` booleano por compatibilidad (`ok === false` solo en error real).
  - Tras 10 s de espera en `awaitIpcWithSlowWarning`, el estado pasa a `status: "slow"`, `ok: true`, `lastError: null` sin liberar el bloqueo de único `invoke` pendiente, y al concluir pasa a `"ok"` o `"error"`.
  - `setPersistStatus` en `telemetry-history.ts` y los selectores `.tele-hist-persist[data-tone="ok"|"warn"|"alarm"]` en `hybrid.css` reflejan los tres estados con `role="status"`, mostrando `"Saving to device is taking longer than usual; waiting for it to finish."` en `"slow"` y `"Saving to device failed: …"` en `"error"`.

### Verificación
- `npx.cmd tsc --noEmit`: salida 0 (sin errores).
- `npx.cmd vite build`: salida 0 (`34 modules transformed`, `dist/` generado). Nota: en entorno sandbox en Windows `vite` falló con `EPERM` al escribir en `node_modules\.vite-temp`, por lo que `npx.cmd vite build` se ejecutó con `BypassSandbox: true`.
- `npm.cmd run test:ui`: 18/18 pruebas en verde en `ui/tests/telemetry.test.mjs` (incluyendo casos específicos para R3-1, R3-2, R3-3 y R3-4).


## Ronda 5

Se registran los valores numéricos presentes e inválidos por indicador, incluidas las dos partes de presión, sin contar ausencias. Las lecturas válidas se conservan y los números no finitos directos también se rechazan.

Validación: `npx tsc --noEmit` y `npx vite build` correctos; `npm run test:ui` pasó 20 pruebas con `NODE_OPTIONS=--experimental-test-isolation=none`. La ejecución inicial del test con aislamiento falló por `spawn EPERM`; PowerShell también fue denegado, por lo que se usó cmd. No se ejecutó Cargo.
