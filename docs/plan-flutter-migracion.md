# Plan: probar Flutter + núcleo Rust (rama `flutter-migration`)

Estado: **v2 del 18-nov-2026**, con la revisión de Astra ([reviews/plan-flutter-astra.md](../reviews/plan-flutter-astra.md)) y la verificación de egui ([reviews/verificacion-egui-movil.md](../reviews/verificacion-egui-movil.md)). Investigación de base: [alternativas-tauri](../reviews/alternativas-tauri.md) y [frontend-rust-frameworks](../reviews/frontend-rust-frameworks.md). Es un **experimento con puerta de salida** (etapa 3). Tauri sigue compilable en `desarrollo` hasta entonces.

## Objetivo y criterio de decisión

Una sola UI en Flutter para Android e iOS, sin depender del WebView del sistema, con el núcleo Rust actual conectado por `flutter_rust_bridge` (FRB).

**Por qué Flutter:** retrocompatibilidad. En Android viejo el WebView queda congelado (Chrome 138 en Android 8 y 9) y Flutter no lo usa. El piso de SO casi no cambia: Flutter exige Android 7 (API 24) e iOS 15. El rendimiento mejor es probable, pero **no está medido**.

**Puerta de decisión (etapa 3):** seguimos si el flujo vertical funciona en los teléfonos más viejos que queremos soportar. No hace falta ganar a Tauri en cifras. Condiciones:
1. Guardar, corregir y recuperar un pulso sin red, en instalación limpia y actualizada, sin duplicar datos.
2. Completar el registro y elegir una zona corporal con TalkBack y VoiceOver, con teclado en español (acentos, pegado, texto grande).
3. Funciona en Android API 26 arm64 de poca RAM e iPhone con iOS 15.
4. Cero cierres inesperados.

El rendimiento se mide igualmente (arranque, memoria, p95 de fotogramas, tamaño) como información. Si hay un APK de Tauri de referencia, se compara; no es obligatorio.

**Salidas posibles:** seguir, seguir solo en el alcance comprobado, o archivar la rama con el informe.

## Qué se reutiliza y qué se rehace

| Pieza | Líneas aprox. | Destino |
|---|---|---|
| `crates/core`, `store`, `slm`, `voice`, `sync`, `seed` | 13 400 / 6 700 / 12 500 / 1 650 / 440 / 230 | Se quedan |
| `src-tauri/src` (32 comandos registrados) | 2 800 | Se desarma por recorridos: la lógica pasa a crates, el pegamento Tauri se descarta al final |
| `ui/` (TypeScript) | 5 800 | Se rehace en Dart; sirve de especificación |
| `crates/local-speech` (plugin Kotlin, permisos, eventos, pausa) | 75 Rust + Kotlin | Se porta a plugin de Flutter (Kotlin + Swift). Es más trabajo que lo que sugieren sus líneas |
| Brandbook, capturas (`docs/referencias`), geometría del cuerpo | — | Se reutilizan |

## Estructura de carpetas

```text
app/
  lib/features/pulse/       pantalla, estado de edición y presentación
  lib/platform/             adaptador Dart; bindings FRB generados, separados
  android/  ios/            permisos, ciclo de vida, notificaciones, captura
crates/
  bridge/                   funciones FRB pequeñas y código generado (aquí vive el `unsafe`)
  api/                      fachada: lib.rs y records.rs al empezar
  core/ store/ seed/        reglas, persistencia, escenarios (existentes)
  sync/ slm/ voice/         adaptadores y motores, se incorporan por recorrido
  local-speech/             plugin del host Tauri mientras siga activo
src-tauri/ ui/ plugins/     host y referencia de comportamiento actuales
```

- Dependencias: Flutter → `bridge` → `api` → `store`/`core`. Tauri usa la misma `api`.
- Para quien empieza en Rust: funciones y una implementación concreta de SQLite. Nada de un trait por operación, un crate por pantalla ni copias manuales del catálogo clínico. Crear módulos según se usen.
- `unsafe_code = "forbid"` se hereda por paquete: el crate `bridge` queda fuera de esa regla, aislado y mínimo.
- Fijar versiones compatibles de generador FRB, crate Rust y paquete Dart; comprobar en CI que los bindings regeneran sin diferencias.
- El puente devuelve DTO pequeños, UTC y errores con código y campo. Una conexión de larga vida, con operaciones fuera del hilo de UI. Las llamadas de red e inferencia sueltan la base mientras esperan.

