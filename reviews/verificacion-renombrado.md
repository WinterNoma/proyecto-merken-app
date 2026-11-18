# Verificación del renombrado de Merken a Pulsys

Fecha: 2026-11-18  
Rama inspeccionada: `desarrollo` (cambios sin commit en el árbol de trabajo)

---

## 1. Resultado de TypeScript (`tsc`) y pruebas de UI

> Nota de entorno en PowerShell: la invocación directa `npx` / `npm` falló por política de ejecución de scripts (`npx.ps1` / `npm.ps1` bloqueados por `PSSecurityException`), por lo que ambos comandos se ejecutaron con `npx.cmd` y `npm.cmd` (y se contrastó directamente con `node --test`).

- **`npx.cmd tsc --noEmit`**: **APROBADO** (código de salida `0`, 0 errores).
- **`npm.cmd run test:ui`** (`node --test ui/tests/*.test.mjs`): **APROBADO** — **3 suites, 43 pruebas ejecutadas, 43 aprobadas, 0 falladas, 0 canceladas, 0 saltadas** (duración `380.0324 ms`; ejecución directa con `node --test ui/tests/*.test.mjs`: 43/43 aprobadas en `470.2494 ms`).

---

## 2. Hallazgos de verificación (`archivo:línea`)

### A. Hallazgos críticos / probables roturas funcionales

1. **Posible rotura de URL de producción/estación de tierra por reemplazo global**
   - `src-tauri/src/recommendations.rs:81` — `const ORIGIN: &str = "https://deployconpulsys.gabogrobier.dev";` (el diff muestra que antes era `"https://deployconmerken.gabogrobier.dev"`).
   - `README.md:64` — documenta el mismo dominio `deployconpulsys.gabogrobier.dev`.
   - **Impacto:** si el servidor y el certificado TLS siguen publicados en `deployconmerken.gabogrobier.dev` y no se aprovisionó `deployconpulsys.gabogrobier.dev`, el inicio de sesión y la descarga del snapshot de estación de tierra fallarán en runtime.

2. **Proyecto Android generado localmente (`src-tauri/gen/android/`) desactualizado y con ID antiguo (archivo sin seguimiento)**
   - `src-tauri/gen/android/app/build.gradle.kts:19` — `namespace = "com.merken.app"`.
   - `src-tauri/gen/android/app/build.gradle.kts:22` — `applicationId = "com.merken.app"`.
   - `src-tauri/gen/android/app/src/main/java/com/merken/app/MainActivity.kt:1` — `package com.merken.app` (junto con `src-tauri/gen/android/app/src/main/java/com/merken/app/generated/*.kt`).
   - `src-tauri/gen/android/app/tauri.build.gradle.kts:1-8` — no incluye `implementation(project(":tauri-plugin-slm-assets"))`, mientras que `src-tauri/gen/android/app/src/main/res/values/strings.xml:2-3` sí tiene `"Pulsys"`.
   - **Impacto:** aunque `.github/workflows/android-apk.yml:77` regenera el proyecto con `npm run tauri -- android init --ci --skip-targets-install`, cualquier compilación local (`npx tauri android build`) sin regenerar o limpiar `src-tauri/gen/android/` compilará con `com.merken.app` y sin enlazar `slm-assets`.

---

### B. Inconsistencias documentales y artefactos residuales sin seguimiento

3. **Nota histórica sobrescrita por reemplazo de texto en documentación del backend**
   - `docs/api/modelo-datos-backend.md:14` — dice `health_recommendations (antes pulsys_suggestions), recommendation_reviews (antes pulsys_suggestion_reviews)`; antes del cambio decía `antes merken_suggestions` / `antes merken_suggestion_reviews`. Cambia el historial de cómo se llamaban esas tablas antes de v0.3.1.

4. **Registros históricos de validación visual alterados tras la ejecución**
   - `docs/front/etapa-1/adaptacion-movil/validacion.json:174,476,778,913` — reemplazó `"Merken home"` por `"Pulsys home"` y `"Open the Merken app..."` por `"Open the Pulsys app..."` dentro de un JSON de resultados capturados previamente.
   - `docs/front/etapa-2/validacion.json:509`, `docs/front/etapa-3/validacion.json:550`, `docs/front/etapa-4/validacion.json:515` — reemplazó `frameDirectory` (`.../Temp/merken-...` → `.../Temp/pulsys-...`) en salidas históricas ya ejecutadas.

