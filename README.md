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

## Preparar el entorno (Mac)

1. Android Studio → Settings → Languages & Frameworks → Android SDK → pestaña **SDK Tools**: marca **NDK (Side by side)**, **Android SDK Command-line Tools** y **Platform-Tools**. Apply.
2. Pega en la Terminal estas dos líneas, una detrás de otra: la primera instala Rust y los targets de Android; la segunda deja las variables fijas en `~/.zshrc`.
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source "$HOME/.cargo/env" && rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
   ```
   ```bash
   echo 'export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"' >> ~/.zshrc && echo 'export ANDROID_HOME="$HOME/Library/Android/sdk"' >> ~/.zshrc && echo 'export NDK_HOME="$ANDROID_HOME/ndk/$(ls -1 $ANDROID_HOME/ndk | tail -1)"' >> ~/.zshrc && echo 'export PATH="$HOME/.cargo/bin:$ANDROID_HOME/platform-tools:$PATH"' >> ~/.zshrc
   ```
3. Cierra la Terminal, abre otra y, en la carpeta del proyecto (Node 20+; si falta, `brew install node`):
   ```bash
   npm install && npm run tauri android build -- --debug --open
   ```
   Se abre Android Studio: elige un emulador (**arm64** en Macs M1/M2/…) y dale a **Run ▶**. No cierres la Terminal mientras uses Android Studio.

Sin instalar nada: GitHub Actions → **Android APK** genera un APK debug arm64 en cada push a `desarrollo`. Descarga el artifact y arrástralo al emulador.

**Emulador en un PC con Windows o en un Mac Intel:** usa la build **x86_64**: Actions → **Android APK** → **Run workflow** → abi `x86_64`. La arm64 se cierra al abrirse en esos emuladores, y sirve para móviles y para los Mac con chip M.

## Datos de los astronautas en la APK

La APK **no se conecta a MySQL directamente**, y es a propósito: tendría que llevar dentro la contraseña de MySQL, que cualquiera puede sacar de un APK. Además, MySQL tendría que quedar abierto a todo internet.

- **APK:** en **Recommendations → Ground-station snapshot**, inicia sesión con el usuario del backend (`deployconmerken.gabogrobier.dev`). Es el backend quien lee MySQL. A partir de ahí la app se actualiza sola cada 15 s mientras está abierta. La sesión vive solo en memoria, y al cerrar la app hay que volver a sincronizar. La contraseña no se guarda nunca.
- **Modo dev** (`npm run dev` o `npm run tauri android dev`): Vite lee MySQL con las credenciales de `server/.env` (copia `server/.env.example`) y refresca cada 2 s.
- Si algún día hace falta otra fuente, la vía correcta es una API propia con login delante de MySQL, nunca las credenciales dentro de la app.

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
