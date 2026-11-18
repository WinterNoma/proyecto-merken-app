# Etapa 2 de Pulsys · Puente Rust y compilación móvil

## 1. Qué hice

- Añadí `pulsys-bridge` con FRB 2.11.1 exacta y una sesión opaca. Un hilo abre y conserva el `App`, atiende una cola de 16 operaciones y drena las solicitudes aceptadas al cerrar.
- El DTO durable V1 conserva raíz de corrección, destino, avisos y el estado exacto de aprobación. La prueba Rust restaura una corrección desde disco antes de su primer guardado y después del commit, y comprueba el mismo recibo.
- `RustAdapter` implementa `PulsysApi`, traduce DTO y errores, y guarda la acción autorizada mediante un reemplazo atómico en el directorio privado. Al abrir la sesión reintenta la acción pendiente.
- Integré la compilación nativa en Gradle y Xcode. Añadí la prueba de integración offline y la CI con Rust, APK debug, simulador iOS con captura y compilación iOS sin firma.

## 2. Archivos tocados

- `Cargo.toml` y `crates/bridge/**`.
- `app/flutter_rust_bridge.yaml`, `app/pubspec.yaml` y `app/rust_builder/**`.
- `app/lib/main.dart`, `app/lib/platform/rust_adapter.dart` y `app/lib/platform/device/{app_environment,pending_pulse_file}.dart`.
- `app/android/{gradle.properties,settings.gradle.kts,app/build.gradle.kts}` y `app/android/app/src/main/kotlin/com/pulsys/app/MainActivity.kt`.
- `app/ios/Runner/AppDelegate.swift` y `app/ios/Runner.xcodeproj/project.pbxproj`.
- `app/integration_test/pulse_flow_test.dart`, `.github/workflows/flutter-ci.yml` y este informe.

## 3. Comandos y resultados

- `Get-Content -LiteralPath 'AGENTS.md'` vía PowerShell fue rechazado al crear el proceso con `CreateProcessAsUserW failed: 5 (Access is denied)`. Leí las reglas y el resto de los archivos con `node:fs`.
- `node:fs` y `node:assert/strict` completaron cinco grupos de comprobaciones estáticas. Verifiqué la rama, ocho archivos de contrato y datos frente a la lectura inicial, las ocho funciones esperadas del puente, los pines y rutas de FRB, las dos ABI de Android y los tres perfiles del Runner de Xcode. Los cinco archivos Rust de producción del puente pasan la búsqueda de bloques `unsafe`, `unwrap`, `expect` y `panic`.
- Las configuraciones locales de los AVD registran `Pulsys_API26_lowram` con API 26, `x86_64` y 1024 MB, y `Pixel_7_Pro` con imagen 37.1, `x86_64` y 2048 MB. La comprobación se hizo sobre `config.ini`.

**NO EJECUTADO** `cargo fetch --manifest-path crates/bridge/Cargo.toml --target x86_64-pc-windows-msvc`.

**NO EJECUTADO** `flutter pub get`, `flutter_rust_bridge_codegen generate` y `dart run build_runner build`.

**NO EJECUTADO** `cargo fmt --all -- --check`.

**NO EJECUTADO** `cargo clippy --locked -p pulsys-api -p pulsys-bridge --all-targets -- -D warnings`.

**NO EJECUTADO** `cargo test -p pulsys-bridge --locked`.

**NO EJECUTADO** `cargo test --locked -p pulsys-core -p pulsys-store -p pulsys-api -p pulsys-bridge`.

**NO EJECUTADO** `flutter analyze --fatal-infos`, `dart format --output=none --set-exit-if-changed lib test integration_test` y `flutter test`.

**NO EJECUTADO** `flutter build apk --debug` y `flutter test integration_test/pulse_flow_test.dart -d <serial>` en ambos AVD.

**NO EJECUTADO** `flutter build ios --simulator --debug`, `flutter test integration_test/pulse_flow_test.dart -d <udid>`, `xcrun simctl io <udid> screenshot` y `flutter build ios --release --no-codesign`.

## 4. Pendiente y decisiones

