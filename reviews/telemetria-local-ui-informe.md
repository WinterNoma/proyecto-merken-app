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