5. **Archivos sin seguimiento (untracked) que aún mencionan `merken` / `com.merken.app`**
   - `docs/plan-flutter-migracion.md:108` — menciona `merken-slm/native`.
   - `docs/plan-flutter-migracion.md:117` — menciona `crates merken-* se aplazan`.
   - `docs/plan-flutter-migracion.md:125` — menciona `Conservar com.merken.app en Android si debe actualizar las instalaciones actuales` (refleja decisiones anteriores al renombrado recién aplicado).
   - `.claude/launch.json:5` — `"name": "merken-ui"`.
   - `entrega-diseno-merken.zip` (en la raíz del repo, ignorado por `*.zip`) — archivo comprimido anterior al renombrado de `entrega-diseno-pulsys/`.
   - `server/.env:6-8` (ignorado por git) — `MYSQL_DATABASE=merken_db`, `MYSQL_USER=merken_user` (configuración de la BD MySQL externa; no debe cambiarse salvo que se renombre la BD en el servidor).
   - `tools/__pycache__/sync_telemetry_dynamo.cpython-314.pyc` — bytecode cacheado anterior al cambio (contiene `"merken-tech-test-"` / `"merken-tech-test"`).

---

### C. Áreas revisadas que resultaron consistentes (sin referencias rotas)

- **Configuración principal y frontend (`src-tauri/tauri.conf.json`, `vite.config.ts`, `package.json`, `package-lock.json`):**
  - `src-tauri/tauri.conf.json:3,5,16` usa `productName: "Pulsys"`, `identifier: "com.pulsys.app"` y `title: "Pulsys"`.
  - `vite.config.ts:1-101` sin referencias antiguas ni rutas rotas.
  - `package.json:2` y `package-lock.json:2,8` usan `"pulsys-app"`.
- **Plugins Kotlin y registro en Rust:**
  - `crates/local-speech/android/build.gradle.kts:9`, `crates/local-speech/android/src/main/java/com/pulsys/speech/LocalSpeechPlugin.kt:1` y `crates/local-speech/src/lib.rs:65` coinciden en `com.pulsys.speech` y `LocalSpeechPlugin`.
  - `plugins/slm-assets/android/build.gradle.kts:6`, `plugins/slm-assets/android/src/main/java/SlmAssetsPlugin.kt:1` y `plugins/slm-assets/src/lib.rs:14` coinciden en `com.pulsys.slmassets` y `SlmAssetsPlugin`. *(Nota menor: `SlmAssetsPlugin.kt` está directamente bajo `java/` y no en `java/com/pulsys/slmassets/`, igual que antes del renombrado; Kotlin lo admite si el `package` coincide).*
- **Capabilities y permissions de Tauri:**
  - `src-tauri/capabilities/default.json:9` usa `"local-speech:default"`.
  - `crates/local-speech/permissions/default.toml:1-3`, `crates/local-speech/permissions/autogenerated/reference.md:22,35,48,61`, `crates/local-speech/permissions/autogenerated/commands/start.toml`, `finish.toml` y `crates/local-speech/permissions/schemas/schema.json` usan `local-speech:*` sin identificadores antiguos.
  - `plugins/slm-assets/permissions/autogenerated/` está vacío (`plugins/slm-assets/build.rs:2` declara `Builder::new(&[])`).
- **Léxico (`pulsys-us-en.txt`) y `.github/scripts/bundle_offline_models.py`:**
  - `crates/voice/lexicon/pulsys-us-en.txt` existe y coincide con `crates/voice/src/lib.rs:197,589`, `crates/voice/src/normalize.rs:374`, `crates/voice/README.md:11,14` y `docs/api/comandos-core.md:187`.
  - `.github/scripts/bundle_offline_models.py:11-20,42` solo empaqueta GGUF y STT Whisper (`base-*`), sin referencias al léxico ni a `merken`; su carpeta temporal es `pulsys-offline-apk`.
- **Enlaces relativos en Markdown:** comprobados todos los `.md` fuera de `reviews/` y `docs/referencias/` (0 enlaces relativos rotos a `entrega-diseno-merken`, `merken-wordmark*`, `merken-us-en.txt`, etc.).
- **Excepciones intencionales preservadas (no marcadas como error):** `"merken-export"` (`crates/store/src/export.rs:361,543`, `src-tauri/src/lib.rs:1063,1080`, `docs/api/export-json.md:15,19`), `"merken-telemetry-v1"` (`crates/store/src/telemetry.rs:349`, `crates/store/src/export.rs:500`, `tools/sync_telemetry_dynamo.py:133`, `docs/api/export-json.md:44,61`, `docs/api/telemetria-local.md:123`), `WinterNoma/proyecto-merken-app` (`Cargo.toml:9`, `docs/android-apk.md:12`, `docs/requisitos-app-movil.md:3,138`, `.idea/workspace.xml:27`) y `"Equipo Merken"` (`src-tauri/Cargo.toml:4`).

---

## 3. Riesgos detectados (dependencias de `com.merken.app` y `merken.sqlite3`)