## Etapas

### Etapa 0 — Fijar la referencia (en `desarrollo`, antes de ramificar)
- Registrar versión del APK instalado, identidad y firma, versiones de SDK, NDK y Rust, datos sintéticos y los equipos disponibles.
- Opcional: generar un Tauri **release** arm64 con funciones equivalentes a las del piloto. Hoy el CI genera debug con STT y el empaquetado de modelos depende del evento.
- Rotar la contraseña de MySQL (la credencial estuvo en GitHub) con quien la administra; registrar el estado sin exponer secretos.
- Instalar Flutter, `cargo-ndk`, `flutter_rust_bridge_codegen`; Xcode en el Mac.
- Crear la rama `flutter-migration` y la decisión `docs/decisiones/004-ui-flutter.md` como **experimento**.
- **Listo cuando:** `flutter doctor` sin errores en Windows y en el Mac, copia SQLite consistente de prueba, un iPhone con iOS 15 reservado, rama creada.

### Etapa 1 — Un solo caso de uso en Rust
- Extraer solo: abrir el almacén, registrar y confirmar un pulso, consultar su historial efectivo y leer el estado de zonas. Tauri usa esas mismas funciones.
- Reloj inyectable. Distinguir dato ausente, inválido, antiguo y aviso que pide confirmación.
- **Arreglar antes del piloto el guardado con resultado ambiguo** (problema 1 de la tabla).
- Dejar HTTP, exportación, SLM y los demás comandos para sus etapas.
- **Listo cuando:** con reloj y SQLite sintéticos, Tauri y la fachada dan la misma lectura; un aviso sin confirmar produce cero escrituras; una corrección conserva su original; repetir un guardado cuyo resultado se perdió no duplica el registro. `cargo test` pasa.
- Modelo: Sol `max`; revisión Astra `high`.

### Etapa 2 — Cruzar el puente en ambos SO
- Fijar primero las firmas y un único responsable de `Cargo.toml`, `Cargo.lock`, configuración FRB y bindings. Después se pueden hacer en paralelo el widget sobre datos de muestra y el adaptador FRB.
- Enlazar Rust y SQLite en Android arm64 y x86_64 y en iOS arm64 (dispositivo y simulador). Probar por separado el enlace de llama.cpp y sherpa-onnx en ambos.
- CI reproducible de ambos hosts.
- **Listo cuando:** un checkout limpio compila y regenera bindings sin diferencias; la app instalada llama a Rust, escribe, cierra y relee sin red en Android e iOS; el enlace de motores tiene resultado por ABI y arquitectura de Apple.
- Modelo: Sol `max`.

### Etapa 3 — Pantalla vertical y puerta de decisión
- Registro → tendencia → zona corporal, con tokens del brandbook. Validar la actualización sobre una instalación existente (continuidad de datos: base v5 con correcciones, pendientes, cursor y cadena de telemetría; conteos y contenido iguales; esquema futuro detectado).
- Pasar la lista de condiciones de la puerta, con TalkBack y VoiceOver.
- Medir como información (10 arranques por variante, alternando; RAM total incluidos procesos auxiliares; p95 de fotogramas; tamaño descargado e instalado). Usar series de 30 y 2 000 datos de origen con el mismo número de puntos dibujados.
- Registrar en la ADR: seguir, seguir con alcance limitado, o archivar.
- Modelo: Sol `max` para código; Astra `high` para lo visual; Claude revisa la figura corporal.

