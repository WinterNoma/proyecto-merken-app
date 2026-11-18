# Compilar la app Android por CLI (sin abrir Android Studio)

Proyecto: Tauri 2 + Rust (`com.pulsys.app`). Android Studio solo hace falta una vez, si quieres instalar `cmdline-tools` desde su SDK Manager.

## Requisitos previos

- Rust con los targets de Android:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

- Node.js y `npm install` ya hecho en la raíz del repo.
- Android SDK (por defecto en `%LOCALAPPDATA%\Android\Sdk`) con `platform-tools` y `cmdline-tools`.
- JDK (sirve el que trae Android Studio: `C:\Program Files\Android\Android Studio\jbr`).

## 1. Instalar el NDK por CLI

Ajusta la ruta de `sdkmanager` si es distinta:

```powershell
& "$env:LOCALAPPDATA\Android\Sdk\cmdline-tools\latest\bin\sdkmanager.bat" "ndk;27.2.12479018"
```

## 2. Variables de entorno

Se ponen una sola vez. Después hay que reabrir la terminal.

```powershell
[Environment]::SetEnvironmentVariable("ANDROID_HOME","$env:LOCALAPPDATA\Android\Sdk","User")
[Environment]::SetEnvironmentVariable("NDK_HOME","$env:LOCALAPPDATA\Android\Sdk\ndk\27.2.12479018","User")
[Environment]::SetEnvironmentVariable("JAVA_HOME","C:\Program Files\Android\Android Studio\jbr","User")
```

## 3. Inicializar el proyecto Android (una vez)

Desde la raíz del repo:

```bash
npx tauri android init
```

Esto crea `src-tauri/gen/android`.

## 4. Compilar el APK de depuración

Un solo target para que sea más rápido (`aarch64` para móvil real, `x86_64` para emulador):

```bash
npx tauri android build --debug --apk --target aarch64
```

El APK queda en:

```
src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

## 5. Instalar y ejecutar

Con el móvil (depuración USB activada) o el emulador conectado:

```bash
npx tauri android dev
```

Compila, instala y arranca con hot reload. Para instalar el APK a mano:

```bash
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
```

## Consejos para que tarde menos

- Compila un solo target (`--target aarch64`), no los cuatro.
- Usa `--debug`. El modo release es mucho más lento.
- La primera compilación es lenta (descarga Gradle y compila todo Rust). Las siguientes son incrementales.
- Añade a `%USERPROFILE%\.gradle\gradle.properties`:

```properties
org.gradle.daemon=true
org.gradle.parallel=true
org.gradle.caching=true
```

## Problemas típicos

- **`ANDROID_HOME` / `NDK_HOME` not set**: faltan las variables del paso 2 o no se reabrió la terminal.
- **`JAVA_HOME` incorrecto**: apunta a una carpeta con `bin\java.exe`.
- **No detecta el dispositivo**: comprueba con `adb devices` que aparece como `device`.