1. **Pérdida de continuidad de datos locales y actualizaciones en sitio (`com.merken.app` → `com.pulsys.app` y `merken.sqlite3` → `pulsys.sqlite3`)**
   - `src-tauri/tauri.conf.json:5` (`com.pulsys.app`) y `src-tauri/src/lib.rs:678` (`dir.join("pulsys.sqlite3")`) no implementan migración ni lectura de respaldo desde `com.merken.app/merken.sqlite3`.
   - **En Android:** al cambiar `applicationId` a `com.pulsys.app`, Android la instala como una app nueva en paralelo, sin actualizar instalaciones existentes de `com.merken.app` ni heredar su SQLite ni sus modelos en `filesDir`.
   - **En escritorio (Windows):** `%APPDATA%\com.pulsys.app\pulsys.sqlite3` arranca vacío si previamente existía `%APPDATA%\com.merken.app\merken.sqlite3`.

2. **Riesgos en `tools/sync_telemetry_dynamo.py` por cambio de ruta, partición e IDs deterministas**
   - `tools/sync_telemetry_dynamo.py:129`: `default_db()` ahora busca `%APPDATA%\com.pulsys.app\pulsys.sqlite3`; sobre bases previas exige pasar `--db <ruta-a-merken.sqlite3>`.
   - `tools/sync_telemetry_dynamo.py:163,169,171,360,412`: se cambiaron el seed de `uuid5` (`"pulsys-telemetry:..."` antes `"merken-telemetry:..."`), `external_id` (`"garmin:pulsys-telemetry-..."`), `created_by.id` (`"pulsys-tech-test"`) y `--prefix` por defecto (`"pulsys-tech-test-"` antes `"merken-tech-test-"`).
   - **Consecuencias:**
     - Si ya se habían subido lecturas desde una base anterior y se re-ejecuta el script, generará **nuevos `record_id` y nuevas particiones** en DynamoDB (rompiendo la idempotencia entre ejecuciones previas y dejando obsoletas las claves `merken-tech-test-*` en `backups/dynamo-telemetry-state.json`).
     - Ejecutar `--purge` con el `--prefix` por defecto no limpiará particiones anteriores (`merken-tech-test-*`); hay que indicar `--prefix merken-tech-test-`.
   - *(La verificación de cadena en `tools/sync_telemetry_dynamo.py:133` sí mantiene `b"merken-telemetry-v1"`, por lo que sigue siendo compatible con filas antiguas).*

3. **Endpoint remoto potencialmente roto (`src-tauri/src/recommendations.rs:81`)**
   - Si el backend de Tierra no responde bajo `https://deployconpulsys.gabogrobier.dev` (y sigue solo en `deployconmerken.gabogrobier.dev`), la sincronización manual en APK fallará en tiempo de ejecución.

4. **Archivo de exportación (`docs/api/export-json.md:9` vs `src-tauri/src/lib.rs:396`)**
   - El nombre de archivo generado cambió a `pulsys-export-AAAAMMDDTHHMMSSZ.json`, mientras que el campo interno sigue siendo `"format": "merken-export"`. Ambos lados (código y doc) coinciden entre sí, pero cualquier script externo que busque `merken-export-*.json` por patrón de nombre dejará de encontrar los nuevos archivos.

5. **Desfase con el plan de migración (`docs/plan-flutter-migracion.md:108,117,125`)**
   - Ese documento (sin seguimiento en git) asumía explícitamente aplazar el renombrado de crates `merken-*` y evaluar conservar `com.merken.app` para actualizar instalaciones existentes.

---

## 4. Qué NO se pudo verificar

- **Compilación y pruebas de Rust / Clippy:** no se ejecutaron `cargo test`, `cargo check` ni `cargo clippy` (ni `--locked` como en `.github/workflows/android-apk.yml:75`) para no disparar `tauri-build`/`tauri-plugin` sobre el árbol de trabajo ni depender de compilación nativa C++/CMake/LLVM (`llama.cpp`) o `sherpa-onnx` (`tts`/`stt`). Por tanto, la consistencia de `Cargo.lock` y la compilación efectiva de los crates y de `src-tauri` quedan sin verificar en ejecución.
- **Compilación Android y workflow de CI:** no se ejecutó `tauri android init` / `tauri android build`, Gradle/Kotlin, `.github/workflows/android-apk.yml` ni `.github/scripts/bundle_offline_models.py` (requiere Android SDK/NDK, modelos y entorno Linux CI), ni se probó la carga de los plugins `com.pulsys.speech.LocalSpeechPlugin` y `com.pulsys.slmassets.SlmAssetsPlugin` en dispositivo/emulador.
- **Conectividad del servidor remoto:** no se comprobó por red si `https://deployconpulsys.gabogrobier.dev` tiene DNS y certificado TLS activos frente a `https://deployconmerken.gabogrobier.dev`.
- **Ejecución en vivo de `tools/sync_telemetry_dynamo.py` y app empaquetada:** no se ejecutó el script contra un `pulsys.sqlite3` / `merken.sqlite3` ni DynamoDB, ni se abrió la app compilada para comprobar creación de `pulsys.sqlite3`, exportación `pulsys-export-*.json` ni carga de `pulsys-us-en.txt` con Kokoro.
