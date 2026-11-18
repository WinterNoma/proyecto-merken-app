# Inventario de la UI anterior (Tauri)

## Informe (formato AGENTS.md)

1. **Qué hiciste**: Se realizó un inventario estático completo de la interfaz Tauri anterior (`ui/index.html`, `ui/src/main.ts`, los módulos de `ui/src/ui/*.ts`, las hojas de estilo `ui/src/*.css` y la captura `docs/referencias/capturas/merken-movil.png`) junto con los límites en Rust (`src-tauri/src/*.rs`, `crates/slm/src/*.rs`, `crates/core/src/interpret/*`, `crates/local-speech/src/lib.rs`). Se documentaron páginas, orden, navegación, elementos de arriba abajo con sus comandos Tauri, controles globales, el flujo real del asistente/SLM y dictado por voz, y el código roto, incompleto o con errores silenciados.
2. **Archivos tocados**: [reviews/inventario-ui-anterior.md](file:///C:/Users/Salva/Desktop/git/nasa.spaceapps/reviews/inventario-ui-anterior.md) (único archivo creado; tarea de solo lectura).
3. **Comandos ejecutados**: **NO EJECUTADO** — La tarea prohíbe explícitamente ejecutar comandos (`no ejecutes comandos`). Todo el análisis se basó en lectura directa de archivos con `read_file`.
4. **Pendiente o en duda**: La implementación nativa Android del plugin `local-speech` (`com.pulsys.speech.LocalSpeechPlugin`) y posibles `TODO` fuera de los archivos leídos quedan marcados como **NO VERIFICADO** al no poder listar directorios ni ejecutar búsquedas por comando.

---

## 1) Páginas, orden, inicio y navegación

### Páginas y orden (izquierda / centro / derecha)

Definido en el array `screens` de `ui/src/ui/page-navigation.ts:3-7` (aunque en `ui/index.html:21-120` aparecen en orden `#overview`, `#core-panel`, `#recommendations-page`, `setupPages` los reordena dentro de `.page-rail` según `screens` en `ui/src/ui/page-navigation.ts:25-33`, dispuestos horizontalmente con `grid-auto-flow: column; grid-auto-columns: 100%` en `ui/src/hybrid.css:62-66`):

| Posición | Índice | ID de sección | Etiqueta en paginación (`page-navigation.ts:4-6`) | Título `<h2>` en HTML |
| :--- | :---: | :--- | :--- | :--- |
| **Izquierda** | `0` | `#core-panel` | `"History"` | `"History / On-device record"` (`ui/index.html:51`) |
| **Centro** | `1` | `#overview` | `"Daily overview"` | `"Daily overview"` (`ui/index.html:23`) |
| **Derecha** | `2` | `#recommendations-page` | `"Recommendations"` | `"Recommendations"` (`ui/index.html:76`) |

### Página inicial al abrir la app

- Abre en la **página central (`#overview` / `"Daily overview"`, índice `1`)**: `const overview = pages.findIndex(page => page.id === "overview"); let active = overview;` (`ui/src/ui/page-navigation.ts:34-36`), aplicando `translate3d(-100%, 0, 0)` inicial en `position()` (`ui/src/ui/page-navigation.ts:41-44,83`).

### Cómo se navegaba entre páginas

1. **Gestos táctiles / puntero (swipe horizontal)** sobre `#screens` (`ui/src/ui/page-navigation.ts:96-164`):
   - Ignora el inicio de arrastre si el objetivo está dentro de `input, select, textarea, summary, a, [contenteditable], .voice-entry, .body-map` (`ui/src/ui/page-navigation.ts:100`).
   - El scroll vertical nativo (`touch-action: pan-y pinch-zoom` en `ui/src/hybrid.css:54,70`) tiene prioridad: exige mover al menos `8px` (`ui/src/ui/page-navigation.ts:113`) y que `Math.abs(dx) > Math.abs(dy) * 1.2` (`ui/src/ui/page-navigation.ts:114-117`).
   - En los bordes extremos aplica resistencia `dx * 0.12` (`ui/src/ui/page-navigation.ts:127-130`).
   - Cambia de página si al soltar `Math.abs(offset) >= clamp(36, 60, clientWidth * 0.16)` o si hay flick rápido (`Math.abs(offset) > 16 && elapsed < 100ms && velocity * direction > 0.45`) (`ui/src/ui/page-navigation.ts:147-152`).
   - Suprime clics accidentales durante `400 ms` tras un arrastre (`ui/src/ui/page-navigation.ts:146,160-164`).
2. **Indicadores inferiores (`.screen-pagination`)** (`ui/index.html:122-127`, `ui/src/ui/page-navigation.ts:27,46-58,88`):
   - 3 botones `.page-dot` (`data-page="0" | "1" | "2"`) con `aria-current="page"` en el activo y una etiqueta `#page-name` con el nombre de la página actual.
   - *Diferencia respecto a la captura `docs/referencias/capturas/merken-movil.png`*: la captura muestra **2 puntos** (píldora blanca activa a la izquierda + círculo vacío a la derecha) y el texto `Overview`, reflejando una iteración anterior a la incorporación de la 3.ª página (`Recommendations`).
3. **Botones y enlaces de acceso directo**:
   - Wordmark `.wordmark` (`PULSYS`) en la cabecera: lleva a `overview` (`ui/src/ui/page-navigation.ts:91-94`).
   - Botón `#overview-alerts` (`"View persistent alerts (N)"`) en `overview`: lleva a `core-panel` (`History`, `ui/src/ui/page-navigation.ts:90`).
   - Tarjeta `#sync-open` (`"Sync"`) en `overview`: lleva a `core-panel` (`History`, `ui/src/ui/page-navigation.ts:89`).
4. **Efecto colateral de navegación**: al entrar en `"core-panel"`, `ui/src/main.ts:62-70` llama a `history.refresh()` e inicia un `setInterval` cada `10000 ms` para refrescar el panel `#telemetry-history`.

---

## 2) Elementos de arriba abajo por página

### Página izquierda (índice 0): `#core-panel` — `"History / On-device record"`

Fuentes: `ui/index.html:50-73`, `ui/src/ui/core-panel.ts`, `ui/src/ui/telemetry-history.ts`, `ui/src/ui/sparkline.ts`, `ui/src/styles.css:249-252`.

1. **Cabecera de sección (`.section-heading`)** (`ui/index.html:51`):
   - Eyebrow `"02 / Device record"` + título `h2#core-title` (`"History / On-device record"`).
   - Badge `#demo-badge` (`"DEMO"`), visible solo si `status.is_demo === true` (`ui/src/ui/core-panel.ts:242`).
2. **Banner de disponibilidad del servicio local (`#core-availability`)** (`ui/index.html:52-55`, `ui/src/ui/core-panel.ts:241,284-296,362-366`):
   - Muestra `#core-availability-title`, `#core-availability-copy` y botón `#core-retry` (`"Try again"`, llama `panel.refresh(true)` en `core-panel.ts:333`).
   - Comandos Tauri de `panel.refresh()`: `get_status`, `get_profile`, `list_records` (`limit: 1`), `list_events` (`limit: 1`) (`core-panel.ts:261-263`) y `list_records` (`limit: 40`) para los 5 indicadores con gráfica (`core-panel.ts:9,276`).
3. **Lista de alertas persistentes (`#core-alerts` + `#alarm-announcement`)** (`ui/index.html:56-57`, `ui/src/ui/core-panel.ts:170-224`):
   - Una tarjeta `.alarm-notice` por cada entrada de `status.alerts` (`get_status`): chip de estado, nombre de indicador/evento, `evaluation.summary`, escalado + `"Persistent event"`, aviso local/DEMO, estado de cola (`status.sync.pending`, `status.sync.failed`) y `<details>` `"View event details"` enriquecido con mensajes de `list_events` (`core-panel.ts:183,201-206`).
4. **Registro del núcleo (`#core-record`)** (`ui/index.html:58-71`; en escritorio `>= 900px` `.core-summary` va en columna 1 y el resto en columna 2, `ui/src/styles.css:249-251`):
   - **4a. Tarjeta resumen (`.core-summary`)** (`ui/index.html:59-65`, `ui/src/ui/core-panel.ts:244-251`):
     - `"On-device profile"`, `#core-profile` (`get_profile` -> `profile.astronaut.display_name`), `#core-mission` (`profile.astronaut.mission.name`), `#demo-copy` (si `status.is_demo`).
     - Chip `#core-overall` (`status.overall.status` de `get_status`).
     - Lista `<dl class="record-list">`: `Escalation` (`#core-escalation`), `Indicators to review` (`#core-worst`), `Mission elapsed time` (`#core-met`), `Synchronization` (`#core-sync`).
     - Pie `#core-evaluated` (`"Evaluated <UTC>"`, de `status.evaluated_at`).
   - **4b. Selector de datos DEMO (`#demo-empty`)** (`ui/index.html:66`, `ui/src/ui/core-panel.ts:269-275,324-360`):
     - Visible cuando el almacén local está vacío o ya tiene datos DEMO (salvo si `load_demo` falló con `real_data_present`, `core-panel.ts:275,350-353`).
     - `<select id="demo-scenario">` (`"routine"`, `"persistent_change"`, `"alarm_event"`) + botón `#load-demo` (`"Load demo data"` / `"Load scenario"`). Invoca `load_demo` (`core-panel.ts:344` -> `ui/src/core-api.ts:474`).
   - **4c. Mensaje de resultado (`#core-action-result`)** (`ui/index.html:67`, `core-panel.ts:348-354`).
   - **4d. Evaluaciones por indicador y síntomas (`.subsection-heading`, `#core-indicators`, `#core-symptoms`)** (`ui/index.html:68-70`, `ui/src/ui/core-panel.ts:28-138,225-235,254-255`):
     - Cada fila (`evaluationView`): título + zona, chip de estado (`evaluationCopy`), último valor (`evaluation.latest_value`), fecha (`evaluation.latest_measured_at`), `evaluation.summary`, fuente (`source_kind`, `source_ref`), aviso DEMO/heurística, escalado, botón **`Explain`** (invoca `explain_indicator` escuchando `slm-progress`, `core-panel.ts:78-105`) + caja `.explain-notice` (`facts` + `note.statements` + `note_status`).
     - Para `heart_rate`, `body_temperature`, `oxygen_in_blood`, `glucose` y `sleep` añade `<details>` `"View recorded history"` con gráfica SVG `sparkline` (`ui/src/ui/sparkline.ts:6-66`) y tabla `"Read sample data"` alimentadas con `list_records` (`core-panel.ts:276`).
5. **Panel de historial de telemetría local (`#telemetry-history` -> `.tele-hist-panel`)** (`ui/index.html:72`, `ui/src/main.ts:58-70`, `ui/src/ui/telemetry-history.ts:334-951`):
   - **5a. Cabecera**: `"Local telemetry"` / `"Traceable history"` + botón `"Refresh"` (llama `recommendations_cached_crew` y `list_telemetry`, `telemetry-history.ts:623,736,834-837`).
   - **5b. Selectores**: `"Astronaut"` (mezcla `recommendations_cached_crew` y crew del feed, `telemetry-history.ts:643-673`) e `"Indicator"` (12 indicadores: 10 de telemetría + `hrv` y `body_battery`, `telemetry-history.ts:33-46`).
   - **5c. Estado de carga (`statusLine`)** y **estado de persistencia (`persistLine`)** (informa resultado de `record_telemetry` del feed, `telemetry-history.ts:470-501`).
   - **5d. Trazabilidad (`.tele-hist-trace-box`)**: resultado de integridad + botón `"Verify chain"` (invoca `verify_telemetry`, `telemetry-history.ts:571`; además se ejecuta automáticamente con cooldown de 60 s al montar o cambiar astronauta, `telemetry-history.ts:603-615`).
   - **5e. Exportación (`.tele-hist-export`)**: checkbox `"Include telemetry"`, botón `"Export JSON"` (invoca `export_json`, `telemetry-history.ts:891`), ruta generada y botón `"Copy path"` (`telemetry-history.ts:407-426,908-912`).
   - **5f. Gráfica y lista (`.tele-hist-body`)**: gráfica SVG hasta 200 puntos de `list_telemetry` (sistólica/diastólica en presión; nota textual en `mood`, `telemetry-history.ts:196-311`) + lista `"Latest 10 readings"` (`telemetry-history.ts:313-331`).

---

### Página central / inicial (índice 1): `#overview` — `"Daily overview"`

Fuentes: `ui/index.html:22-49`, `ui/src/ui/dashboard.ts`, `ui/src/ui/vitals.ts`, `ui/src/ui/body-map.ts`, `ui/src/ui/voice.ts`, `ui/src/hybrid.css:93-420`, `docs/referencias/capturas/merken-movil.png`.

1. **Encabezado (`.overview-heading`)** (`ui/index.html:23`):
   - Eyebrow `"YOUR HEALTH, IN VIEW"`, `h2` `"Daily overview"`, intro `"A moment to check in."`.
   - En móvil (`< 600px`) `.eyebrow` y `.overview-intro` están ocultos (`ui/src/hybrid.css:335`). En la captura `merken-movil.png` tampoco aparece el `<h2>` `"Daily overview"`.
2. **Caja de resumen general (`#overall-box`)** (`ui/index.html:24-29`, `ui/src/ui/dashboard.ts:41-59`):
   - Primera línea (`.overall-line`):
     - `#overall-status`: chip de estado (`get_status` -> `overall.status` o `confirmedAlert.status` si `available`; si no, cálculo local `crewStatus(member.telemetry)` en `dashboard.ts:42-43`). En `merken-movil.png`: `"⚠ Outside range"`.
     - `#overview-demo`: etiqueta `"DEMO"`, visible si `status?.is_demo || feed?.preview` (`dashboard.ts:46`).
     - `#feed-status`: última actualización UTC (`get_status` -> `evaluated_at` si `available`, o estado del feed `vitals.ts:73-75`, `dashboard.ts:50`). En `merken-movil.png`: `"Live · 23:45:45 UTC"`.
   - `#overall-scope`: `"On-device · <display_name>"` (vía `get_profile`, `dashboard.ts:47-48`) o `"MySQL · Reading-level assessment"` / `"Synthetic crew · Reading-level assessment"` (`dashboard.ts:49`, visible así en `merken-movil.png`).
   - `#overall-extra` (oculto si vacío): indicadores no evaluados y `status.overall.worst` (`get_status`, `dashboard.ts:51-54`).
   - `#overview-alerts` (`"View persistent alerts (N)"`, oculto si `0`): navega a `#core-panel` (`dashboard.ts:55-56`, `page-navigation.ts:90`).
3. **Banner de aviso de feed (`#feed-notice`)** (`ui/index.html:30`, `ui/src/ui/vitals.ts:77-80`, `ui/src/main.ts:91`):
   - Muestra `#feed-message` (replay en caché, sin tripulación o sin conexión) y botón `#feed-retry` (`"Retry connection"` -> `feed.refresh()`).
4. **Escenario corporal y tarjetas vitales (`.body-stage`)** (`ui/index.html:32-42`, `ui/src/hybrid.css:108-179,342-370`):
   Rejilla de 3 columnas (4 filas x 2 laterales + mapa central de 4 filas):
   - **Columna izquierda** (`ui/index.html:33-36`, valores del miembro seleccionado en `vitals.ts:32-70`):
     1. `#vital-heart` (`"Pulse"`, `bpm`, estado local `heartRateStatus`).
     2. `#vital-pressure` (`"Pressure"`, `mmHg`, `systolic / diastolic`, estado local `bloodPressureStatus`).
     3. `#vital-oxygen` (`"SpO₂"`, `%`, estado local `oxygenStatus`).
     4. `#vital-mood` (`"Mood"`, texto traducido vía `moods` en `vitals.ts:8-23`, `"Self-reported · Not evaluated"` / compacto `"Not evaluated"` en `< 600px`, `vitals.ts:61-68`, `hybrid.css:356-360`).
   - **Centro (`.body-center`)** (`ui/index.html:37`, `ui/src/ui/body-map.ts:31-117`):
     - `#body-map`: SVG frontal (`body-reference.webp` + `body-reference-geometry.json`) con 12 zonas interactivas (`head`, `neck`, `chest`, `abdomen`, `right_arm`, `left_arm`, `right_hand`, `left_hand`, `right_leg`, `left_leg`, `right_foot`, `left_foot`) coloreadas según `status.body_zones` de `get_status` (`dashboard.ts:105`, `body-map.ts:103-114`). Al pulsar una zona resalta las tarjetas asociadas (`dashboard.ts:11-23,37-40`) y emite pulso en partículas (`main.ts:53-55`).
     - `#body-extra`: botones `"Back"` y `"Other"` (`body-map.ts:24,75,92`). Oculto en `< 600px` por `ui/src/hybrid.css:368` (pero visible en `merken-movil.png`).
     - `#zone-selection`: `"Select a zone"` o `"<Zona> · N linked readings"` (`dashboard.ts:22`).
     - `#zones-open`: botón `"All zones"`, abre `#zones-dialog` (`ui/index.html:154`) con las 14 zonas y su estado de `get_status` (`body-map.ts:65,95-113`).
   - **Columna derecha** (`ui/index.html:38-41`):
     1. `#vital-temperature` (`"Temp"` en HTML / `"Temperature"` en `merken-movil.png`, `°C`, `temperatureStatus`).
     2. `#vital-glucose` (`"Glucose"`, `mg/dL`, `glucoseStatus`).
     3. `#vital-sleep` (`"Sleep"`, `h`, `sleepStatus`).
     4. `#vital-nutrition` (`"Nutrition"`, `kcal`, `"Context only"`).
   - **Al pulsar cualquiera de las 10 tarjetas** (`dashboard.ts:60-93`): selecciona la zona asociada y abre el sheet `#reading-dialog` (`ui/index.html:155`), que muestra la lectura del feed y en `.detail-local` invoca `get_status`, `get_profile` y `list_records({ indicator, limit: 40 })` (`dashboard.ts:79`), pintando `evaluationView` (con botón `Explain` -> `explain_indicator` y `sparkline` abierta).
5. **Línea de fuente (`#body-source`)** (`ui/index.html:43`, `ui/src/ui/dashboard.ts:58`):
   - `"Cards: MySQL crew · Zones: on-device symptoms"` (+ `" · DEMO"` si aplica) o `"Local service unavailable · Body zones not evaluated"`.
6. **Fila de contexto y sincronización (`.context-grid`)** (`ui/index.html:44-45`):
   - `#vital-radiation` (`"Radiation"`, `µSv/h`, abre `#reading-dialog`, `dashboard.ts:93`).
   - `#vital-distance` (`"Distance"`, `km`, abre `#reading-dialog`, `dashboard.ts:93`).
   - `#sync-open` (`"Sync"` + `#overview-sync`: `"<pending> pending · <failed> failed"` de `get_status`, o `"Unavailable"`, `dashboard.ts:57`): navega a `#core-panel` (`page-navigation.ts:89`).
7. **Entrada de voz / escritura (`.voice-entry`)** (`ui/index.html:46-48`, `ui/src/hybrid.css:185-241,377-390`, `ui/src/ui/voice.ts:163-179`):
   - `#talk-hold`: píldora con icono de micrófono + `"Voice"` (en `merken-movil.png` era botón circular con texto `"Hold to talk"` abajo). Al hacer **clic** abre `#voice-dialog` en modo dictado (`dictation.show()`, `voice.ts:170-176`).
   - `#manual-open`: botón con icono lápiz + `"Type a reading"` (`"Type"` en `< 600px`, `hybrid.css:388-389`). Al hacer clic abre `#voice-dialog` en modo chat (`showChat()`, `voice.ts:177`).
   - `.voice-inline-status`: contenedor vacío (`ui/index.html:47`).

*(Nota sobre origen del feed en `#overview`)*: En builds Tauri de producción (`available && !import.meta.env.DEV`), `createTelemetryFeed` recibe `cacheOnly = true` (`ui/src/main.ts:90`), por lo que no consulta `/api/astronauts` sino `recommendations_cached_crew` (`ui/src/ui/telemetry.ts:505-511`) y se refresca con el evento `pulsys:backend-snapshot` (`telemetry.ts:535`). En `DEV` (sin `?preview=1`) consulta `/api/astronauts` cada 2 s y guarda lotes en SQLite mediante `record_telemetry` (`telemetry.ts:387,512,533`).

---

### Página derecha (índice 2): `#recommendations-page` — `"Recommendations"`

Fuentes: `ui/index.html:74-120`, `ui/src/ui/recommendations.ts`, `src-tauri/src/recommendations.rs`.

1. **Encabezado (`.recommendations-heading`)** (`ui/index.html:75-78`): `h2#recommendations-title` (`"Recommendations"`) + subtítulo `"Get an on-device suggestion based on saved measurements."`.
2. **Panel de configuración (`#recommendation-form`)** (`ui/index.html:80-99`, `ui/src/ui/recommendations.ts:65-166,254-259`):
   - Título `"Set up a recommendation"`.
   - Select `#recommendation-source` (`"Measurement source"`): `"On-device profile"` + tripulantes cacheados de `recommendations_cached_crew` (`recommendations.ts:102-113,255`; deshabilita los que no tienen `garminLinked`).
   - Select `#recommendation-preference` (`"Leisure preference"`): `""` (`"No preference"`), `"READING"`, `"MUSIC"`, `"PUZZLE"`, `"JOURNAL"`.
   - Nota `#recommendation-source-note` (`recommendations.ts:93-97`).
   - Botón submit `#recommendation-generate` (`"Generate recommendation"`): invoca `recommendations_generate` (`recommendations.ts:145`).
   - Botón `#recommendation-cancel` (`"Cancel"`): invoca `recommendations_cancel` (`recommendations.ts:161`).
   - Estado `#recommendation-status`.
3. **Panel de resultado (`.recommendation-result`)** (`ui/index.html:100-104`, `ui/src/ui/recommendations.ts:21-62,146-148`):
   - Título `"Suggested next step"`, texto vacío `#recommendation-empty` y salida `#recommendation-output` (proveedor `SLM_OFFLINE`/`CONTRACT_RULES`/`CORE_POLICY`, texto de recomendación, `caveat`, `Snapshot`, `Core action`, desplegables `"Measurements used"` y `"Source and references"`, y aviso de soporte experimental).
4. **Desplegable `"Ground-station snapshot"`** (`ui/index.html:106-115`, `ui/src/ui/recommendations.ts:168-248`):
   - Formulario `#recommendation-sync`: `#recommendation-username` (valor inicial `"demo"`), `#recommendation-password`, botón `#recommendation-connect` (`"Synchronize measurements"`), estado `#recommendation-sync-status`.
   - Al enviar invoca `recommendations_sync_backend` (`recommendations.ts:236`) y activa refresco cada 15 s con `recommendations_refresh_backend` (`recommendations.ts:195,213`), emitiendo `pulsys:backend-snapshot` (`recommendations.ts:175`).
5. **Desplegable `"Mission exercise guidance"`** (`ui/index.html:116-119`): texto estático informativo sobre equipamiento de ejercicio y autorización médica.

---

## 3) Elementos globales

1. **Fondo y partículas ambientales**: `.ambient-backdrop` y `<canvas id="ambient-particles">` (`ui/index.html:9-10`, `ui/src/ui/ambient-particles.ts`, `ui/src/main.ts:48,54,94`).
2. **Enlace de accesibilidad**: `<a class="skip-link" href="#main">Skip to observations</a>` (`ui/index.html:11`).
3. **Banner de preview de desarrollo (`#visual-preview`)**: visible solo si `DEV && ?preview=1` (`ui/index.html:13`, `ui/src/main.ts:32,35-38`). Muestra `"DEMO · Synthetic readings · Nothing is saved"` y enlace `"Exit preview"`.
4. **Cabecera global sticky (`.app-header`)** (`ui/index.html:14-18`, `ui/src/styles.css:138-145`, `ui/src/hybrid.css:29-51`):
   - **Wordmark `.wordmark`** (izquierda): texto `"PULSYS"` (`"MERKEN"` en `merken-movil.png`); al pulsar vuelve a `overview` (`ui/src/ui/page-navigation.ts:91-94`).
   - **Selector de tripulante (`#crew-open`)** (centro): píldora con `#crew-name` + chevron (icono y etiqueta `"Crew member"` ocultos en móvil `< 600px`, `hybrid.css:41-43,277`). Abre el modal `#crew-dialog` (`ui/index.html:130-142`, `ui/src/ui/crew.ts`):
     - Permite navegar con `<select id="crew-select">`, flechas `#crew-prev`/`#crew-next`, puntos `#crew-dots` o teclado (`ArrowLeft`/`ArrowRight`/`Home`/`End`, `crew.ts:79-89`).
     - Muestra previsualización del miembro y botón `#crew-confirm` (`"Select crew member"` / `"Selected · View readings"`).
     - En entorno Tauri (`available === true`, `main.ts:81`) muestra el botón `#crew-core` (`"Use in on-device core"`), que invoca `import_crew_member` (`main.ts:74-80` -> `src-tauri/src/lib.rs:663-673`) para copiar las lecturas guardadas en SQLite de ese tripulante al núcleo local **como datos DEMO** (`Store::import_synthetic`) y refresca `core-panel`.
   - **Botón de ayuda urgente (`#help-open`)** (derecha): `"Need help"` (icono de alarma oculto en `< 600px`, `hybrid.css:51,280`; fondo rosa `--mk-emergency` en `merken-movil.png` / `styles.css:71` vs fondo oscuro con borde en `hybrid.css:45-50`). Abre el modal `#help-dialog` (`ui/index.html:143-152`, `ui/src/ui/help.ts`):
     - Al abrir consulta `get_status` y `get_profile` (`help.ts:59`), muestra `#help-profile` y si `status.is_demo` muestra `#help-demo-badge` y `#help-demo-warning` (`help.ts:30-34`).
     - Permite escribir mensaje opcional (`#help-message`, máx 2000 caracteres).
     - Botón `#help-confirm` (`"Record help request"`): vuelve a consultar `get_status` (`help.ts:87-92`) por si apareció DEMO mientras estaba abierto; si procede, invoca `raise_event({ kind: "need_help", message })` (`help.ts:95`), actualiza `panel.acceptHelp` y llama `panel.refresh(true)` (`help.ts:97,126`).
5. **Indicadores globales de DEMO**:
   - `#visual-preview` en `?preview=1` (`main.ts:35-38`).
   - `#overview-demo` y sufijo `" · DEMO"` en `#body-source` en `overview` (`dashboard.ts:46,58`).
   - `#demo-badge`, `#demo-copy`, selector `#demo-empty` (`load_demo`), y etiquetas en alertas y evaluaciones en `core-panel` (`core-panel.ts:121,211,242-243,275,344`).
   - Avisos de borrado de datos DEMO al registrar datos reales en `#help-dialog` (`#help-demo-warning`, `help.ts:88-92`) y `#voice-dialog` (`#manual-demo`, `voice.ts:159,224-227`).

---

## 4) Flujo del asistente/SLM y del dictado por voz

El diálogo `#voice-dialog` (`ui/index.html:156-197`) contiene tres sub-vistas mutuamente excluyentes controladas desde `ui/src/ui/voice.ts`: `#voice-capture` (dictado), `#reading-chat` (chat rápido de pulso) y `#manual-form` (formulario + intérprete SLM `.assist-block`).

### A. Flujo de escritura en chat rápido (`Type a reading` -> `#reading-chat`)

- **Entrada**: pulsar `#manual-open` (`overview`) llama `showChat()` (`ui/src/ui/voice.ts:163-169,177`) y muestra `#reading-chat` (`ui/index.html:174-183`), o bien pulsar `"Open text chat"` (`#voice-type`) desde dictado (`voice.ts:178`).
- **Mensaje inicial**: `"Type a pulse reading. I’ll ask for any missing measurement time."` (`ui/src/ui/reading-chat.ts:65`).
- **Preguntas de seguimiento y de qué dependen**: **toda la lógica vive en TypeScript local sin SLM ni comandos Tauri** (`ui/src/ui/reading-chat.ts:34-57` y `ui/src/ui/reading-language.ts:1-31`):
  1. Al enviar texto en `#chat-compose`, llama `readPulse(text)` (`reading-language.ts:8-20`), una expresión regular en español/inglés que **solo reconoce pulso** (`pulse`, `heart rate`, `pulso`, `frecuencia cardiaca` + entero de 1–3 dígitos + unidad opcional + `at`/`a las <hora>`).
  2. Si detecta pulso e incluye sufijo temporal (`pulse.when`), intenta parsearlo con `readUtcTime(pulse.when, now)` (`reading-language.ts:22-31`: admite `"now"`, `"ahora"` o `[YYYY-MM-DD ]HH:MM[:SS][ UTC|Z]`).
     - Si la hora es válida -> genera burbuja con botón `"Review reading"` (`propose`, `reading-chat.ts:22-32`), que abre `#manual-form` pre-rellenado (`voice.ts:43,133-148`).
     - **Si falta la hora o no cumple formato UTC/`now`** -> guarda `pending = pulse.bpm` (`reading-chat.ts:47`) y pregunta: `"When was it measured? Enter a UTC time or type “now.” For an earlier date, include YYYY-MM-DD."` (`reading-chat.ts:48`).
  3. En el siguiente mensaje, si `readPulse(text)` es `null` pero `pending !== null`: intenta `readUtcTime(text, now)` (`reading-chat.ts:50-51`). Si falla, vuelve a preguntar: `"I still need the measurement time. Try “13:32 UTC” or “now.”"` (`reading-chat.ts:53`).
  4. Si `readPulse(text)` es `null` y `pending === null`: responde `"I can prepare a pulse reading. Try “Pulse 76 bpm” or use the form for another reading."` (`reading-chat.ts:55`).
- Para salir hacia el resto de indicadores o usar el intérprete SLM, el usuario debe pulsar `"Use reading form"` (`#chat-manual`, `voice.ts:179`).

### B. Flujo de dictado por voz (`#talk-hold` -> `#voice-capture`)

- **Entrada**: pulsar `#talk-hold` (`overview`) abre `#voice-dialog` con título `"Voice dictation"` y llama `dictation.show()` (`ui/src/ui/voice.ts:170-176`).
- **Selección de modo (`ui/src/ui/dictation.ts:20`)**:
  - **En Android (`mode === "live"`)**: pulsar `#voice-microphone` llama `startLive()` (`dictation.ts:81-110`), abriendo un `Channel<SpeechEvent>` e invocando `plugin:local-speech|start` (`crates/local-speech/src/lib.rs:10-36`, activo solo en `target_os = "android"`). Recibe transcripciones parciales en tiempo real (`event.state === "partial"`) en `#voice-transcript` y finaliza con `plugin:local-speech|finish` (`dictation.ts:50-52,62-71`). Si falla al iniciar, revela `#voice-recorded` (`"Use recorded dictation"`, `dictation.ts:108,145-150`) para cambiar a modo `"recorded"`.
  - **En escritorio o tras fallback (`mode === "recorded"`)**: pulsar `#voice-microphone` llama `startRecorded()` -> `recordDictation()` (`dictation.ts:112-127`, `ui/src/core-api.ts:666-912`), que captura audio con `AudioWorkletNode` (o `ScriptProcessorNode`), mezcla a mono, remuestrea a 16 kHz Float32LE (tope 30 s, `core-api.ts:510-512,576-622`) y al detener llama `dictate()` -> comando Tauri `transcribe_audio` (`core-api.ts:924-937`, `src-tauri/src/voice.rs:554-607`, que busca modelos locales `stt-whisper-base`, `stt-whisper-small` o `stt-canary-180m` en `voice.rs:298-316`).
- **Paso al intérprete**: el usuario edita si quiere `#voice-transcript` y pulsa `"Review transcript"` (`#voice-review`, `dictation.ts:152-155`), que llama `manual(undefined, text)` (`voice.ts:125,133-162`).

### C. Flujo del asistente/SLM de interpretación (`.assist-block` en `#manual-form`)

- **Activación**:
  - Automática al venir de dictado: `voice.ts:154` llama `assistant.interpretInto(transcript, "voice")`.
  - Manual desde el formulario: escribir en `#assist-text` y pulsar `"Interpret"` (`#assist-interpret`, `ui/src/ui/assistant.ts:237-239`, con `source: "text"`).
- **Ejecución y progreso**: `runInterpret` (`assistant.ts:172-221`) escucha el evento Tauri `slm-progress` (`assistant.ts:42-57`, `src-tauri/src/lib.rs:708-711`) mostrando fases (`"loading"`, `"generating"`, `"validating"`) e invoca **`interpret_text`** (`src-tauri/src/lib.rs:143-155`). Botón `#assist-cancel` invoca **`slm_cancel`** (`assistant.ts:241-246`, `src-tauri/src/lib.rs:211-218`).
- **Lógica en Rust (2 capas en `crates/slm/src/service.rs:434-481,642-700`)**:
  1. **Capa 1 (siempre)**: `SlmService::interpret` (`service.rs:446`) ejecuta el intérprete determinista por reglas `pulsys_core::interpret::interpret(&text)` (`crates/core/src/interpret/mod.rs:38-52`).
  2. **Detección de residuo**: `unresolved_fragments(&text, &proposal)` (`crates/slm/src/service_residual.rs:415-538`) calcula qué palabras con contenido no quedaron cubiertas por Capa 1; `needed = !unresolved.is_empty() || proposal.uninterpreted.iter().any(|u| has_content(u))` (`service.rs:447-449`).
  3. **Capa 2 (SLM, condicional)**: en el worker (`service.rs:659-698`), el modelo (`space-health-base-Q4_K_M.gguf` con gramática `PROPOSAL_GBNF`, `src-tauri/src/lib.rs:717`, `crates/slm/src/service.rs:53-66`, `crates/slm/src/interpreter.rs:36-104`) **solo se ejecuta si `needed == true` y el motor `slm-llama` está disponible**. Su salida se sanea (`sanitize_proposal`, `crates/slm/src/interpreter.rs:296-428`), valida con `Proposal::check` (`crates/core/src/proposal.rs:464-580`) y filtra con `filter_layer2` (`crates/slm/src/service.rs:584-609`: no puede duplicar indicadores ya resueltos en Capa 1 ni inventar citas fuera de `unresolved`/`uninterpreted`).
- **¿Hacía preguntas de seguimiento (p. ej. pedir la hora)?**:
  - **No hay diálogo ni preguntas multi-turno en el SLM.** Es una única petición `interpret_text` -> `InterpretResponse`.
  - Respecto a **la hora del dato**: ni Capa 1 (`crates/core/src/interpret/time.rs:1-493`) ni Capa 2 (`crates/slm/src/interpreter.rs:89,108`, `crates/core/src/proposal.rs:49-57`) calculan ni inventan fechas/horas. Si el texto no menciona momento devuelven `time: { kind: "unspecified" }` (el formulario pre-rellena `#manual-time` con `now` UTC en `voice.ts:80,149`); si menciona una referencia temporal (p. ej. `"a las 8"`, `"this morning"`), devuelven `time: { kind: "ambiguous", quote: "..." }`.
  - Cuando `record.time.kind === "ambiguous"`, la tarjeta de propuesta muestra el aviso estático: ``Time mentioned: “${record.time.quote}” · set the time below`` (`ui/src/ui/assistant.ts:104-112`).
  - Al pulsar `"Use this"` (`assistant.ts:114`), `applyProposal` (`voice.ts:75-109`) rellena los campos entendidos y **mueve el foco al primer campo obligatorio vacío** (`input[required], select[required]`, `voice.ts:104-108`) —por ejemplo `window_start`/`window_end` en sueño (`crates/core/src/proposal.rs:260-267`, `crates/slm/src/interpreter.rs:392-400`) o valores nulos— sin guardar hasta que el usuario pulsa `"Save reading"` (`log_record`, `voice.ts:229`).

---

## 5) Qué se veía roto o a medias según el código

1. **Placeholder sin conectar en detalle de lectura (`#reading-dialog`)**:
   - `ui/index.html:155`: `<div class="assistant-slot"><span>Explain (assistant not loaded)</span></div>` queda fijo en el diálogo de detalle de tarjeta sin ningún script que lo cargue u oculte, coexistiendo con el botón `"Explain"` funcional dentro de `.detail-local` (`ui/src/ui/dashboard.ts:87-91` -> `ui/src/ui/core-panel.ts:41-49`).
2. **Explicador IA inutilizado por catálogo de fichas vacío (`[]`)**:
   - `crates/slm/fichas/fichas.json:1` contiene `[]`. Por ello `offered_statements(&fichas, eval.status)` siempre devuelve vacío (`crates/slm/src/service.rs:712-713`), el motor v5 nunca se carga y `explain_indicator` siempre responde `note_status: "no_fichas"`. En `ui/src/ui/core-panel.ts:15-26`, `noteUnavailableMessage("no_fichas")` devuelve `null`, ocultando silenciosamente que no hubo nota IA.
3. **Comandos Tauri implementados pero sin UI / rutas muertas en frontend**:
   - **Síntesis de voz (TTS)**: `speak_text`, `speak_cancel`, `voice_release` existen en `src-tauri/src/lib.rs:724-726`, `src-tauri/src/voice.rs:136-289` y `ui/src/core-api.ts:456-458`, pero ningún componente UI los llama ni hay control de audio TTS.
   - **Features de voz desactivadas por defecto**: en `src-tauri/Cargo.toml:36-39`, `default = ["slm-llama"]` excluye `voice-tts` y `voice-stt`, por lo que en compilaciones por defecto `transcribe_audio`, `stt_release`, `speak_text`, `speak_cancel` y `voice_release` devuelven `"voice_unavailable"` (`src-tauri/src/voice.rs:146,254,274,570,614`).
   - **Corrección de registros (`correct_record`)**, **borrado manual de DEMO (`clear_demo`)**, **importación de API de simulación (`sim_list_astronauts`, `sim_import`)**, **estado/descarga explícita de modelos (`slm_status`, `slm_release`, `stt_release`)** y **`core_status`**: todos declarados en `src-tauri/src/lib.rs:491-657,728,731-733` y (salvo `core_status`) en `ui/src/core-api.ts:461-482,505`, pero sin ningún botón ni flujo en `ui/src/ui/*.ts`.
4. **Elementos HTML/CSS huérfanos de iteraciones previas**:
   - `.voice-inline-status` (`ui/index.html:47`, `ui/src/hybrid.css:226,386`): div vacío nunca escrito por JS.
   - `.voice-wave` y `#voice-held` (`ui/src/styles.css:206-211`): estilos sin elemento en HTML/TS; `#talk-hold` conserva ID y callback `ambient.setHolding` (`ui/src/main.ts:94`), pero ya no responde a "mantener pulsado" sino a clic simple (`ui/src/ui/voice.ts:170-176`) reutilizando `onListening` cuando `phase === "listening"` (`ui/src/ui/dictation.ts:35`).
5. **Desajuste entre CSS móvil, captura y código**:
   - `ui/src/hybrid.css:368` oculta `.body-extra` (`"Back"`, `"Other"`) en `< 600px`, mientras que en `docs/referencias/capturas/merken-movil.png` sí son visibles bajo la silueta.
   - La captura muestra `MERKEN`, `Need help` rosa, sin `<h2>Daily overview</h2>`, `Temperature`, `Hold to talk` y 2 puntos (`Overview`); el código actual tiene `PULSYS`, `Need help` oscuro, `Daily overview`, `Temp`, `Voice` + `Type` y 3 puntos.
6. **Chat rápido (`#reading-chat`) limitado a un solo indicador y desconectado del núcleo**:
   - `ui/src/ui/reading-chat.ts` solo interpreta pulso mediante regex locales (`ui/src/ui/reading-language.ts`), dejando los otros 10 indicadores y el intérprete SLM aislados detrás del enlace `"Use reading form"`.
7. **Duplicación de tipos y desajuste leve de tipado**:
   - `TelemetryPersistStateKind` / `TelemetryPersistStatus` en `ui/src/ui/telemetry-history.ts:69-79` duplican `TelemetryPersistStatusKind` / `TelemetryPersistState` en `ui/src/ui/telemetry.ts:11-22`.
   - `mysqlStatuses` en `ui/src/ui/status-copy.ts:29-53` declara retorno `Record<VitalKey, StatusCopy>` (6 claves), aunque en líneas 39-51 escribe también sobre `"nutrition"`, `"radiation"` y `"distance"`.
8. **Errores silenciados**:
   - `ui/src/ui/telemetry-history.ts:637-640`: `catch {}` vacío en `core.cachedCrew()`.
   - `ui/src/ui/telemetry-history.ts:910`: `.catch(() => {})` al copiar ruta al portapapeles.
   - `ui/src/ui/assistant.ts:227,245`: `.catch(() => {})` en `core.slmCancel(...)`.
   - `ui/src/ui/dictation.ts:58`: `.catch(() => {})` en `stopLive(session, true)`.
   - `ui/src/ui/telemetry.ts:518-521`: `catch` sin parámetro en `refresh()` oculta el motivo de fallo de red/parseo.
   - `ui/src/core-api.ts:697,702,706,710,724,820,875`: varios `catch { /* ignore */ }` y `.catch(() => {})` al cerrar/desconectar Web Audio.
   - `ui/src/ui/recommendations.ts:257-259`: `.catch(() => ...)` descarta el error real de `recommendations_cached_crew`.
   - `src-tauri/src/lib.rs:710`: `let _ = handle.emit("slm-progress", event);`.
   - `crates/slm/src/service.rs:234,351,393`: descarte de `self.tx.send`, `catch_unwind` del worker y `previous.handle.join()`.
   - `src-tauri/src/recommendations.rs:439`: `let _ = V7_VERIFIED.set(path.clone());`.
9. **`TODO` / `FIXME`**:
   - No se hallaron `TODO`/`FIXME` en ninguno de los archivos leídos (`ui/`, `src-tauri/src/`, `crates/slm/src/`, `crates/local-speech/src/`). Existencia en otros archivos no leídos: **NO VERIFICADO**.