- Claude debe generar `crates/bridge/src/frb_generated.rs`, `app/lib/platform/generated/` y `app/ios/Runner/frb_generated.h`, y actualizar los locks con Cargo y Flutter. El modo compacto de FRB produce una cabecera sin declaraciones C. Xcode carga el archivo estático completo y Dart resuelve los símbolos desde el proceso. [Generador de cabecera FRB](https://github.com/fzyzcjy/flutter_rust_bridge/blob/v2.11.1/frb_codegen/src/library/codegen/generator/wire/c/spec_generator/mod.rs) · [Cargador de bibliotecas de FRB](https://raw.githubusercontent.com/fzyzcjy/flutter_rust_bridge/v2.11.1/frb_dart/lib/src/platform_types/_io.dart)
- `rust_builder` integra `cargo-ndk` en las fuentes JNI de cada variante Gradle y aporta el script que ejecuta Xcode. Ambos builds apuntan al crate del workspace. El canal nativo entrega el directorio privado y la versión instalada del runner para cumplir §2. [Directorio NDK de Gradle](https://developer.android.com/reference/tools/gradle-api/9.3/com/android/build/api/dsl/SdkComponents#ndkDirectory()) · [cargo-ndk](https://github.com/bbqsrc/cargo-ndk/tree/v4.1.2)
- `serde`, `serde_json` y `tempfile` ya estaban resueltos en el workspace. La conversión usa los nombres Serde de la acción de la fachada para preservar los campos privados. Si cambia ese esquema, hay que adaptar la conversión y conservar el lector V1.
- El adaptador mantiene el DTO original con la identidad del destino que conserva `PreparedPulse.approve()`. El contrato Dart sirve para todo el recorrido pedido. Si al recuperar al arranque Rust exige otra confirmación, termina ese envío autorizado sin escribir. Para volver a mostrar ese borrador haría falta acordar con Hydrogen un estado de recuperación en la interfaz.
- Los comandos offline deshabilitan Wi-Fi y datos de los AVD durante la prueba. iOS ejecuta el escenario con las peticiones HTTP de Dart bloqueadas en la prueba. La validez del enlace, el runtime iOS, la captura y la ejecución de ambas ABI siguen pendientes. Los dos AVD locales son `x86_64`. El contenido `arm64-v8a` del APK se comprueba inspeccionando el ZIP.

### Comandos para Claude en Windows

Ejecutar en PowerShell 7. Dentro de `app/`, ejecutar el bloque de generación directamente. `integrate` sobrescribiría la configuración del arranque con el ejemplo de FRB. Este proyecto ya tiene el paquete local y las fases de compilación.

```powershell
Set-Location 'C:\Users\Salva\Desktop\git\nasa.spaceapps'
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$env:ANDROID_HOME = 'C:\Users\Salva\AppData\Local\Android\Sdk'
$env:JAVA_HOME = 'C:\Program Files\Android\Android Studio\jbr'
$env:Path = "C:\dev\flutter\bin;C:\Users\Salva\.cargo\bin;$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:Path"
$env:RUSTUP_TOOLCHAIN = '1.96.0'

rustup toolchain install 1.96.0 --profile minimal
rustup component add rustfmt clippy
rustup target add aarch64-linux-android x86_64-linux-android
cargo install --locked cargo-expand --version 1.0.118
rustc --version
flutter --version
dart --version
flutter_rust_bridge_codegen --version
cargo ndk --version

cargo fetch --manifest-path crates/bridge/Cargo.toml --target x86_64-pc-windows-msvc

Push-Location app
flutter pub get
flutter_rust_bridge_codegen generate
dart run build_runner build
dart format lib/platform/generated lib/main.dart lib/platform/rust_adapter.dart lib/platform/device integration_test
Pop-Location

cargo fmt -p pulsys-bridge
cargo test -p pulsys-bridge --locked
cargo clippy --locked -p pulsys-api -p pulsys-bridge --all-targets -- -D warnings
cargo test --locked -p pulsys-core -p pulsys-store -p pulsys-api

Push-Location app
flutter pub get --enforce-lockfile
flutter analyze --fatal-infos
flutter test
flutter build apk --debug
```

Desde `app/`, comprobar el APK y ejecutar la prueba en los dos emuladores. El bloque reutiliza un AVD con el nombre indicado si ya está conectado. Si se arranca uno nuevo, usa los puertos 5560 y 5562.

```powershell
Add-Type -AssemblyName System.IO.Compression.FileSystem
$pulsysApk = (Resolve-Path 'build/app/outputs/flutter-apk/app-debug.apk').Path
$pulsysArchive = [System.IO.Compression.ZipFile]::OpenRead($pulsysApk)
try {
    $pulsysLibraries = @($pulsysArchive.Entries.FullName |
        Where-Object { $_ -match '^lib/[^/]+/libpulsys_bridge\.so$' } |
        Sort-Object)
    if (($pulsysLibraries -join ',') -ne 'lib/arm64-v8a/libpulsys_bridge.so,lib/x86_64/libpulsys_bridge.so') {
        throw "Unexpected Rust libraries: $pulsysLibraries"
    }
    $pulsysLibraries
} finally {
    $pulsysArchive.Dispose()
}

$pulsysAdb = Join-Path $env:ANDROID_HOME 'platform-tools\adb.exe'
$pulsysEmulator = Join-Path $env:ANDROID_HOME 'emulator\emulator.exe'
& $pulsysAdb start-server

function Invoke-PulsysOfflineIntegration {
    param([string] $Avd, [int] $Port)

    $pulsysSerial = $null
    foreach ($pulsysDeviceLine in (& $pulsysAdb devices)) {
        if ($pulsysDeviceLine -match '^(emulator-\d+)\s+device$') {
            $pulsysCandidate = $Matches[1]
            $pulsysName = (& $pulsysAdb -s $pulsysCandidate emu avd name | Select-Object -First 1).Trim()
            if ($pulsysName -eq $Avd) {
                $pulsysSerial = $pulsysCandidate
                break
            }
        }
    }

    if ($null -eq $pulsysSerial) {
        $pulsysSerial = "emulator-$Port"
        Start-Process -FilePath $pulsysEmulator -ArgumentList @('-avd', $Avd, '-port', $Port, '-no-window', '-no-snapshot-save') -WindowStyle Hidden
    }
    $pulsysDeadline = (Get-Date).AddMinutes(4)
    do {
        Start-Sleep -Seconds 2
        if ((Get-Date) -gt $pulsysDeadline) {
            throw "$Avd did not boot"
        }
        $pulsysConnected = (& $pulsysAdb devices) -match "^$pulsysSerial\s+device$"
    } until ($pulsysConnected -and (& $pulsysAdb -s $pulsysSerial shell getprop sys.boot_completed).Trim() -eq '1')

    & $pulsysAdb -s $pulsysSerial shell getprop ro.build.version.sdk
    & $pulsysAdb -s $pulsysSerial shell getprop ro.product.cpu.abi
    $pulsysWifi = (& $pulsysAdb -s $pulsysSerial shell settings get global wifi_on).Trim()
    $pulsysData = (& $pulsysAdb -s $pulsysSerial shell settings get global mobile_data).Trim()
    try {
        & $pulsysAdb -s $pulsysSerial shell svc wifi disable
        & $pulsysAdb -s $pulsysSerial shell svc data disable
        flutter test integration_test/pulse_flow_test.dart -d $pulsysSerial
    } finally {
        if ($pulsysWifi -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc wifi enable
        }
        if ($pulsysData -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc data enable
        }
    }
}

Invoke-PulsysOfflineIntegration -Avd 'Pulsys_API26_lowram' -Port 5560
Invoke-PulsysOfflineIntegration -Avd 'Pixel_7_Pro' -Port 5562
```

Después de la integración, ejecutar las comprobaciones de formato y revisar los archivos que aparecerán en el diff. Los bindings y los locks deben formar parte de la rama para que CI pueda comprobar una regeneración reproducible.

```powershell
dart format --output=none --set-exit-if-changed lib test integration_test
Pop-Location
cargo fmt --all -- --check
git diff --check
git status --short --untracked-files=all
```

Los comandos para iOS están en el job `ios`. `macos-15` proporciona un runner arm64. [Imágenes de GitHub Actions](https://github.com/actions/runner-images)

## Ronda 2

### 1. Qué cambié

Corregí las dos comparaciones de confirmación a `PulseConfirmation.required_`, el nombre que FRB genera al escapar la palabra reservada de Dart.
El helper `_optional` acepta ahora una conversión `R? Function(T)`, compatible con `Bpm.tryFrom` y con las conversiones que siempre devuelven un valor.
Añadí `allow(clippy::excessive_nesting)` exclusivamente a `mod frb_generated`, con el motivo verificable en esa misma línea. El motivo del `allow` previo de `unwrap_used` y `panic` también quedó en su línea.
Revisé el aviso de lifetimes en el parser de FRB 2.11.1. Es informativo cuando esa opción está desactivada. Las funciones actuales devuelven DTO propios y una sesión propietaria, por lo que no requieren `enable_lifetime: true`.

### 2. Archivos tocados

- `crates/bridge/src/lib.rs`.
- `app/lib/platform/rust_adapter.dart`.
- `reviews/etapa2-puente-sol.md`.

### 3. Comandos y resultados

- `flutter pub get --enforce-lockfile`, desde `app/`, pasó. Recuperó las dependencias faltantes en la caché local con `build_runner 2.15.1`, la versión corregida por el usuario.
- `dart format --output=none --set-exit-if-changed lib/platform/rust_adapter.dart`, desde `app/`, pasó con 1 archivo comprobado y 0 cambios.
- `flutter analyze --fatal-infos --no-pub`, desde `app/`, pasó con `No issues found!`. El primer intento había fallado por paquetes ausentes en la caché local. Se repitió después de recuperar las dependencias.
- `cargo fmt -p pulsys-bridge -- --check`, con Rust 1.96.0, pasó.
- `cargo clippy --locked -p pulsys-api -p pulsys-bridge --all-targets -- -D warnings`, con Rust 1.96.0, pasó.
- `cargo test --locked -p pulsys-bridge`, con Rust 1.96.0, pasó con 2 pruebas del worker y 2 de recuperación, 4 aprobadas y 0 fallidas. También pasó la fase de documentación con 0 pruebas.
- `cargo fmt --all -- --check`, con Rust 1.96.0, falló por diferencias de formato en `crates/core/src/telemetry.rs` y `crates/store/src/records.rs`.

### 4. Pendiente

El formato global sigue pendiente en los dos archivos indicados de core y store, fuera del alcance de esta ronda. La comprobación de formato de bridge pasa.
Los dos errores de la constante y el error de nulabilidad quedaron resueltos según el analizador. Clippy acepta el módulo generado con la excepción acotada.

## Ronda 3

### 1. Qué cambié

Conecté la recuperación durable con el controlador y el diálogo, preservando la intención renovada hasta la decisión del usuario.
Corregí la confirmación consecutiva, los reintentos y la expiración, y añadí la corrección desde el historial y los estados de refresco fallido.
Añadí una golden inicial con fuentes locales, regresiones del adaptador y de la UI, un caso Rust con reloj controlado y el recorrido móvil de formulario a corrección y reapertura.
La CI comprueba el formato de API/bridge y regenera también la localización. Los mensajes nuevos usan el catálogo inglés.

| Hallazgo | Cambio | Archivo y línea | Prueba nueva o ampliada |
|---|---|---|---|
| 1 | Conservar y volver a presentar la preparación renovada, con la misma clave, hasta aprobar o cancelar. | [app/lib/platform/rust_adapter.dart:171](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/platform/rust_adapter.dart:171) · [app/lib/features/pulse/pulse_controller.dart:90](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:90) | [app/test/platform/rust_adapter_recovery_test.dart:51](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/platform/rust_adapter_recovery_test.dart:51) · [app/test/platform/rust_adapter_recovery_test.dart:86](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/platform/rust_adapter_recovery_test.dart:86) · [crates/bridge/tests/pulse_recovery.rs:99](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/bridge/tests/pulse_recovery.rs:99) |
| 2, golden | Primera golden del estado inicial, con reloj fijo, escala 1, plataforma Android y fuentes locales. | [app/pubspec.yaml:44](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/pubspec.yaml:44) | [app/test/goldens/pulse_initial_test.dart:25](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/goldens/pulse_initial_test.dart:25) |
| 3 | Estado de confirmación explícito y bucle que atiende la preparación vigente al terminar cada diálogo. | [app/lib/platform/pulsys_api.dart:174](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/platform/pulsys_api.dart:174) · [app/lib/features/pulse/pulse_screen.dart:112](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_screen.dart:112) | [app/test/widgets/pulse_review_test.dart:27](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:27) |
| 4 | Bloquear el envío ya completado hasta editar. Preparar una intención nueva y revisar el destino tras expirar. Reintentar el DTO exacto ante un fallo transitorio. | [app/lib/features/pulse/pulse_controller.dart:234](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:234) · [app/lib/features/pulse/pulse_controller.dart:351](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:351) | [app/test/features/pulse_controller_test.dart:16](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:16) · [app/test/features/pulse_controller_test.dart:36](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:36) · [app/test/features/pulse_controller_test.dart:119](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:119) |
| 5 | Error unsupported_action_version/action.version, conservación de los bytes y estado visible de recuperación bloqueada. | [crates/bridge/src/durable.rs:20](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/bridge/src/durable.rs:20) · [app/lib/platform/rust_adapter.dart:192](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/platform/rust_adapter.dart:192) · [app/lib/features/pulse/pulse_screen.dart:679](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_screen.dart:679) | [crates/bridge/tests/pulse_recovery.rs:89](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/bridge/tests/pulse_recovery.rs:89) · [app/test/platform/rust_adapter_recovery_test.dart:115](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/platform/rust_adapter_recovery_test.dart:115) · [app/test/widgets/pulse_review_test.dart:117](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:117) |
| 6 | Identificar estado e historial anteriores como última lectura disponible, con reintentos independientes y recibo conservado. | [app/lib/features/pulse/pulse_controller.dart:125](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:125) · [app/lib/features/pulse/pulse_screen.dart:244](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_screen.dart:244) · [app/lib/features/pulse/pulse_screen.dart:531](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_screen.dart:531) | [app/test/features/pulse_controller_test.dart:94](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:94) · [app/test/widgets/pulse_review_test.dart:62](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:62) |
| 7 | Añadir corrección desde una fila, transportar replaces y probar el recorrido de UI con reapertura del adaptador. | [app/lib/features/pulse/pulse_controller.dart:151](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:151) · [app/lib/features/pulse/pulse_screen.dart:596](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_screen.dart:596) | [app/test/widgets/pulse_review_test.dart:92](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:92) · [app/integration_test/pulse_flow_test.dart:121](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:121) |
| 8 | Formato limitado a api/bridge, gen-l10n y control de diferencias y archivos nuevos en ambos generados. | [.github/workflows/flutter-ci.yml:28](C:/Users/Salva/Desktop/git/nasa.spaceapps/.github/workflows/flutter-ci.yml:28) · [.github/workflows/flutter-ci.yml:67](C:/Users/Salva/Desktop/git/nasa.spaceapps/.github/workflows/flutter-ci.yml:67) | [app/test/config/flutter_ci_test.dart:6](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/config/flutter_ci_test.dart:6) |
| 9 | Mostrar una representación decimal que conserve el valor aceptado, incluido 0.04. | [app/lib/platform/pulsys_api.dart:29](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/platform/pulsys_api.dart:29) | [app/test/platform/bpm_test.dart:5](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/platform/bpm_test.dart:5) · [app/test/widgets/pulse_review_test.dart:27](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:27) |
| 10 | Usar evaluación de fixture predeterminada o inyectada y una huella tipada con la identidad completa del destino. | [app/test/fakes/fake_pulsys_api.dart:350](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/fakes/fake_pulsys_api.dart:350) · [app/test/fakes/fake_pulsys_api.dart:373](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/fakes/fake_pulsys_api.dart:373) | [app/test/fakes/fake_pulsys_api_test.dart:7](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/fakes/fake_pulsys_api_test.dart:7) · [app/test/fakes/fake_pulsys_api_test.dart:31](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/fakes/fake_pulsys_api_test.dart:31) · [app/test/fakes/fake_pulsys_api_test.dart:56](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/fakes/fake_pulsys_api_test.dart:56) |
| 11 | Localizar el error de apertura y retirar instrucciones de plantilla en pubspec y Android. | [app/lib/main.dart:61](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/main.dart:61) · [app/lib/l10n/app_en.arb:107](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/l10n/app_en.arb:107) | [app/test/widgets/pulse_review_test.dart:138](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:138) · [app/test/config/flutter_ci_test.dart:16](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/config/flutter_ci_test.dart:16) |

### 2. Archivos tocados

- `crates/bridge/src/durable.rs` y `crates/bridge/tests/pulse_recovery.rs`.
- `app/lib/platform/{pulsys_api,rust_adapter}.dart` y el nuevo `app/lib/platform/device/rust_session.dart`.
- `app/lib/features/pulse/{pulse_controller,pulse_screen,pulse_confirmation_dialog}.dart`, `app/lib/main.dart` y `app/lib/l10n/app_en.arb`.
- `app/lib/design/fonts/` con tres archivos TTF y sus tres licencias OFL, y `app/pubspec.yaml`.
- `app/test/fakes/{fake_pulsys_api,fake_rust_session,fake_pulsys_api_test}.dart`, `app/test/features/pulse_controller_test.dart`, `app/test/platform/{bpm_test,rust_adapter_recovery_test}.dart`, `app/test/widgets/pulse_review_test.dart`, `app/test/config/flutter_ci_test.dart` y `app/test/goldens/pulse_initial_test.dart`.
- `app/integration_test/pulse_flow_test.dart`, `.github/workflows/flutter-ci.yml`, `app/android/app/build.gradle.kts`, `app/android/app/src/main/AndroidManifest.xml` y este informe.

El contrato Dart incorpora `PulseConfirmation`, `PulseRecovery`, `NoPendingPulse`, `RecoveredPulse`, `RecoveredReceipt` y `PulseRecoveryBlocked`. Los nombres de estos estados adicionales no figuraban en la estructura original. `RustSession` y `NativeRustSession` delimitan las llamadas generadas y permiten probar el mismo adaptador con un archivo pendiente real sin cargar una biblioteca nativa en los tests de Flutter.

Las fuentes se guardaron bajo `lib/design/fonts/`, dentro de los archivos autorizados para esta ronda. Los TTF y las licencias están declarados en el paquete. Sus revisiones de origen quedan fijadas por URL. [Barlow Semi Condensed](https://github.com/google/fonts/tree/3218ae2acc5bce7ccd97c477965b5b4094241fb0/ofl/barlowsemicondensed) · [Public Sans](https://github.com/google/fonts/tree/8b0a1d0f5983c89bc2b93f1b5fb55f9e252744b5/ofl/publicsans) · [Roboto Mono](https://github.com/google/fonts/tree/aa24661a5ba24c8f8a550a16fb13198719fc2c23/ofl/robotomono)

### 3. Comprobaciones y comandos

El usuario confirmó para la entrega anterior Clippy y formato limpios en API/bridge, bridge 2+2, análisis Flutter limpio, widgets 8/8, APK con ambas ABI e integración offline en API 26 y API 37 x86_64. Son resultados anteriores a estos cambios.

En esta ronda usé `node:fs/promises` y `node:assert/strict` para comprobar referencias del catálogo, imports locales, delimitadores de 17 archivos Dart manuales, pines de dependencias, recursos TTF y el control de generación en CI. Comparé el archivo Dart generado con la lectura inicial y comprobé que los dos archivos Android conservan el mismo contenido funcional al retirar los comentarios. La rama leída sigue siendo `flutter-migration`.

**NO EJECUTADO** Cargo, Flutter, Dart, generadores, builds y emuladores en esta ronda, por la instrucción del usuario de no ejecutar procesos. Hay 24 declaraciones nuevas de tests Flutter, dos casos Rust adicionales y un segundo recorrido móvil. Sus resultados y la referencia PNG siguen pendientes de ejecución.

#### Secuencia exacta en PowerShell 7

Ejecutar desde la raíz del repositorio. Las firmas y los tipos expuestos a FRB conservan su forma. La regeneración incluida comprueba la reproducibilidad del puente. El cambio de ARB requiere `flutter gen-l10n`.

```powershell
Set-Location 'C:\Users\Salva\Desktop\git\nasa.spaceapps'
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$env:RUSTUP_TOOLCHAIN = '1.96.0'
$env:ANDROID_HOME = 'C:\Users\Salva\AppData\Local\Android\Sdk'
$env:JAVA_HOME = 'C:\Program Files\Android\Android Studio\jbr'
$env:Path = "C:\dev\flutter\bin;C:\Users\Salva\.cargo\bin;$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:Path"

cargo fmt -p pulsys-bridge
cargo fmt -p pulsys-api -p pulsys-bridge -- --check
cargo clippy --locked -p pulsys-api -p pulsys-bridge --all-targets -- -D warnings
cargo test --locked -p pulsys-core -p pulsys-store -p pulsys-api -p pulsys-bridge

Push-Location app
flutter pub get --enforce-lockfile
flutter_rust_bridge_codegen generate
dart run build_runner build
flutter gen-l10n
dart format lib test integration_test
flutter analyze --fatal-infos
dart format --output=none --set-exit-if-changed lib test integration_test

flutter test --update-goldens test/goldens/pulse_initial_test.dart
flutter test test/goldens/pulse_initial_test.dart
flutter test
flutter build apk --debug

```

La referencia que crea el comando es `app/test/goldens/baselines/pulse_initial.png`. Revisar esa imagen antes de aceptarla y conservarla en la entrega junto con las fuentes.

Desde `app/`, ejecutar las dos pruebas de integración en cada AVD con este bloque. Reutiliza un AVD conectado por su nombre o lo inicia sin ventana y conserva el estado previo de Wi-Fi y datos.

```powershell
$pulsysAdb = Join-Path $env:ANDROID_HOME 'platform-tools\adb.exe'
$pulsysEmulator = Join-Path $env:ANDROID_HOME 'emulator\emulator.exe'
& $pulsysAdb start-server

function Invoke-PulsysRound3Integration {
    param([string] $Avd, [int] $Port)

    $pulsysSerial = $null
    foreach ($pulsysDeviceLine in (& $pulsysAdb devices)) {
        if ($pulsysDeviceLine -match '^(emulator-\d+)\s+device$') {
            $pulsysCandidate = $Matches[1]
            $pulsysAvdName = (& $pulsysAdb -s $pulsysCandidate emu avd name |
                Select-Object -First 1).Trim()
            if ($pulsysAvdName -eq $Avd) {
                $pulsysSerial = $pulsysCandidate
                break
            }
        }
    }

    if ($null -eq $pulsysSerial) {
        $pulsysSerial = "emulator-$Port"
        Start-Process -FilePath $pulsysEmulator -ArgumentList @(
            '-avd', $Avd, '-port', $Port, '-no-window', '-no-snapshot-save'
        ) -WindowStyle Hidden
    }

    $pulsysDeadline = (Get-Date).AddMinutes(4)
    do {
        Start-Sleep -Seconds 2
        if ((Get-Date) -gt $pulsysDeadline) {
            throw "$Avd did not boot"
        }
        $pulsysConnected = (& $pulsysAdb devices) -match "^$pulsysSerial\s+device$"
    } until ($pulsysConnected -and
        (& $pulsysAdb -s $pulsysSerial shell getprop sys.boot_completed).Trim() -eq '1')

    & $pulsysAdb -s $pulsysSerial shell getprop ro.build.version.sdk
    & $pulsysAdb -s $pulsysSerial shell getprop ro.product.cpu.abi
    $pulsysWifi = (& $pulsysAdb -s $pulsysSerial shell settings get global wifi_on).Trim()
    $pulsysData = (& $pulsysAdb -s $pulsysSerial shell settings get global mobile_data).Trim()
    try {
        & $pulsysAdb -s $pulsysSerial shell svc wifi disable
        & $pulsysAdb -s $pulsysSerial shell svc data disable
        flutter test integration_test/pulse_flow_test.dart -d $pulsysSerial
    } finally {
        if ($pulsysWifi -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc wifi enable
        }
        if ($pulsysData -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc data enable
        }
    }
}

Invoke-PulsysRound3Integration -Avd 'Pulsys_API26_lowram' -Port 5560
Invoke-PulsysRound3Integration -Avd 'Pixel_7_Pro' -Port 5562
Pop-Location

cargo fmt -p pulsys-api -p pulsys-bridge -- --check
git diff --check
git status --short --untracked-files=all

```

### 4. Pendiente y límites de la evidencia

- Ejecutar la secuencia anterior, revisar la primera PNG y registrar los resultados de esta ronda. La golden se añadió como prueba y todavía necesita que el usuario genere la referencia.
- El segundo test móvil cubre el formulario completo, una nueva confirmación para la corrección y la reapertura del adaptador en el mismo proceso. El relanzamiento real del proceso continúa pendiente de un arnés de lanzamiento externo. Esta es la alternativa aceptada por el usuario para el hallazgo 7.
- La ejecución en API 26 arm64 queda pendiente. Este PC x86 no dispone de emulación arm64 para API 26. Las ejecuciones previas y los comandos de esta ronda corresponden a los AVD x86_64.
- El resultado del job iOS y la matriz de enlaces opcionales del hallazgo 2 continúan pendientes. Esta ronda atiende la parte de la golden que asignó el usuario.
- El formato global de core/store queda bajo sus responsables. Durante la adopción la CI y esta secuencia aplican `fmt --check` a `pulsys-api` y `pulsys-bridge`, conforme a la indicación de la tarea.

## Ronda 4

### 1. Qué cambié y causa del fallo

Mantuve 201 bpm y la exigencia de confirmación. La regla implementada usa el rango plausible inclusivo 30–200: 201 activa `unusual_value/plaus.heart_rate`; `prepare_pulse` entrega el estado `Required` y una confirmación sin aprobar devuelve `NeedsConfirmation` sin insertar la lectura. El título y la acción del test coincidían con el catálogo inglés. [rules.rs:45](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/core/src/rules.rs:45) · [plausibility.rs:30](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/core/src/plausibility.rs:30) · [pulse.rs](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse.rs) · [app_en.arb](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/l10n/app_en.arb)

Corregí la sincronización del arnés: el botón se habilita al terminar la recuperación, antes de que terminen las lecturas independientes de historial y zonas. El estado de zonas puede añadir una tarjeta y mover Guardar. Además, `IntegrationTestWidgetsFlutterBinding` conserva el teclado nativo (`registerTestTextInput = false`), mientras que los tests de widgets usan el teclado simulado. El test anterior desplazaba y pulsaba sin cerrar ese teclado ni comprobar el hit test. Esas condiciones permiten que el toque no llegue a Guardar y que solo aparezca el timeout genérico. La nueva secuencia espera la carga inicial, comprueba el texto del controlador, cierra el teclado, espera sus insets a cero y exige un objetivo alcanzable antes de tocarlo.

**Límite del diagnóstico:** el fallo anterior no registró fase, error, contenido de los campos ni historial, y no ejecuté el emulador en esta ronda. La carrera del arnés está identificada en el código; atribuir con certeza aquella ejecución a un toque perdido sigue pendiente de verificar. No hay evidencia de que 201 deba guardarse sin diálogo ni de que el literal inglés fuera incorrecto. Los nuevos fallos muestran el estado necesario para distinguir un toque perdido, una validación, un fallo del adaptador o un guardado inesperado.

El recorrido comprueba ahora, con Rust real y hora omitida como en el formulario, que preparar 201 exige revisión y no escribe filas. Usa el catálogo generado para encontrar el diálogo y su acción; falla inmediatamente si encuentra un error o un guardado antes de la confirmación. Conserva las comprobaciones de cero filas antes de aprobar, corrección y reapertura. Desactivé `debugShowCheckedModeBanner` en la raíz `PulsysApp`, responsable de la presentación global según §2 de la estructura, y la golden exige su ausencia.

| Cambio | Archivo:línea | Prueba |
|---|---|---|
| Preparación real de 201 con hora omitida; aviso esperado y cero escrituras. | [pulse_flow_test.dart:134](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:134) | Segundo test de integración, antes de montar la UI. |
| Espera de carga inicial, texto recibido y diálogo obtenido del catálogo. | [pulse_flow_test.dart:152](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:152) | Formulario → confirmación → corrección → reapertura en API 26 y API 37. |
| Cierre del teclado y comprobación del objetivo antes del toque. | [pulse_flow_test.dart:275](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:275) | Cada pulsación del segundo test pasa por `_tapVisible`. |
| Diagnóstico de fase, errores, campos, botones, teclado, textos de la ruta e historial efectivo; detección de guardado sin diálogo. | [pulse_flow_test.dart:298](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:298) · [pulse_flow_test.dart:320](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:320) | Ambos tests de integración usan la espera diagnosticable. Los textos incluyen contenido desplazado fuera de la vista. La consulta de historial tiene límite de tres segundos. |
| Franja DEBUG desactivada en la app. | [app.dart:17](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/app.dart:17) | [pulse_initial_test.dart:51](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/goldens/pulse_initial_test.dart:51): propiedad falsa, ningún `CheckedModeBanner` y comparación PNG. |

### 2. Archivos tocados

- `app/integration_test/pulse_flow_test.dart`.
- `app/lib/app.dart`.
- `app/test/goldens/pulse_initial_test.dart`.
- `reviews/etapa2-puente-sol.md`.

Conservé las dos correcciones del usuario en `pulse_screen.dart` y `fake_rust_session.dart`. La PNG existente y los bindings generados no se modificaron. No cambié Rust, firmas FRB, dependencias ni ARB.

### 3. Comprobaciones y comandos

**Resultados comunicados por el usuario, anteriores a esta ronda:** formato y Clippy de API/bridge limpios; core 86, store 70, API 9+1 y bridge 2+4 aprobados; FRB, build_runner y gen-l10n aprobados; análisis sin incidencias, formato limpio y Flutter 32/32; golden generada y APK debug aprobado. El primer test de integración pasa. El segundo falla al esperar el primer diálogo en API 26 y API 37 x86_64.

En esta ronda ejecuté lecturas con `node:fs/promises` y comprobaciones con `node:assert/strict`: coincidencia de umbral y catálogo, presencia de las dos guardas contra guardado sin diálogo, orden de cierre del teclado y hit test, cobertura de diagnósticos y ausencia del banner. Pasaron esas comprobaciones estructurales. Verifiqué la rama `flutter-migration` y que el formulario conserva exactamente la lectura inicial de esta ronda. También comprobé hashes de la pantalla, el fake corregido, la PNG y el Rust generado antes y después de las ediciones finales.

**NO EJECUTADO** Dart, Flutter, Cargo, generadores, builds y emuladores por la instrucción del usuario de no ejecutar procesos. Las comprobaciones estructurales no sustituyen analyze ni los tests. La PNG actual todavía contiene la franja y debe regenerarse.

#### Secuencia exacta en PowerShell 7

No hace falta regenerar FRB, ejecutar build_runner ni gen-l10n: sus entradas no cambiaron. Ejecutar desde la raíz:

```powershell
Set-Location 'C:\Users\Salva\Desktop\git\nasa.spaceapps'
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$env:RUSTUP_TOOLCHAIN = '1.96.0'
$env:ANDROID_HOME = 'C:\Users\Salva\AppData\Local\Android\Sdk'
$env:JAVA_HOME = 'C:\Program Files\Android\Android Studio\jbr'
$env:Path = "C:\dev\flutter\bin;C:\Users\Salva\.cargo\bin;$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:Path"

Push-Location app
dart format lib/app.dart test/goldens/pulse_initial_test.dart integration_test/pulse_flow_test.dart
flutter analyze --fatal-infos
dart format --output=none --set-exit-if-changed lib test integration_test
flutter test --update-goldens test/goldens/pulse_initial_test.dart
flutter test test/goldens/pulse_initial_test.dart
flutter test
flutter build apk --debug
```

Revisar `app/test/goldens/baselines/pulse_initial.png`: debe conservar las fuentes locales y mostrar el estado inicial sin la franja DEBUG. Después, todavía desde `app/`, ejecutar las dos pruebas con la red desactivada en cada AVD:

```powershell
$pulsysAdb = Join-Path $env:ANDROID_HOME 'platform-tools\adb.exe'
$pulsysEmulator = Join-Path $env:ANDROID_HOME 'emulator\emulator.exe'
& $pulsysAdb start-server

function Invoke-PulsysRound4Integration {
    param([string] $Avd, [int] $Port)

    $pulsysSerial = $null
    foreach ($pulsysDeviceLine in (& $pulsysAdb devices)) {
        if ($pulsysDeviceLine -match '^(emulator-\d+)\s+device$') {
            $pulsysCandidate = $Matches[1]
            $pulsysAvdName = (& $pulsysAdb -s $pulsysCandidate emu avd name |
                Select-Object -First 1).Trim()
            if ($pulsysAvdName -eq $Avd) {
                $pulsysSerial = $pulsysCandidate
                break
            }
        }
    }

    if ($null -eq $pulsysSerial) {
        $pulsysSerial = "emulator-$Port"
        Start-Process -FilePath $pulsysEmulator -ArgumentList @(
            '-avd', $Avd, '-port', $Port, '-no-window', '-no-snapshot-save'
        ) -WindowStyle Hidden
    }

    $pulsysDeadline = (Get-Date).AddMinutes(4)
    do {
        Start-Sleep -Seconds 2
        if ((Get-Date) -gt $pulsysDeadline) {
            throw "$Avd did not boot"
        }
        $pulsysConnected = (& $pulsysAdb devices) -match "^$pulsysSerial\s+device$"
    } until ($pulsysConnected -and
        (& $pulsysAdb -s $pulsysSerial shell getprop sys.boot_completed).Trim() -eq '1')

    & $pulsysAdb -s $pulsysSerial shell getprop ro.build.version.sdk
    & $pulsysAdb -s $pulsysSerial shell getprop ro.product.cpu.abi
    $pulsysWifi = (& $pulsysAdb -s $pulsysSerial shell settings get global wifi_on).Trim()
    $pulsysData = (& $pulsysAdb -s $pulsysSerial shell settings get global mobile_data).Trim()
    try {
        & $pulsysAdb -s $pulsysSerial shell svc wifi disable
        & $pulsysAdb -s $pulsysSerial shell svc data disable
        flutter test integration_test/pulse_flow_test.dart -d $pulsysSerial --reporter expanded
    } finally {
        if ($pulsysWifi -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc wifi enable
        }
        if ($pulsysData -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc data enable
        }
    }
}

Invoke-PulsysRound4Integration -Avd 'Pulsys_API26_lowram' -Port 5560
Invoke-PulsysRound4Integration -Avd 'Pixel_7_Pro' -Port 5562
Pop-Location

git diff --check
git status --short --untracked-files=all

```

### 4. Pendiente y límites de la evidencia

- Ejecutar la secuencia, regenerar y revisar la golden y confirmar ambos tests en cada AVD. Si falla una espera, conservar el mensaje completo con `phase`, `fields`, `routeTexts` y `effectiveHistory`; ya no se pierde ese contexto en el timeout.
- La reapertura continúa siendo del adaptador en el mismo proceso. El relanzamiento real del proceso y la ejecución API 26 arm64 siguen pendientes en los términos aceptados en la Ronda 3; este PC x86 no puede aportar esa ejecución arm64.
- Esta ronda no aporta resultados nuevos de iOS ni cierra la etapa 2 por sí sola.

## Ronda 5

### 1. Qué cambié y causa

Corregí la entrada del test: ahora toca el campo bpm antes de `enterText`, verifica foco y edición habilitada, y comprueba que tanto el editor como el controlador reciben el valor.
Añadí una regresión de widget con el teclado de pruebas desregistrado, que conserva la caché del editor al perder el foco y cubre 201 → confirmación → corrección a 202 → refresco → nuevo guardado.
Amplié el diagnóstico móvil con `canEdit`, `canSubmit`, `replaces`, foco y `readOnly` de cada editor.
El cambio afecta solo a las pruebas. El controlador sí habilita la corrección y `_syncText` conserva el texto actualizado por `onChanged`; la nueva regresión comprueba ambos caminos.

#### Causa en Flutter 3.50.0

Leí el SDK instalado en `C:/dev/flutter`, cuya caché declara Flutter **3.50.0**, Dart **3.14.0** y revisión de framework `ab2912035e160d9cad6306df307576da6128ebdb`. La secuencia que explica el fallo es:

1. `IntegrationTestWidgetsFlutterBinding.registerTestTextInput` devuelve `false`: los mensajes de cierre salen por el canal nativo. [integration_test.dart:99](C:/dev/flutter/packages/integration_test/lib/integration_test.dart:99)
2. `WidgetTester.enterText` llama a `showKeyboard`, que asigna el estado del editor a `binding.focusedEditable`. El setter solo llama a `requestKeyboard()` cuando recibe **otra instancia**. La segunda edición corresponde al mismo `EditableTextState` del campo bpm. [widget_tester.dart:1120](C:/dev/flutter/packages/flutter_test/lib/src/widget_tester.dart:1120) · [widget_tester.dart:1159](C:/dev/flutter/packages/flutter_test/lib/src/widget_tester.dart:1159) · [binding.dart:1616](C:/dev/flutter/packages/flutter_test/lib/src/binding.dart:1616)
3. La limpieza de esa caché está conectada a `TestTextInput.onCleared`, que ejecuta el stub cuando intercepta `TextInput.clearClient`. Como el stub no está registrado en integración, perder el foco cierra la conexión pero deja la instancia en la caché del binding. Así, tras Guardar y Corregir, otro `enterText` al mismo campo no vuelve a enfocarlo ni a abrir la conexión. [binding.dart:1299](C:/dev/flutter/packages/flutter_test/lib/src/binding.dart:1299) · [test_text_input.dart:141](C:/dev/flutter/packages/flutter_test/lib/src/test_text_input.dart:141)
4. `TestTextInput.enterText` inyecta un mensaje de edición; el despachador de `TextInput` lo descarta si no hay conexión actual. No se llega a `onChanged`, por eso el controlador conserva 201. Esto también explica por qué el primer ingreso funciona y los widgets con el stub registrado no mostraban el fallo. [test_text_input.dart:210](C:/dev/flutter/packages/flutter_test/lib/src/test_text_input.dart:210) · [text_input.dart:2231](C:/dev/flutter/packages/flutter/lib/src/services/text_input.dart:2231)

El toque al campo abre la conexión por la interacción normal del widget antes de inyectar el texto. El arnés no cambia directamente el `TextEditingController`, no llama a `updateBpm` ni registra un teclado falso en el test móvil. El SDK y la app conservan sus implementaciones.

Descarté el bloqueo por envío completado en el código: `startCorrection` limpia la intención enviada y la preparación, asigna `replaces`, pasa a `editing` y notifica; `canEdit` permite editar ese estado. `_syncText` escucha al controlador de dominio: cuando llega `onChanged`, `updateBpm` almacena 202 antes de notificar, por lo que el refresco copia 202. La prueba nueva exige campo habilitado, `readOnly == false`, nueva clave, texto 202 después de refrescar y recibo de corrección con la raíz original. [pulse_controller.dart:154](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:154) · [pulse_controller.dart:188](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:188) · [pulse_screen.dart:89](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_screen.dart:89)

| Cambio | Archivo:línea | Prueba |
|---|---|---|
| Tocar el campo, comprobar foco/edición e introducir bpm mediante el canal de entrada. | [pulse_input.dart:4](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/support/pulse_input.dart:4) | Helper compartido entre integración y la regresión del widget. Sin el toque, la segunda edición vuelve a usar la caché desconectada. |
| Verificar que Corregir activó `editing` con el `replaces` correcto antes de escribir 202. | [pulse_flow_test.dart:203](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:203) · [pulse_flow_test.dart:267](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:267) | Segundo recorrido móvil, con Rust/SQLite reales y reapertura del adaptador. |
| Reproducir editor en caché sin foco ni stub; editar y guardar la corrección. | [pulse_review_test.dart:232](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:232) | `correction refocuses a cached editor without the keyboard stub`: clave nueva, campo editable, 202 conservado tras refresco, ninguna fila nueva antes de confirmar y una versión efectiva con `replaces`/raíz correctos. El stub se restaura en `finally`. |
| Incluir estado del controlador y del editor si vuelve a fallar la entrada. | [pulse_flow_test.dart:267](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:267) · [pulse_flow_test.dart:345](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:345) | La entrada fallida adjunta el diagnóstico completo, incluido el historial efectivo. |

### 2. Archivos tocados

- `app/test/support/pulse_input.dart` (nuevo).
- `app/test/widgets/pulse_review_test.dart`.
- `app/integration_test/pulse_flow_test.dart`.
- `reviews/etapa2-puente-sol.md`.

El código de producción, las correcciones previas del usuario, el catálogo, los bindings y la PNG aprobada quedaron sin cambios. No se añadieron dependencias.

### 3. Comprobaciones y comandos

**Resultados comunicados por el usuario para Ronda 4:** `dart format` cambió un archivo, analyze limpio, golden regenerada sin DEBUG y aprobada, Flutter 32/32. En ambos AVD pasa la primera prueba de integración y el segundo recorrido ya confirma y guarda 201; falla al intentar introducir 202 en la corrección. Estos resultados son anteriores a los cambios de Ronda 5.

Usé `node:fs/promises` para leer el SDK y el repositorio, y `node:assert/strict` para verificar el orden toque → entrada, las dos llamadas al helper desde integración, la configuración y restauración del stub en la regresión y la presencia de los diagnósticos. Pasaron las comprobaciones estructurales. Los hashes de `pulse_controller.dart`, `pulse_screen.dart`, `app.dart`, el fake corregido por el usuario, la golden y el Rust generado coinciden con los del inicio de esta ronda.

**NO EJECUTADO** Dart, Flutter, Cargo, builds, generadores y emuladores, por la instrucción de no ejecutar procesos. El nuevo test de widget y el recorrido corregido necesitan ejecutarse. Hay una prueba Flutter adicional: se esperan 33 en la suite completa si todas pasan.

#### Secuencia exacta en PowerShell 7

No cambiaron entradas FRB, ARB, dependencias ni presentación: no hace falta codegen, build_runner, gen-l10n ni regenerar la golden. La suite completa comprueba la referencia PNG ya aprobada. La integración compila e instala su APK de pruebas.

```powershell
Set-Location 'C:\Users\Salva\Desktop\git\nasa.spaceapps'
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$env:RUSTUP_TOOLCHAIN = '1.96.0'
$env:ANDROID_HOME = 'C:\Users\Salva\AppData\Local\Android\Sdk'
$env:JAVA_HOME = 'C:\Program Files\Android\Android Studio\jbr'
$env:Path = "C:\dev\flutter\bin;C:\Users\Salva\.cargo\bin;$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:Path"

Push-Location app
dart format test/support/pulse_input.dart test/widgets/pulse_review_test.dart integration_test/pulse_flow_test.dart
flutter analyze --fatal-infos
dart format --output=none --set-exit-if-changed lib test integration_test
flutter test test/widgets/pulse_review_test.dart --reporter expanded
flutter test
```

Todavía desde `app/`, ejecutar las dos pruebas de integración offline en cada AVD:

```powershell
$pulsysAdb = Join-Path $env:ANDROID_HOME 'platform-tools\adb.exe'
$pulsysEmulator = Join-Path $env:ANDROID_HOME 'emulator\emulator.exe'
& $pulsysAdb start-server

function Invoke-PulsysRound5Integration {
    param([string] $Avd, [int] $Port)

    $pulsysSerial = $null
    foreach ($pulsysDeviceLine in (& $pulsysAdb devices)) {
        if ($pulsysDeviceLine -match '^(emulator-\d+)\s+device$') {
            $pulsysCandidate = $Matches[1]
            $pulsysAvdName = (& $pulsysAdb -s $pulsysCandidate emu avd name |
                Select-Object -First 1).Trim()
            if ($pulsysAvdName -eq $Avd) {
                $pulsysSerial = $pulsysCandidate
                break
            }
        }
    }

    if ($null -eq $pulsysSerial) {
        $pulsysSerial = "emulator-$Port"
        Start-Process -FilePath $pulsysEmulator -ArgumentList @(
            '-avd', $Avd, '-port', $Port, '-no-window', '-no-snapshot-save'
        ) -WindowStyle Hidden
    }

    $pulsysDeadline = (Get-Date).AddMinutes(4)
    do {
        Start-Sleep -Seconds 2
        if ((Get-Date) -gt $pulsysDeadline) {
            throw "$Avd did not boot"
        }
        $pulsysConnected = (& $pulsysAdb devices) -match "^$pulsysSerial\s+device$"
    } until ($pulsysConnected -and
        (& $pulsysAdb -s $pulsysSerial shell getprop sys.boot_completed).Trim() -eq '1')

    & $pulsysAdb -s $pulsysSerial shell getprop ro.build.version.sdk
    & $pulsysAdb -s $pulsysSerial shell getprop ro.product.cpu.abi
    $pulsysWifi = (& $pulsysAdb -s $pulsysSerial shell settings get global wifi_on).Trim()
    $pulsysData = (& $pulsysAdb -s $pulsysSerial shell settings get global mobile_data).Trim()
    try {
        & $pulsysAdb -s $pulsysSerial shell svc wifi disable
        & $pulsysAdb -s $pulsysSerial shell svc data disable
        flutter test integration_test/pulse_flow_test.dart -d $pulsysSerial --reporter expanded
    } finally {
        if ($pulsysWifi -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc wifi enable
        }
        if ($pulsysData -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc data enable
        }
    }
}

Invoke-PulsysRound5Integration -Avd 'Pulsys_API26_lowram' -Port 5560
Invoke-PulsysRound5Integration -Avd 'Pixel_7_Pro' -Port 5562
Pop-Location

git diff --check
git status --short --untracked-files=all

```

### 4. Pendiente y límites

- Ejecutar primero la regresión de widgets y después ambos tests en API 26 y API 37. La causa se fundamenta en el camino exacto del SDK instalado; no se presenta la nueva regresión como aprobada hasta ejecutar Flutter.
- Si falla la entrada, conservar el mensaje completo: ahora incluye foco, `readOnly`, `canEdit`, `canSubmit`, `replaces`, texto del editor/controlador e historial.
- Continúan los límites anteriores: reapertura del adaptador en el mismo proceso, relanzamiento real pendiente, API 26 arm64 pendiente por falta de emulación en este PC x86 e iOS sin resultados nuevos.

## Ronda 6

### 1. Qué cambié

Añadí la acción localizada «New reading» al corregir y después de guardar: limpia los campos y `replaces`, genera otra intención y conserva el bloqueo del envío completado.
Conservé el `DateTime` UTC exacto al cargar una corrección o una preparación, hasta que el usuario edite la hora; iniciar una lectura nueva también elimina ese instante conservado.
Amplié la integración con igualdad exacta de la hora al corregir y reabrir, una lectura independiente y dos puntos efectivos después de la reapertura.
Añadí seis regresiones entre controlador, widgets y golden; la pantalla inicial conserva su diseño y la referencia PNG existente.

**N1, causa y transición.** Antes solo se asignaba `_replaces` al corregir y no había ninguna transición para limpiarlo. `startNewReading()` pasa ahora por el mismo inicio de edición que una corrección, con destino ausente, campos vacíos y clave nueva. El botón solo aparece si hay destino de corrección o si la fase es `saved`; no aparece en el estado inicial. `canSubmit` conserva la condición que bloquea `saved`. La nueva transición se deshabilita durante confirmación, recuperación o envío, y también cuando queda una preparación pendiente por un fallo transitorio, para no descartar una intención cuyo resultado se desconoce.

**N2, causa y precisión.** El texto del campo representa minutos, por lo que no debe ser la fuente de la hora de un registro que el usuario no ha editado. `_preservedMeasuredAtUtc` conserva el instante tipado recibido en el punto o la preparación. El envío usa ese instante antes de interpretar el campo. Una edición real del texto elimina la conservación: una hora válida la sustituye, una inválida se rechaza y un campo vacío vuelve a pedir a Rust la hora actual. La lectura nueva también elimina la hora conservada. El caso de regresión usa exactamente `2026-11-18T12:34:56.789Z`.

| Hallazgo | Archivo:línea | Prueba nueva o ampliada |
|---|---|---|
| N1: salir de corrección, crear otra intención y conservar el bloqueo tras éxito. | [pulse_controller.dart:88](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:88) · [pulse_controller.dart:156](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:156) · [pulse_screen.dart:400](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_screen.dart:400) · [app_en.arb:16](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/l10n/app_en.arb:16) | [pulse_review_test.dart:319](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:319): guardar → corregir → guardar → nueva lectura → guardar; `replaces == null`, raíz nueva, dos puntos efectivos y botón Guardar deshabilitado tras cada éxito. [pulse_controller_test.dart:161](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:161): la nueva transición no descarta un envío pendiente y su reintento conserva el DTO. [pulse_flow_test.dart:246](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:246): mismo recorrido con Rust y dos raíces tras reabrir. |
| N1: acción visible y localizada, sin cambiar el estado inicial. | [pulse_screen.dart:403](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_screen.dart:403) | [pulse_initial_test.dart:69](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/goldens/pulse_initial_test.dart:69): nueva golden posterior a la corrección, con fuentes locales y reloj fijo. La golden inicial exige que el botón no exista. |
| N2: preservar UTC con segundos y milisegundos hasta editar la hora. | [pulse_controller.dart:177](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:177) · [pulse_controller.dart:196](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:196) · [pulse_controller.dart:224](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:224) · [pulse_controller.dart:268](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:268) | [pulse_controller_test.dart:193](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:193): igualdad exacta en entrada, recibo e historial. [pulse_controller_test.dart:209](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:209) y [pulse_controller_test.dart:223](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:223): hora editada, inválida y vacía. [pulse_review_test.dart:319](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:319): precisión conservada al editar solo bpm y hora actual para lectura nueva. [pulse_flow_test.dart:243](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:243) y [pulse_flow_test.dart:294](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:294): igualdad exacta antes y después de reabrir. |

### 2. Archivos tocados

- `app/lib/features/pulse/pulse_controller.dart`.
- `app/lib/features/pulse/pulse_screen.dart`.
- `app/lib/l10n/app_en.arb`.
- `app/test/features/pulse_controller_test.dart`.
- `app/test/widgets/pulse_review_test.dart`.
- `app/test/goldens/pulse_initial_test.dart`.
- `app/integration_test/pulse_flow_test.dart`.
- `reviews/etapa2-puente-sol.md`.

No cambié Rust, FRB, dependencias ni el helper de teclado de Ronda 5. Conservé la corrección del usuario en el bucle `while (true)` con comprobación de `mounted`, el fake corregido por el usuario, los generados y la PNG inicial.

### 3. Comprobaciones y comandos

**Resultados aportados por el usuario para Ronda 5:** analyze limpio, Flutter 33/33 e integración offline 2/2 en cada AVD API 26 y API 37. Son resultados anteriores a estos cambios. Leí íntegra `reviews/etapa2-revision-astra-r2.md`; esta ronda atiende sus dos hallazgos N1 y N2.

Ejecuté lecturas con `node:fs/promises` y comprobaciones estructurales con `node:assert/strict`: texto del ARB, vínculo de la acción con la transición, bloqueo de envío completado y preparación pendiente, conservación e invalidación del instante tipado, aserciones de hora y dos raíces en integración y seis tests adicionales. Esas comprobaciones pasaron. Los hashes de la PNG inicial, el helper de teclado, el fake corregido, el adaptador y los generados coinciden con la lectura inicial de esta ronda.

**NO EJECUTADO** Dart, Flutter, Cargo, generadores, builds, emuladores y generación de la nueva PNG por la instrucción de no ejecutar procesos. Los seis tests nuevos no se presentan como aprobados. Si pasan todos, la suite completa tendrá 39 tests Flutter.

#### Secuencia exacta en PowerShell 7

**Hay que ejecutar `flutter gen-l10n`** antes del análisis y las pruebas para generar `pulseNewReadingAction`. No hace falta FRB codegen ni build_runner: no cambiaron sus entradas.

**La pantalla inicial no cambia:** conservar `baselines/pulse_initial.png` y comprobarla. Generar solamente la nueva referencia `app/test/goldens/baselines/pulse_corrected.png`, que muestra el formulario con la corrección guardada y la acción «New reading», a 390 × 1400 con las fuentes locales.

```powershell
Set-Location 'C:\Users\Salva\Desktop\git\nasa.spaceapps'
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$env:RUSTUP_TOOLCHAIN = '1.96.0'
$env:ANDROID_HOME = 'C:\Users\Salva\AppData\Local\Android\Sdk'
$env:JAVA_HOME = 'C:\Program Files\Android\Android Studio\jbr'
$env:Path = "C:\dev\flutter\bin;C:\Users\Salva\.cargo\bin;$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:Path"

Push-Location app
flutter gen-l10n
dart format lib test integration_test
flutter analyze --fatal-infos
dart format --output=none --set-exit-if-changed lib test integration_test
flutter test test/features/pulse_controller_test.dart test/widgets/pulse_review_test.dart --reporter expanded
flutter test --update-goldens test/goldens/pulse_initial_test.dart --plain-name 'saved correction offers a new reading with bundled fonts'
flutter test test/goldens/pulse_initial_test.dart
flutter test
flutter build apk --debug
```

Revisar visualmente la nueva PNG y conservarla junto con la entrega. Después, todavía desde `app/`, ejecutar ambos recorridos offline en cada AVD:

```powershell
$pulsysAdb = Join-Path $env:ANDROID_HOME 'platform-tools\adb.exe'
$pulsysEmulator = Join-Path $env:ANDROID_HOME 'emulator\emulator.exe'
& $pulsysAdb start-server

function Invoke-PulsysRound6Integration {
    param([string] $Avd, [int] $Port)

    $pulsysSerial = $null
    foreach ($pulsysDeviceLine in (& $pulsysAdb devices)) {
        if ($pulsysDeviceLine -match '^(emulator-\d+)\s+device$') {
            $pulsysCandidate = $Matches[1]
            $pulsysAvdName = (& $pulsysAdb -s $pulsysCandidate emu avd name |
                Select-Object -First 1).Trim()
            if ($pulsysAvdName -eq $Avd) {
                $pulsysSerial = $pulsysCandidate
                break
            }
        }
    }

    if ($null -eq $pulsysSerial) {
        $pulsysSerial = "emulator-$Port"
        Start-Process -FilePath $pulsysEmulator -ArgumentList @(
            '-avd', $Avd, '-port', $Port, '-no-window', '-no-snapshot-save'
        ) -WindowStyle Hidden
    }

    $pulsysDeadline = (Get-Date).AddMinutes(4)
    do {
        Start-Sleep -Seconds 2
        if ((Get-Date) -gt $pulsysDeadline) {
            throw "$Avd did not boot"
        }
        $pulsysConnected = (& $pulsysAdb devices) -match "^$pulsysSerial\s+device$"
    } until ($pulsysConnected -and
        (& $pulsysAdb -s $pulsysSerial shell getprop sys.boot_completed).Trim() -eq '1')

    & $pulsysAdb -s $pulsysSerial shell getprop ro.build.version.sdk
    & $pulsysAdb -s $pulsysSerial shell getprop ro.product.cpu.abi
    $pulsysWifi = (& $pulsysAdb -s $pulsysSerial shell settings get global wifi_on).Trim()
    $pulsysData = (& $pulsysAdb -s $pulsysSerial shell settings get global mobile_data).Trim()
    try {
        & $pulsysAdb -s $pulsysSerial shell svc wifi disable
        & $pulsysAdb -s $pulsysSerial shell svc data disable
        flutter test integration_test/pulse_flow_test.dart -d $pulsysSerial --reporter expanded
    } finally {
        if ($pulsysWifi -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc wifi enable
        }
        if ($pulsysData -eq '1') {
            & $pulsysAdb -s $pulsysSerial shell svc data enable
        }
    }
}

Invoke-PulsysRound6Integration -Avd 'Pulsys_API26_lowram' -Port 5560
Invoke-PulsysRound6Integration -Avd 'Pixel_7_Pro' -Port 5562
Pop-Location

git diff --check
git status --short --untracked-files=all

```

### 4. Pendiente o en duda

- Ejecutar la secuencia y registrar resultados. Generar y revisar únicamente la nueva golden de corrección; comprobar que la referencia inicial aprobada sigue pasando.
- `gen-l10n` actualizará los Dart generados de localización. No los edité a mano; el getter nuevo solo existirá después de generar.
- La reapertura sigue siendo del adaptador en el mismo proceso. Continúan los pendientes aceptados de relanzamiento real, iOS, API 26 arm64 y resultados de enlaces opcionales. Este PC x86 no aporta ejecución API 26 arm64.
