# Compilar y descargar el APK

El workflow **Android APK** ejecuta pruebas Rust, construye Android ARM64 con
Tauri 2 y exporta un APK debug firmado, metadatos y SHA-256 como artefacto Actions.
Se ejecuta en PR hacia `desarrollo`, pushes a `desarrollo` y manualmente.
No despliega servicios ni necesita credenciales AWS, SSH o datos personales.

En GitHub: Actions → Android APK → ejecución correcta → Artifacts. Descargar y
extraer `merken-arm64-debug.apk`. En Windows con GitHub CLI:

```powershell
gh run download NUMERO_EJECUCION --repo WinterNoma/proyecto-merken-app --dir artifacts
```

Es un APK de desarrollo para pruebas, no una publicación de Play Store. La clave
debug del runner es temporal; las compilaciones pueden necesitar desinstalar la
versión anterior si cambia la firma. Desinstalar borra los datos locales: exportar
los registros antes de hacerlo. Para actualizaciones estables se necesita una
clave de firma persistente y privada, que este workflow no crea ni publica.

Los modelos GGUF/voz no se incluyen dentro del APK. Sus funciones requieren la
provisión y verificación de modelos por separado según los requisitos del proyecto.
La compilación no demuestra funcionamiento clínico ni rendimiento en un teléfono;
hay que probar arranque, SQLite, permisos y funcionamiento offline en un Android.

Herramientas: Node 24, Java 17, Rust estable, Android API/build-tools 36,
NDK 28.2.13676358. Las dependencias de la app usan package-lock.json y Cargo.lock.

Referencia: [CLI Android Tauri](https://v2.tauri.app/reference/cli/).
