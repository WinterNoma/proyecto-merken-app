# Merken

App móvil de monitoreo de salud para astronautas. Es nuestro proyecto para el reto de NASA Space Apps 2026 [Health Monitoring Software](https://www.spaceappschallenge.org/2026/challenges/health-monitoring-software/).

**Alcance de entrega: exclusivamente app móvil.** El navegador y la ejecución de escritorio se usan para desarrollo y revisión de la interfaz; no se está preparando un producto web o de escritorio.

Funciona **sin conexión** y sincroniza con Tierra cuando hay red. La lógica de salud vive en **Rust** y la interfaz es **Tauri 2** (TypeScript + Vite).

## Estructura

```
crates/
  core/    dominio y evaluación (puro, sin E/S, sin pánicos)
  store/   SQLite local + bandeja de salida
  sync/    cliente de la API de sincronización
  slm/     modelos pequeños en el dispositivo (interpretar / explicar)
  seed/    escenarios sintéticos
src-tauri/ app Tauri (solo une los crates con la interfaz)
ui/        interfaz web (TS + Vite)
docs/      requisitos, decisiones e investigación
reviews/   revisiones de Sol
```

Empieza por [docs/requisitos-app-movil.md](docs/requisitos-app-movil.md) y por las [decisiones](docs/decisiones/).

## Preparar el entorno (Windows)

1. Rust estable (`rustup`), Node 20+ y Visual Studio Build Tools (C++).
2. Android Studio → SDK Manager → SDK Tools: **NDK (Side by side)** y **Android SDK Command-line Tools**.
3. Variables de entorno de usuario:
   - `JAVA_HOME` = `C:\Program Files\Android\Android Studio\jbr`
   - `ANDROID_HOME` = `%LOCALAPPDATA%\Android\Sdk`
   - `NDK_HOME` = `%ANDROID_HOME%\ndk\<versión>`
4. Targets de Rust para Android:
   ```bash
   rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
   ```
5. Dependencias de JS: `npm install`

## Comandos

```bash
npm run tauri dev            # escritorio
npm run tauri android init   # una vez, genera src-tauri/gen/android
npm run tauri android dev    # emulador o móvil conectado
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Reglas del proyecto

- **Las decisiones de salud las toma `merken-core`.** Ni TypeScript, ni el SLM, ni el servidor.
- En `crates/*` están prohibidos `unwrap`, `expect`, `panic!` y el indexado sin comprobar (lo hace cumplir clippy).
- **Nunca** subir credenciales: van en `.env` (ignorado). Los modelos `.gguf` tampoco se suben (van en `externo/`).
