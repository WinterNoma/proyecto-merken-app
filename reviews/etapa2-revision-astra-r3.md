**Dictamen: cerrar la etapa 2 en Android dentro del alcance aceptado.** N1 y N2 están resueltos en código y pruebas. La revisión acotada no detectó regresiones bloqueantes ni importantes.

## 1. Qué revisé

Leí `AGENTS.md` y la Ronda 6 completa.
Contrasté las transiciones del controlador con sus pruebas de widgets e integración.
Inspeccioné ambas goldens y la configuración de formato y regeneración de CI.

| Punto | Estado | Evidencia |
|---|---|---|
| **N1 — Salir de corrección e iniciar otra lectura** | **Cerrado** | `startNewReading()` limpia campos, hora conservada y `replaces`, y genera otra clave. Mantiene el bloqueo durante un envío pendiente y después del éxito. Las pruebas comprueban una raíz independiente y dos puntos efectivos tras reabrir. [pulse_controller.dart:156](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:156) · [pulse_controller_test.dart:161](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:161) · [pulse_review_test.dart:319](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/widgets/pulse_review_test.dart:319) · [pulse_flow_test.dart:246](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:246) |
| **N2 — Conservar segundos y milisegundos** | **Cerrado** | El instante UTC tipado tiene prioridad mientras la hora permanezca sin editar. Editarla invalida esa conservación. Hay pruebas de igualdad exacta, sustitución, entrada inválida, campo vacío y persistencia tras reapertura. [pulse_controller.dart:177](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:177) · [pulse_controller.dart:224](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:224) · [pulse_controller.dart:268](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/features/pulse/pulse_controller.dart:268) · [pulse_controller_test.dart:193](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/features/pulse_controller_test.dart:193) · [pulse_flow_test.dart:294](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/integration_test/pulse_flow_test.dart:294) |

**El cambio de `rustfmt::skip` es coherente con CI.** Está acotado al módulo generado y explica su propiedad en la misma línea. CI fija FRB 2.11.1, regenera y comprueba diferencias y archivos nuevos, incluido `frb_generated.rs`. `add_mod_to_lib: false` conserva la declaración manual con sus atributos. Clippy y los tests siguen comprobando ese módulo. [lib.rs:11](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/bridge/src/lib.rs:11) · [flutter-ci.yml:56](C:/Users/Salva/Desktop/git/nasa.spaceapps/.github/workflows/flutter-ci.yml:56) · [flutter-ci.yml:64](C:/Users/Salva/Desktop/git/nasa.spaceapps/.github/workflows/flutter-ci.yml:64) · [flutter_rust_bridge.yaml:5](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/flutter_rust_bridge.yaml:5)

**Ambas goldens quedan visualmente aprobadas.** Tienen texto legible, controles completos y ausencia de franja DEBUG. La corregida muestra Guardar deshabilitado, «New reading», recibo e historial corregido. [pulse_initial.png](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/goldens/baselines/pulse_initial.png) · [pulse_corrected.png](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/goldens/baselines/pulse_corrected.png)

La observación sobre **«Current pulse status — No data»** corresponde al fixture fijo del fake. La golden corregida acredita la composición visual con ese fixture. El adaptador de producción obtiene la evaluación de Rust. Lo considero una limitación de representatividad de esa referencia, sin gravedad bloqueante o importante. [fake_pulsys_api.dart:350](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/test/fakes/fake_pulsys_api.dart:350) · [rust_adapter.dart:234](C:/Users/Salva/Desktop/git/nasa.spaceapps/app/lib/platform/rust_adapter.dart:234)

## 2. Archivos modificados

Ninguno.

## 3. Comprobaciones y resultados

Ejecuté lecturas con `Get-Content`, `rg` y `node:fs`, inspección visual de las dos PNG y `git diff --check`. Este último terminó con código 0 y una advertencia LF/CRLF en `Cargo.lock`.

**Resultados aportados por ti sobre el estado actual.** Análisis, formato y Clippy limpios. Flutter 39/39, integración offline 2/2 en cada AVD y Rust core 86, store 70, API 9+1 y bridge 2+4 aprobados.

**NO EJECUTADO** por esta revisora — suites, generadores, builds y emuladores. El dictamen combina la inspección actual con tus resultados ejecutados.

## 4. Pendiente

Se mantienen las reservas aceptadas de iOS, API 26 arm64, relanzamiento real del proceso y enlaces opcionales, documentadas en la Ronda 6. [etapa2-puente-sol.md:776](C:/Users/Salva/Desktop/git/nasa.spaceapps/reviews/etapa2-puente-sol.md:776)