### Etapa 4 — Portar por recorridos completos
- Extraer componentes al aparecer su segundo uso. Orden: registro, ayuda urgente, estado, correcciones, historial; después tripulación, eventos, informe y exportación, sincronización.
- Conservar separados el perfil local, la DEMO y el replay de Tierra. Extraer cada caso de uso junto con su prueba de paridad.
- Cada recorrido: modo avión, persistencia tras reinicio, textos en inglés, lector, escala de texto y manejo de errores.
- Resolver aquí los problemas 2 a 4 y 8 a 10 de la tabla al pasar por cada flujo.

### Etapa 5 — Servicios de plataforma
- Android e iOS juntos: permisos, captura de audio, interrupciones, voz offline y recordatorios locales. Preparación de modelos, tareas cancelables y liberación de memoria. Asistente sobre la misma API.
- Resolver los problemas 5 a 7 de la tabla.
- **Listo cuando:** en modo avión, tras preparar modelos, funcionan las capacidades anunciadas. Con permiso denegado, modelo ausente o poco espacio se puede registrar a mano. El aviso llega con la app cerrada.

### Etapa 6 — Certificar y entregar
- Repetir la matriz con Android antiguo y actual, iOS 15 y actual. armv7 físico solo si se distribuirá.
- Inspeccionar minSdk, ABI, bibliotecas y alineación de 16 KB en el artefacto final. Firmar APK y build iOS; probar actualización, restauración y distribución beta (TestFlight; instalación directa si el cliente exige una versión más nueva de iOS).
- Actualizar README y ADR; archivar el host anterior con una etiqueta reproducible.

## Problemas del código que se corrigen de paso

| # | Problema | Cuándo |
|---|---|---|
| 1 | **Guardado con resultado ambiguo:** se confirma el registro y la bandeja de salida antes de armar la respuesta; si falla y se reintenta, puede duplicarse (`crates/store/src/api.rs:92–101`, `records.rs:293–357`). Dar a cada acción una clave estable resuelta dentro de la transacción. | Antes del piloto |
| 2 | **Reglas copiadas en la vista:** rangos comparados en TS (`ui/src/vital-status.ts`, `status-copy.ts`, `vitals.ts`) además de Rust (`crates/core/src/rules.rs`). Devolver la clasificación desde Rust. | Antes de fijar el contrato |
| 3 | **Historial completo bajo candado:** `list_records` carga, clona y ordena antes del `limit` (`records.rs:327–350,428–481`). Consulta de tendencia acotada. | Antes de medir |
| 4 | **Motores ligados al control:** el host activa `merken-slm/native` y STT en CI (`src-tauri/Cargo.toml`, `crates/slm/Cargo.toml`); definir perfil básico y perfil equipado y verificar con `cargo tree -e features`. | Antes de comparar tamaños |
| 5 | **Audio copiado y preparación acoplada al GGUF:** PCM a Base64 y el preparador exige el GGUF aunque solo se use Whisper (`ui/src/core-api.ts`, `voice.rs`, `SlmAssetsPlugin.kt`). Búfer binario acotado y paquetes por capacidad. | Etapa 5 |
| 6 | **Falta procedencia de la interpretación:** `interpreted_by: None` fijo (`store/src/api.rs`). Propagar motor y versión. | Etapa 5 |
| 7 | **Sync con Tierra incompleto:** `sync` solo implementa el cliente de simulación. Distinguir snapshot recibido, pendiente, aceptado y revisado. Contrato de push/pull con el responsable de Tierra. | Etapa 4 |
| 8 | **Límite de memoria tardío en el roster:** se lee entero antes del límite de 2 MiB (`recommendations.rs`). Acotar durante la lectura. | Etapa 4 |
| 9 | **Exportación duplica el documento en memoria** (`store/src/export.rs`, `lib.rs`). Escribir de forma incremental a un temporal privado. | Etapa 4 |
| 10 | **Reemplazo no atómico de la DEMO** (`records.rs`). Usar una sola transacción, como `import_synthetic`. | Etapa 4 |
| 11 | **HTTP duplicado:** `sync` usa `ureq` sin TLS y el host `reqwest` con rustls. Dejar uno con rustls y probar HTTPS real. | Etapa 4 |
| 12 | **Piso de Android con dos valores** (24 de Tauri frente a 26 del proyecto). Fijar uno con el manifiesto fusionado. | Etapa 0 |
| 13 | **Nombre inconsistente** (Merken / Pulsys). Cambiar solo el nombre visible; los nombres de crates `merken-*` se aplazan. | Después de la puerta |
| 14 | **README y decisiones desactualizados** (siguen diciendo Tauri). | Etapa 6 |
| 15 | **Pruebas visuales** sueltas en `.mjs`: pasar a tests de widgets y capturas de referencia (golden) en Flutter. | Etapa 4 |

Higiene del repo (zips y modelos pesados en la raíz): ya están en `.gitignore`; mover fuera de la carpeta no está en el camino crítico. **No borrar `server/`**: Vite y el README documentan allí la configuración de desarrollo.

## Decisiones tomadas

1. **Nombre e identidad:** visible **Pulsys**. Conservar `com.merken.app` en Android si debe actualizar las instalaciones actuales; usar una identidad de ensayo distinta para las pruebas lado a lado. Confirmar la custodia de la firma y la disponibilidad del bundle ID en iOS.
2. **Modelos:** «preparar para la misión». App base con registro manual y reglas deterministas offline; los modelos (~1,25 GiB) se descargan o copian **antes** de partir, con verificación de tamaño y hash, activación atómica y versión anterior conservada. El APK con modelos es una variante de pruebas.
3. **Pisos:** Android **API 26 arm64** como referencia, **API 24** como ampliación para el registro básico; **iOS 15 arm64** desde el primer corte. API 23 queda fuera de Flutter: si fuera requisito, reevaluar Compose + SwiftUI antes de portar pantallas. armv7 solo con instalación de 32 bits probada. x86_64 para CI y emulador.
4. **Tauri:** se mantiene compilable hasta la puerta. Si gana Flutter, se congelan las funciones visuales nuevas en Tauri y solo se aceptan correcciones críticas.
5. **iOS:** reservar ahora un Mac con Xcode, firma de desarrollo y un iPhone físico con iOS 15. Sin hardware, el juicio de iOS queda provisional y avanza solo Android.

## Alternativas descartadas o en reserva

- **egui/eframe: descartado.** Verificado el 18-11-2026 (egui 0.37.0): en Android, `NativeActivity` no se combina con la accesibilidad y `GameActivity` no entrega el texto compuesto a egui (acentos y teclados con candidatos); la ruta de TalkBack necesita API 26 o más; en iOS falta la composición de texto. Para escribir en español con retrocompatibilidad es el punto más débil. Se reabre solo si winit entrega la composición de texto (propuestas de noviembre de 2026 aún abiertas) y alguien comprueba el recorrido accesible en teléfono.
- **Slint: reserva** si la UI en Rust se volviera obligatoria. Ensayo mínimo: teclado, ambos lectores y toque dentro del contorno real.
- **Dioxus:** descartado (WebView o alfa sin paridad verificada).
- **Compose + SwiftUI + UniFFI:** reserva si API 23 fuera requisito, o si el teclado o la accesibilidad impidieran una UI compartida.

## Reparto de trabajo (según CLAUDE.md)

- **Sol 6.2 `max`:** código Rust y Dart, puente, voz.
- **Astra 6.2 `high`:** revisiones y trabajo visual.
- **Claude:** orquestación, figura corporal y revisión visual clave.
- **Sol 6.2 `high`:** investigaciones y verificaciones.
- Hydrogen: sin cambios; el contenido médico no se toca en esta migración.
- Dos agentes nunca escriben los mismos archivos. Las etapas 1 (`crates/`, `src-tauri/`) y 2 (`app/`, `crates/bridge`) pueden ir en paralelo una vez fijadas las firmas.

## Riesgos conocidos (sin medir)

- Rendimiento real de Flutter frente a Tauri en Pulsys.
- Accesibilidad de la figura corporal dibujada a mano en TalkBack y VoiceOver.
- Compilar llama.cpp y sherpa-onnx en iOS y armv7 con las versiones fijadas.
- Memoria con el SLM y la voz cargados en teléfonos viejos.
- Estado real de las instalaciones que hay que conservar y de la rotación de la credencial: no verificados.
