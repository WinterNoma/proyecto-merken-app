# Etapa 3 · Tendencia del pulso

## 1. Qué hice

- Implementé `HistoryScreen` sobre `PulsysApi.effectivePulseHistory`, con carga, estado vacío, error y reintento.
- La gráfica usa la hora de medición UTC y los bpm registrados. La tabla muestra las mismas lecturas con encabezados y celdas semánticas, marca DEMO y texto adaptable a 2×.
- Amplié a 2 000 el máximo de 500 que fija `docs/estructura-flutter.md` §3 para la consulta efectiva de pulso. La tarea necesita la serie completa en una sola lectura. Hasta 30 se muestran todos los puntos. Por encima de 30 se conservan los extremos temporales y los mínimos y máximos de 14 grupos consecutivos.
- Conecté Pulse → History → Body y Pulse → Body con `MaterialPageRoute<void>` y la misma sesión. Añadí ocho casos de widget, un golden y la comprobación del registro nuevo en la tendencia del recorrido con Rust.

El límite por omisión de la consulta es 100. La escala se calcula con las lecturas y la gráfica utiliza el acento neutro del tema.

## 2. Archivos tocados

- `app/lib/features/history/history_screen.dart`, `history_controller.dart`, `pulse_trend.dart` y `pulse_trend_painter.dart`.
- `app/lib/routing/app_router.dart`, `app/lib/app.dart` y `app/lib/features/pulse/pulse_screen.dart`.
- `app/lib/l10n/app_en.arb`.
- `crates/store/src/pulse_history.rs` y `crates/store/src/pulse_tests/history.rs`.
- `app/test/fakes/fake_pulsys_api.dart` y `app/test/widgets/history_screen_test.dart`.
- `app/test/goldens/history_screen_test.dart` e `app/integration_test/pulse_flow_test.dart`.
- `reviews/etapa3-tendencia-sol.md`.

## 3. Comprobaciones y comandos

Pasaron las comprobaciones en memoria con `node:fs/promises` y `node:assert/strict` sobre la sintaxis JSON y los placeholders del ARB, imports locales, constructor de `BodyScreen` y alcance de archivos. Un modelo JavaScript del muestreo pasó 44/44 combinaciones de tamaños, orden, extremos y empates, incluidas series de 30 y 2 000. La prueba de widget verifica el conteo real de llamadas de dibujo de la gráfica y la correspondencia de sus 30 puntos con las filas.

**NO EJECUTADO** `flutter gen-l10n`, formato y análisis Dart por la instrucción de trabajar sin ejecutar procesos.

**NO EJECUTADO** suites Flutter, generación y comparación de goldens e integración Android por la misma instrucción.

**NO EJECUTADO** formato, Clippy y pruebas Rust. La prueba ampliada del almacén solicita 2 001 puntos de una base con 2 004 y comprueba el límite de 2 000, su orden y el número de candidatos validados.

### Secuencia en PowerShell 7

```powershell
Set-Location 'C:\Users\Salva\Desktop\git\nasa.spaceapps'
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true
$env:RUSTUP_TOOLCHAIN = '1.96.0'
$env:ANDROID_HOME = 'C:\Users\Salva\AppData\Local\Android\Sdk'
$env:JAVA_HOME = 'C:\Program Files\Android\Android Studio\jbr'
$env:Path = "C:\dev\flutter\bin;C:\Users\Salva\.cargo\bin;$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:Path"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --locked -p pulsys-store -p pulsys-api -p pulsys-bridge

Push-Location app
flutter gen-l10n
dart format lib/features/history lib/routing/app_router.dart lib/app.dart lib/features/pulse/pulse_screen.dart test/fakes/fake_pulsys_api.dart test/widgets/history_screen_test.dart test/goldens/history_screen_test.dart integration_test/pulse_flow_test.dart
dart format --output=none --set-exit-if-changed .
dart analyze --fatal-infos
flutter test --exclude-tags golden --reporter expanded
flutter test --update-goldens test/goldens/history_screen_test.dart test/goldens/pulse_initial_test.dart
flutter test --tags golden test/goldens
```

La actualización genera `history_trend.png` y actualiza `pulse_initial.png` y `pulse_corrected.png` por los dos accesos nuevos. Revisar las tres PNG en `app/test/goldens/baselines/`.

Con ambos AVD iniciados y desde `app/`, usar los identificadores que muestre `flutter devices`.

```powershell
flutter devices
$pulsysApi26 = Read-Host 'ID de Pulsys_API26_lowram'
$pulsysActual = Read-Host 'ID de Pixel_7_Pro'
flutter test integration_test/pulse_flow_test.dart -d $pulsysApi26 --reporter expanded
flutter test integration_test/pulse_flow_test.dart -d $pulsysActual --reporter expanded
Pop-Location
```

## 4. Pendiente o en duda

- Generar los getters y las referencias golden con el catálogo inglés integrado. El constructor acordado de `BodyScreen` ya está presente. Al cierre de esta revisión, las cadenas `body*` del trabajo paralelo siguen pendientes de incorporarse a `app_en.arb`.
- Ejecutar las suites y revisar las imágenes, TalkBack y VoiceOver. La latencia de consulta, el tiempo de fotograma y el consumo de memoria en dispositivo quedan sin medir.

## Ronda 2

### 1. Qué hice

- Los accesos History y Body reciben el estado de edición del controlador y quedan deshabilitados durante recuperación, envío y confirmación. Se habilitan de nuevo al cancelar o finalizar.
- El contador de Pulso incluye solo zonas con estado distinto de `notEvaluated`, respetando el estado agregado de Rust.
- Añadí R y L a la figura frontal, con el lado derecho del astronauta a la izquierda de quien mira y etiquetas semánticas completas del ARB.
- El fake desempata por orden local de inserción descendente. El contrato documenta el límite de 2 000, con 100 por defecto. Añadí cinco casos de widget.

| Hallazgo | Archivo y línea | Prueba |
|---|---|---|
| 2. Evitar navegar mientras llega una confirmación de Pulso | [app/lib/features/pulse/pulse_screen.dart:219](../app/lib/features/pulse/pulse_screen.dart#L219) y [app/lib/routing/app_router.dart:23](../app/lib/routing/app_router.dart#L23) | [app/test/widgets/pulse_navigation_test.dart:14](../app/test/widgets/pulse_navigation_test.dart#L14) intenta abrir ambos destinos durante preparación y confirmación demoradas. Comprueba botones deshabilitados, diálogo en Pulso y recuperación de los accesos tras cancelar. |
| 3. Contar zonas realmente evaluadas | [app/lib/features/pulse/pulse_screen.dart:296](../app/lib/features/pulse/pulse_screen.dart#L296) | [app/test/widgets/pulse_navigation_test.dart:63](../app/test/widgets/pulse_navigation_test.dart#L63) cubre 14 zonas sin evaluar y una mezcla de Stable, Change, Alarm y NotEvaluated. Espera 0 y 3. |
| 4. Indicar lateralidad frontal | [app/lib/features/body/body_figure.dart:67](../app/lib/features/body/body_figure.dart#L67) | [app/test/widgets/body_screen_test.dart:103](../app/test/widgets/body_screen_test.dart#L103) verifica R/L, etiquetas del astronauta, posición respecto de cada brazo, visibilidad a 320 dp y texto 2×. |
| 5. Alinear el desempate del fake con local_order | [app/test/fakes/fake_pulsys_api.dart:331](../app/test/fakes/fake_pulsys_api.dart#L331) | [app/test/widgets/history_screen_test.dart:129](../app/test/widgets/history_screen_test.dart#L129) inserta z, a, m con la misma hora. Espera m, a al limitar a 2, z → a → m en la gráfica y m → a → z en la tabla. Ordenar por ID produce z, m y rompe la primera aserción. |
| 6. Documentar el máximo de 2 000 | [docs/estructura-flutter.md:115](../docs/estructura-flutter.md#L115) | Cotejo textual de la fila HistoryQuery. La única sustitución es 1–500 → 1–2 000. |

### 2. Archivos tocados

- `app/lib/features/pulse/pulse_screen.dart` y `app/lib/routing/app_router.dart`.
- `app/lib/features/body/body_figure.dart`, limitado a las dos etiquetas y su estilo tipográfico existente.
- `app/lib/l10n/app_en.arb`, con cuatro claves `navBody*` para R/L y sus etiquetas semánticas.
- `app/test/fakes/fake_pulsys_api.dart`, `app/test/widgets/pulse_navigation_test.dart`, `app/test/widgets/history_screen_test.dart` y `app/test/widgets/body_screen_test.dart`.
- `docs/estructura-flutter.md` y `reviews/etapa3-tendencia-sol.md`.

### 3. Comprobaciones y comandos

Leí íntegra `reviews/etapa3-revision-astra.md`. Pasaron las comprobaciones en memoria con `node:fs/promises` y `node:assert/strict` para el ARB y sus claves previas, bloqueo de ambos accesos, conteo por estado y límite escrito. El cotejo de orden con z/a/m distingue inserción de orden alfabético. Se conserva el inicializador `required this._api` del usuario y `semantics.dispose()` al final del test de tabla.

Los resultados aportados por el usuario corresponden a la entrega integrada anterior a esta ronda. Analyze limpio, 52/52 tests, 4/4 goldens, Rust verde e integración offline 2/2 en API 26 y 2/2 en API 37. También verificó 72/78/85 y el recorrido Pulso → History → Body en Pixel.

**NO EJECUTADO** generadores, formato, análisis, suites y comparación de goldens de esta ronda, manteniendo la instrucción de trabajar sin ejecutar procesos. Los cinco casos añadidos quedan pendientes de ejecución.

Comandos exactos desde PowerShell 7, con el entorno Flutter/Android/Rust ya configurado.

```powershell
Set-Location 'C:\Users\Salva\Desktop\git\nasa.spaceapps\app'
flutter gen-l10n
dart format lib/features/pulse/pulse_screen.dart lib/routing/app_router.dart lib/features/body/body_figure.dart test/fakes/fake_pulsys_api.dart test/widgets/pulse_navigation_test.dart test/widgets/history_screen_test.dart test/widgets/body_screen_test.dart
dart format --output=none --set-exit-if-changed .
dart analyze --fatal-infos
flutter test test/widgets/pulse_navigation_test.dart test/widgets/history_screen_test.dart test/widgets/body_screen_test.dart --reporter expanded
flutter test --exclude-tags golden --reporter expanded
flutter test --update-goldens test/goldens/pulse_initial_test.dart test/goldens/body_figure_test.dart
flutter test --tags golden test/goldens
```

Para la integración, desde `app/` y con los dos AVD iniciados.

```powershell
flutter devices
$pulsysApi26 = Read-Host 'ID de Pulsys_API26_lowram'
$pulsysApi37 = Read-Host 'ID del AVD API 37'
flutter test integration_test/pulse_flow_test.dart -d $pulsysApi26 --reporter expanded
flutter test integration_test/pulse_flow_test.dart -d $pulsysApi37 --reporter expanded
```

### 4. Pendiente o en duda

- Regenerar localización y ejecutar la secuencia anterior. La suite prevista pasa de 52 a 57 pruebas de widget/unidad, sujetas a ejecución.
- Regenerar y revisar `pulse_initial.png` y `pulse_corrected.png` porque el contador cambia de 14 a 0 en ambos fixtures. Regenerar y revisar `body_figure.png` por R/L. Las tres referencias están en `app/test/goldens/baselines/`.
- La comprobación manual de TalkBack queda a cargo del usuario según esta ronda. El cierre de la etapa conserva los pendientes de la revisión de Astra.

## Ronda 3

### 1. Qué hice

- Implementé el contenedor de tres páginas, con Today inicial, cabecera fija, botones inferiores accesibles, gestos y retorno al centro con Atrás.
- Reutilicé la tendencia y su tabla en History, con filtro UTC de 6 h, 24 h y 7 días aplicado antes del muestreo. Añadí perfil e indicadores en el orden solicitado.
- Extraje BodyPanel para colocar la figura y sus fichas entre las dos columnas de signos. La tarjeta de pulso abre el formulario existente. Al cerrarlo se refrescan las páginas.
- Recommendations presenta pulso y zonas de Rust, aviso a los médicos solo ante alarma, advertencia y sugerencia pendiente. Añadí pruebas del contenedor, tres goldens y adapté el recorrido con Rust.

| Requisito | Archivo y línea | Prueba |
|---|---|---|
| 1. PageView central, cabecera fija, indicadores y Atrás | [app/lib/routing/app_shell.dart:34](../app/lib/routing/app_shell.dart#L34) | [app/test/widgets/app_shell_test.dart:15](../app/test/widgets/app_shell_test.dart#L15) y [app/test/widgets/app_shell_test.dart:41](../app/test/widgets/app_shell_test.dart#L41). Selección, acción semántica, gestos, botones y Atrás. Cabecera y ayuda en [app/test/widgets/app_shell_test.dart:77](../app/test/widgets/app_shell_test.dart#L77). |
| 2. History en orden y filtro UTC antes del muestreo | [app/lib/routing/app_shell.dart:159](../app/lib/routing/app_shell.dart#L159) y [app/lib/features/history/history_controller.dart:101](../app/lib/features/history/history_controller.dart#L101) | [app/test/widgets/app_shell_test.dart:169](../app/test/widgets/app_shell_test.dart#L169) verifica alertas → perfil → gráfica → tabla → indicadores. [app/test/widgets/history_screen_test.dart:227](../app/test/widgets/history_screen_test.dart#L227) comprueba límites inclusivos, exclusión de futuros y selección sin otra consulta. Se conservan los casos de 30/2 000 puntos, vacío, error y tabla accesible. |
| 3. Today con figura, signos, contexto y asistente pendiente | [app/lib/features/overview/overview_screen.dart:33](../app/lib/features/overview/overview_screen.dart#L33) | [app/test/widgets/app_shell_test.dart:119](../app/test/widgets/app_shell_test.dart#L119) comprueba orden y lados, apertura del formulario y ficha de cuello. [app/test/widgets/app_shell_test.dart:306](../app/test/widgets/app_shell_test.dart#L306) cubre entrada deshabilitada y navegación a 320 dp y texto 2×. |
| 4. Recommendations con estados de Rust y aviso solo por alarma | [app/lib/features/assistant/recommendations_screen.dart:19](../app/lib/features/assistant/recommendations_screen.dart#L19) | [app/test/widgets/app_shell_test.dart:284](../app/test/widgets/app_shell_test.dart#L284) cubre pulso y zona con/sin alarma, incluso escalado urgente sin alarma. Comprueba la advertencia de orientación. |
| 5. Formulario reutilizado, refresco y recorrido instalado | [app/lib/features/pulse/pulse_screen.dart:156](../app/lib/features/pulse/pulse_screen.dart#L156) y app/lib/routing/app_router.dart | [app/test/widgets/app_shell_test.dart:199](../app/test/widgets/app_shell_test.dart#L199) confirma, corrige y encuentra el ganador en History. [app/test/widgets/pulse_navigation_test.dart:13](../app/test/widgets/pulse_navigation_test.dart#L13) intenta salir durante una confirmación demorada y permite volver tras un fallo de recuperación. [app/integration_test/pulse_flow_test.dart:156](../app/integration_test/pulse_flow_test.dart#L156) recorre Today → formulario → History → Recommendations → Today y conserva la reapertura. |
| 5. Una golden por página | [app/test/goldens/pages_test.dart:30](../app/test/goldens/pages_test.dart#L30) | Tres referencias nuevas con reloj fijo, fuentes locales, 560 × 1450, escala 1 y animaciones deshabilitadas. Las cuatro pruebas golden de componentes siguen incluidas. |
| 6. Contrato de navegación actualizado | [docs/estructura-flutter.md:85](../docs/estructura-flutter.md#L85) | Cotejo estructural de PageView con orden History → Today → Recommendations, inicio central, salida de formulario y refresco. |

### 2. Archivos tocados

- `app/lib/app.dart`, `routing/app_router.dart` y el nuevo `routing/app_shell.dart`.
- Nuevos componentes en `app/lib/features/overview/`: `overview_controller.dart`, `overview_screen.dart`, `status_summary.dart` y `vital_card.dart`.
- Nuevo `app/lib/features/assistant/recommendations_screen.dart`, limitado a la página de estado y placeholders.
- `app/lib/features/body/body_screen.dart`, `features/history/history_controller.dart`, `features/history/history_screen.dart` y `features/pulse/pulse_screen.dart` y `features/pulse/pulse_controller.dart`.
- `app/lib/l10n/app_en.arb`.
- `app/test/widgets/app_shell_test.dart`, `pulse_navigation_test.dart`, `pulse_screen_test.dart` y `history_screen_test.dart`, más `app/test/widget_test.dart`.
- `app/test/support/overview_fixture.dart` y `pulse_app.dart`.
- `app/test/goldens/pages_test.dart`, `pulse_initial_test.dart` y `history_screen_test.dart`.
- `app/integration_test/pulse_flow_test.dart`, `docs/estructura-flutter.md` y este informe.

Los nombres AppShell, OverviewScreen, BodyPanel y RecommendationsScreen concretan las responsabilidades de las carpetas de la estructura. La especificación fija los nombres de las tres páginas y sus posiciones.

### 3. Comprobaciones y comandos

Leí `docs/pantallas-flutter.md` y las secciones 1–3 de `reviews/inventario-ui-anterior.md`. Pasaron los cotejos en memoria con `node:fs/promises` y `node:assert/strict` de delimitadores Dart, imports locales, claves y placeholders del ARB, preservación de sus claves previas, orden de páginas y signos, condiciones de alarma, rango inclusivo y alcance de archivos. La revisión estructural acredita la retirada de las rutas apiladas de History y Body del router.

Los resultados aportados por el usuario para la Ronda 2 integrada son analyze limpio, 57/57 tests, 4/4 goldens e integración offline 2/2 en API 26 y 2/2 en API 37. Corresponden al estado anterior a esta reorganización.

**NO EJECUTADO** gen-l10n, formato, analyze, suites, goldens, builds e integración de esta ronda por la instrucción de trabajar sin ejecutar procesos.

Secuencia exacta en PowerShell 7.

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
dart format lib/app.dart lib/routing lib/features/overview lib/features/assistant/recommendations_screen.dart lib/features/body/body_screen.dart lib/features/history lib/features/pulse/pulse_screen.dart lib/features/pulse/pulse_controller.dart test integration_test
dart format --output=none --set-exit-if-changed .
dart analyze --fatal-infos
flutter test test/widgets/app_shell_test.dart test/widgets/history_screen_test.dart test/widgets/pulse_navigation_test.dart test/widgets/pulse_screen_test.dart test/widgets/body_screen_test.dart --reporter expanded
flutter test --exclude-tags golden --reporter expanded
flutter test --update-goldens test/goldens
flutter test --tags golden test/goldens --reporter expanded
flutter devices
$pulsysApi26 = Read-Host 'ID de Pulsys_API26_lowram iniciado'
$pulsysApi37 = Read-Host 'ID del AVD API 37 iniciado'
flutter test integration_test/pulse_flow_test.dart -d $pulsysApi26 --reporter expanded
flutter test integration_test/pulse_flow_test.dart -d $pulsysApi37 --reporter expanded
Pop-Location
```

Para repetir la condición sin red, mantener Wi-Fi y datos desactivados en los dos AVD durante esos comandos. La prueba conserva además el bloqueo de clientes HTTP dentro del proceso Dart.

### 4. Pendiente o en duda

- Ejecutar la secuencia y registrar los resultados. Generar y revisar `page_history.png`, `page_today.png` y `page_recommendations.png` en `app/test/goldens/baselines/`. Regenerar también las referencias de componentes `pulse_initial.png`, `pulse_corrected.png`, `history_trend.png` y `body_figure.png`, afectadas por la composición y retirada de accesos anteriores.
- El DTO actual expone estados de pulso y zonas. La línea de estado presenta la evaluación de pulso, el recuento de zonas evaluadas y la hora UTC. El resumen añade el escalado de pulso. El perfil se identifica como Local profile y la sincronización queda como placeholder.
- Para conservar la figura y sus objetivos táctiles, el bloque de tres columnas dispone de desplazamiento horizontal cuando supera el ancho disponible. Las goldens de página usan 560 dp para mostrar ambas columnas completas. Las pruebas incluyen navegación y texto ampliado a 320 dp.
- Need help muestra el aviso de etapa 4. El selector local, otros signos, contexto, Explain, sugerencia, micrófono y entrada del asistente quedan como placeholders solicitados.

## Ronda 4

### 1. Qué hice

- Reproduje el fallo en API 26 con sondas de geometría, ruta, scroll e insets. El teclado nativo aparecía después de que el test hubiera interpretado un inset cero como cierre completado.
- `_enterBpm` espera ahora el inset positivo de apertura del teclado. Después, `_tapVisible` quita el foco, espera su cierre y desplaza el botón. La espera depende de esos estados dentro del plazo existente.
- Añadí al fallo de tap el rectángulo del destino, viewport, actividad de scroll, ruta actual y recorrido del hit test, con capturas anteriores y posteriores a `ensureVisible`.
- Completé cinco ejecuciones consecutivas sin red en API 26 y dos en API 37, además de análisis, formato, 70 tests y siete goldens.

**Causa observada.** En la reproducción, Guardar estaba habilitado en `/pulse`, con `isCurrent=true` y scroll en `IdleScrollActivity`. Antes y justo después de `ensureVisible`, el inset era 0, el viewport medía 603,43 dp y el botón ocupaba `Rect.fromLTRB(32.0, 442.4, 379.4, 492.4)`. Al terminar `pumpAndSettle`, llegó la apertura pendiente del IME con 625 px físicos de inset. El viewport se redujo a 365,33 dp y el centro del botón quedó fuera de su área recortada. El hit test alcanzaba el Scaffold. Los campos ya habían perdido el foco. El cero inicial correspondía a un teclado cuya apertura todavía estaba pendiente. `pumpAndSettle` atiende los frames de Flutter y esa solicitud nativa podía llegar después.

### 2. Archivos tocados

- [app/integration_test/pulse_flow_test.dart:384](../app/integration_test/pulse_flow_test.dart#L384), sincronización de entrada y diagnósticos del tap.
- `reviews/etapa3-tendencia-sol.md`, esta sección.

### 3. Comandos y resultados reales

Entorno comprobado con `flutter --version`, `adb devices -l`, `wm size`, `wm density` y `getprop`. Flutter 3.50.0 y Dart 3.14.0. `emulator-5560` usa API 26 x86_64, 1080 × 1920 y densidad 420. `emulator-5562` usa API 37 x86_64.

| Comando desde `app/` | Resultado |
|---|---|
| `flutter test integration_test -d emulator-5560 --reporter expanded`, reproducción con sondas | Falló el segundo test. Capturé la transición de inset 0 → 625 y viewport 603,43 → 365,33 dp. |
| `flutter test integration_test -d emulator-5560 --reporter expanded`, después del cambio y con desconexión verificada | Cinco ejecuciones consecutivas aprobadas, 2/2 en cada una. |
| `flutter test integration_test -d emulator-5562 --reporter expanded` | Dos ejecuciones consecutivas aprobadas sin red, 2/2 en cada una. |
| `flutter analyze --fatal-infos` | Código 0. Sin incidencias. |
| `dart format --output=none --set-exit-if-changed lib test integration_test` | Código 0. 60 archivos, 0 cambios. |
| `flutter test --exclude-tags golden --reporter expanded` | Código 0. 70/70 aprobados. |
| `flutter test --tags golden --reporter expanded` | Código 0. 7/7 aprobados. |
| `dart format --output=none --set-exit-if-changed .`, tras las suites | Código 0. 60 archivos, 0 cambios. |

La primera comprobación de formato sobre `.` durante la integración encontró el listener temporal `build/test/listener_0.dart` generado por Flutter. Al terminar las suites, `dart format build/test` encontró cero archivos y la comprobación completa pasó.

**Desconexión efectiva.** En API 26, `svc wifi disable` desde el usuario shell dejaba Wi-Fi conectado. `su 0 svc wifi disable` en la imagen userdebug aplicó la desactivación. Verifiqué en ambos AVD `Wi-Fi is disabled`, `Active default network: none` y `mobile_data=0`. La serie de aceptación offline empieza después de esa verificación. Hubo además dos ensayos aprobados de API 26 antes de establecer ese estado. El test conserva su bloqueo de clientes HTTP.

Comandos para repetir la serie completa, con Flutter, Cargo, Java y Android SDK en el entorno de las rondas anteriores. En esta ronda, la serie de aceptación de API 26 se completó en dos bloques consecutivos de tres y dos ejecuciones.

```powershell
adb -s emulator-5560 shell su 0 svc wifi disable
adb -s emulator-5560 shell su 0 svc data disable
adb -s emulator-5560 shell dumpsys wifi
adb -s emulator-5560 shell dumpsys connectivity
adb -s emulator-5560 shell settings get global mobile_data

foreach ($pulsysRun in 1..5) {
    flutter test integration_test -d emulator-5560 --reporter expanded
    if ($LASTEXITCODE -ne 0) { throw "API 26 failed on run $pulsysRun" }
}

adb -s emulator-5562 shell svc wifi disable
adb -s emulator-5562 shell svc data disable
foreach ($pulsysRun in 1..2) {
    flutter test integration_test -d emulator-5562 --reporter expanded
    if ($LASTEXITCODE -ne 0) { throw "API 37 failed on run $pulsysRun" }
}
```

### 4. Pendiente o en duda

El criterio de aceptación de esta ronda está cumplido. Ambos AVD quedan con Wi-Fi y datos desactivados.

## Ronda 5

### 1. Qué hice

- Sustituí la fila fija de 528 dp y su scroll horizontal por una composición con `LayoutBuilder`: cuatro signos a cada lado y figura escalada dentro del ancho disponible.
- Conservé los objetivos táctiles de 48 dp de las zonas frontales, separando sus centros al reducir la figura. Back y Other ocupan el ancho de la página debajo de las tres columnas; la leyenda tiene el título localizado Zone status.
- Compacté las tarjetas con ellipsis o `scaleDown`, conservando sus etiquetas semánticas completas. Recommendations queda en una línea y el asistente muestra Coming soon una sola vez.
- Añadí regresiones a 320, 411 y 600 dp y tres goldens a 411 × 891 dp. Verifiqué ambos emuladores sin red y las cuatro capturas nativas del release en el Pixel.

| Requisito | Archivo:línea | Prueba / evidencia |
|---|---|---|
| Tres columnas dentro de 320–600 dp, sin scroll horizontal interno | `app/lib/features/body/body_screen.dart:82`, `app/lib/features/overview/overview_screen.dart:49`, `app/lib/features/overview/vital_card.dart:67` | `responsive_pages_test.dart`: límites y posición de las ocho tarjetas a 320, 411 y 600 dp; en cada página existe exactamente un `Scrollable` horizontal y pertenece al `PageView`. |
| Figura escalable y zonas utilizables | `app/lib/features/body/body_figure.dart:15`, `app/lib/features/body/body_figure.dart:61` | Los 12 objetivos frontales permanecen dentro del canvas, miden al menos 48 dp y no se solapan en los tres anchos. La comparación admite 0,001 dp de redondeo de coma flotante. Los tests existentes de zonas y ficha siguen pasando. |
| Back/Other completos y leyenda identificada | `app/lib/features/body/body_screen.dart:238`, `app/lib/features/body/body_screen.dart:276`, `app/lib/l10n/app_en.arb:247` | El test comprueba los botones debajo del canvas y más anchos que él; capturas Today superior/inferior revisadas. |
| Recommendations en una línea, incluso a 320 dp | `app/lib/routing/app_shell.dart:245` | El test comprueba `maxLines`, ausencia de salto, límites del texto y `RenderParagraph.didExceedMaxLines`; texto completo conservado. |
| Un solo Coming soon junto al asistente | `app/lib/features/overview/overview_screen.dart:79` | Una única coincidencia dentro de `overview-assistant`; captura inferior. El campo deshabilitado usa desplazamiento vertical interno. |
| Deslizar sobre la figura cambia de página | `app/test/widgets/responsive_pages_test.dart:131` | Arrastre hacia History y hacia Recommendations con comprobación de selección semántica y sin abrir ficha. También verificado con `adb shell input ... swipe` en release. |
| Goldens del tamaño del Pixel | `app/test/goldens/pages_test.dart:30` | Tres referencias nuevas `page_history_411.png`, `page_today_411.png`, `page_recommendations_411.png`, de 411 × 891 dp; se mantienen las tres de 560 dp. |

### 2. Archivos tocados

- `app/lib/features/body/body_figure.dart` y `body_screen.dart`.
- `app/lib/features/overview/overview_screen.dart` y `vital_card.dart`.
- `app/lib/routing/app_shell.dart`.
- `app/lib/l10n/app_en.arb` y sus dos Dart generados en `app/lib/l10n/generated/`.
- `app/test/widgets/responsive_pages_test.dart` (nuevo), `app/test/goldens/pages_test.dart` y las referencias regeneradas en `app/test/goldens/baselines/`, incluidas las tres nuevas de 411 dp.
- Las cuatro capturas solicitadas en `reviews/etapa3-*-pixel7.png`, sobrescritas, y este informe.

### 3. Comandos y resultados reales

Flutter 3.50.0 / Dart 3.14.0. Entorno: `ANDROID_HOME=C:\Users\Salva\AppData\Local\Android\Sdk`, `JAVA_HOME=C:\Program Files\Android\Android Studio\jbr`, `RUSTUP_TOOLCHAIN=1.96.0`; Flutter, Cargo, Java y platform-tools en PATH.

| Comando desde `app/` | Resultado final observado |
|---|---|
| `flutter gen-l10n` | Código 0; generados actualizados con `bodyLegendTitle`. |
| `dart format --output=none --set-exit-if-changed lib test integration_test` | Código 0; 61 archivos, 0 cambios. |
| `dart format --output=none --set-exit-if-changed .`, al terminar las suites | Código 0; 61 archivos, 0 cambios. |
| `flutter analyze --fatal-infos` | Código 0; No issues found. |
| `flutter test test/widgets/responsive_pages_test.dart --reporter expanded` | 4/4 aprobados. En la preparación se corrigieron una conversión `num` → `double` y una comparación exacta de 48 dp que recibía 47,99999999999997 a 600 dp. |
| `flutter test --exclude-tags golden --reporter expanded` | Código 0; 74/74 aprobados. |
| `flutter test --update-goldens --tags golden --reporter expanded` | Código 0; 10/10 aprobados y baselines actualizadas. |
| `flutter test --tags golden --reporter expanded` | Código 0; 10/10 aprobados contra las referencias regeneradas. |
| `flutter test integration_test -d emulator-5560 --reporter expanded` | Código 0; 2/2 aprobados sin red, API 26. |
| `flutter test integration_test -d emulator-5562 --reporter expanded` | Código 0; 2/2 aprobados sin red, API 37. |
| `flutter build apk --release` | Código 0; `build/app/outputs/flutter-apk/app-release.apk`, 39,7 MB. |
| `git diff --check` sobre los archivos de esta ronda | Código 0; solo avisos de normalización LF/CRLF en localización. |

Desconexión aplicada antes de la integración y comprobada con `dumpsys connectivity`: `Active default network: none` en ambos AVD. En API 26 se utilizó root, como en Ronda 4:

```powershell
adb -s emulator-5560 shell su 0 svc wifi disable
adb -s emulator-5560 shell su 0 svc data disable
adb -s emulator-5562 shell svc wifi disable
adb -s emulator-5562 shell svc data disable
adb -s emulator-5560 shell dumpsys connectivity
adb -s emulator-5562 shell dumpsys connectivity
```

**Verificación nativa y capturas.** Instalé con `adb -s emulator-5562 install -r app/build/app/outputs/flutter-apk/app-release.apk` desde la raíz. El paquete instalado no tiene `DEBUGGABLE`. `wm size` da 1440 × 3120 y `wm density` 560: aproximadamente 411,43 × 891,43 dp. Abrí con `adb -s emulator-5562 shell am start -W -n com.pulsys.app/.MainActivity` y comprobé los siguientes gestos sobre la figura:

```powershell
# Today superior: abrir después de force-stop y capturar.
adb -s emulator-5562 shell am force-stop com.pulsys.app
adb -s emulator-5562 shell am start -W -n com.pulsys.app/.MainActivity

# Today inferior: desplazar y capturar.
adb -s emulator-5562 shell input swipe 720 2500 720 900 450

# Para los gestos laterales, volver a Today superior con un arranque nuevo.
# Hacia History, desde dentro de la figura: seleccionado History.
adb -s emulator-5562 shell input swipe 450 1700 1380 1700 500

# PULSYS vuelve al centro. Hacia Recommendations: seleccionado Recommendations.
adb -s emulator-5562 shell input tap 230 230
adb -s emulator-5562 shell input touchscreen swipe 940 1700 80 1700 500

# Después de cada gesto, volcar la selección accesible y capturar la página estable.
adb -s emulator-5562 shell uiautomator dump /sdcard/pulsys-r5.xml
adb -s emulator-5562 exec-out cat /sdcard/pulsys-r5.xml
```

Las selecciones de los indicadores se comprobaron en el XML y en la imagen. Los gestos finales recorren más de media pantalla; los ensayos cortos que volvieron a Today no se contaron como validación. Para escribir cada PNG se usó `adb exec-out screencap -p` mediante `subprocess.check_output` y `Path.write_bytes`, sin redirección de texto de PowerShell. Ejemplo exacto desde la raíz, cambiando el nombre por el de la página capturada:

```powershell
python -c "import subprocess;from pathlib import Path;Path('reviews/etapa3-today-pixel7.png').write_bytes(subprocess.check_output(['C:/Users/Salva/AppData/Local/Android/Sdk/platform-tools/adb.exe','-s','emulator-5562','exec-out','screencap','-p']))"
```

Capturas reales sobrescritas y revisadas visualmente, todas de 1440 × 3120:

- [Today superior](etapa3-today-pixel7.png): ambas columnas completas, figura escalada y Recommendations en una línea.
- [Today inferior](etapa3-today-abajo-pixel7.png): Back/Other completos, Zone status y un único Coming soon bajo el campo.
- [History](etapa3-history-pixel7.png): llegada por gesto y navegación inferior en una línea.
- [Recommendations](etapa3-recomendaciones-pixel7.png): llegada por gesto y navegación inferior en una línea.

### 4. Pendiente o en duda

El criterio de aceptación de Ronda 5 está cumplido. Las capturas de release muestran el estado local sin lecturas; la integración verifica el registro con confirmación y la aparición del punto en History. Los tests y capturas anteriores de otras rondas se conservan como historial; esta sección sustituye la decisión de Ronda 3 de permitir scroll horizontal. Ambos emuladores siguen sin red.

## Ronda 6

### 1. Qué hice

- Leí completa `etapa3-paginas-astra.md` y corregí los nueve puntos de la tarea, incluidos los hallazgos menores de sus dos tablas.
- Separé los resultados de lectura y evaluación en carga, correcto, anterior con hora y fallo. La tarjeta solo atribuye la evaluación a una lectura si ambas consultas son correctas y Rust incluye ese registro como evidencia.
- Asocié la navegación a la identidad de API, conservé los ocho signos en la lista de zonas y completé la alerta persistente, el orden de Recommendations y los dos estados vacíos de History.
- Amplié las regresiones, corregí las métricas de las goldens y comprobé integración offline, Atrás de Android y capturas del release. No modifiqué Rust ni añadí dependencias o reglas médicas.

| Hallazgo | Cambio → archivo:línea | Prueba y resultado |
|---|---|---|
| **1. Lectura nueva y evaluación anterior tras fallo parcial** | `overview/overview_controller.dart:5` distingue resultados; `:55` evita asociar una evaluación anterior o ajena al registro. `overview/status_summary.dart:73`, `overview/vital_card.dart:53` y `assistant/recommendations_screen.dart:26` muestran qué consulta falló y la hora de los datos anteriores. Los dos resultados se publican juntos al completar la generación de carga. | `pages_revision_test.dart:17`: carga correcta → guardar 78 → falla solo estado o solo lectura → Today y Recommendations identifican el dato anterior → reintento muestra 78 y Alarm. Se comprueba que la tarjeta no atribuye Stable ni Alarm al dato sin evaluación vigente. `:90` distingue un fallo inicial de No data. Aprobados. |
| **2. `/pulse` sigue ligada a la API A** | `app.dart:18` usa `ObjectKey(api)` en la raíz de navegación. Al sustituir la API se dispone la pila anterior, incluido el formulario y sus diálogos; B abre una pila y un borrador nuevos. | `pages_revision_test.dart:114`: dos fakes con almacenes A/B, en edición y con confirmación abierta. Desaparecen formulario y diálogo de A; B abre vacío y guarda 78. A conserva su lectura de 72 y no recibe nuevas confirmaciones; B no recibe el borrador 201. Ambos casos aprobados. |
| **3. La lista oculta el registro de Pulso** | `body/body_screen.dart:210`: `BodyStage` conserva las columnas de signos; el conmutador cambia únicamente su contenido central entre figura y lista. | `pages_revision_test.dart:166`: ocho tarjetas en modo lista → abrir Pulso → volver a Today en la misma lista → abrir la ficha de Head. Aprobado. |
| **4. Alerta persistente ausente en History** | `overview/status_summary.dart:105` presenta persistencia, estado/motivo disponible, hora y fuente/regla de Rust. `routing/app_shell.dart` la coloca antes de `ProfileSummary` solo si `pulse.persistent` es verdadero. | `pages_revision_test.dart:197`: casos true/false, texto Persistent, Change detected, hora y regla; se mide el orden sobre el perfil. La prueba de orden de `app_shell_test.dart` usa ahora una alerta real, con altura. Aprobados. |
| **5. Orden y bloques de Recommendations** | `assistant/recommendations_screen.dart:47`: sugerencia → motivo/evaluación → acción ante alarma → fuentes → mediciones/evidencias → advertencia. Después quedan los espacios de ocio y pautas de ejercicio. Procedencia, versión e IDs salen del DTO; el texto del asistente sigue siendo provisional. | `pages_revision_test.dart:236`: alarma en Chest, orden de los ocho bloques, Contact flight surgeons antes de fuentes y mediciones, y placeholders presentes. Los cuatro casos existentes de pulso/zona siguen comprobando que el contacto aparece solo ante alarma. Goldens de ambas anchuras y capturas release revisadas. |
| **6. Rango vacío confundido con almacén vacío; DEMO** | `history/history_controller.dart:28` y `:125` separan `HistoryEmpty` de `HistoryRangeEmpty`. `history/history_screen.dart:121` nombra el rango vacío; `:149` reserva el aviso de DEMO para una consulta sin lecturas. | `history_screen_test.dart`: almacén vacío muestra No pulse readings recorded yet y el aviso DEMO, también al cambiar el rango. `:304`: lectura de hace dos días → 6 h muestra No pulse readings in the selected range (6 h), sin DEMO → 7 days recupera el gráfico. Aprobados. La fachada no tiene comando ni catálogo de escenarios DEMO: se muestra su indisponibilidad, como permite la tarea. |
| **7. Golden de Today sin métricas ni leyenda** | `goldens/pages_test.dart:47` parte de `MediaQueryData.fromView(tester.view)` y cambia únicamente `disableAnimations`. Añadí las referencias inferiores de Today a 560 y 411 dp. | `pages_test.dart:66` y `responsive_pages_test.dart` comprueban altura positiva de la leyenda y que título/estado están dentro tanto de su rectángulo como del viewport tras desplazar. También se comprueba que la cabecera no se mueve. Zone status y Not evaluated son visibles en las referencias revisadas. |
| **8. History ya abierta no se refresca en la prueba anterior** | Se conserva el mecanismo `refreshToken`; la prueba nueva ejercita su valor visible. La integración abre History y selecciona 6 h antes de abrir de nuevo el formulario para corregir. | `pages_revision_test.dart:296`: fila 72 visible en History → corregir a 78 → volver a History sin cambiar rango → fila antigua ausente, tabla/tarjeta/perfil nuevos → Today muestra 78 con la misma evaluación Alarm. `integration_test/pulse_flow_test.dart:257` verifica el mismo refresco con FRB/SQLite y los IDs efectivos. Aprobado en widget y ambos AVD. |
| **9. Navegación, muestreo y objetivos a 2×** | Las comprobaciones de página exigen indicador seleccionado y contenido propio; History se identifica por `recordId`. `app_shell_test.dart:95` verifica la petición de salida y `integration_test/android_back_test.ps1` envía Atrás real y consulta la actividad en primer plano. La prueba de filtro usa 122 lecturas válidas, 40 dentro de 6 h. | `history_screen_test.dart:244` prueba los límites inclusivos y demuestra que muestrear antes de filtrar produce otra serie. `app_shell_test.dart:360` mide los tres rectángulos semánticos ≥48 × 48 dp a 320 dp/2× y exige los nombres completos de las ocho tarjetas. `android_back_test.ps1` aprobado en API 26 y 37: tras Atrás, la actividad reanudada es NexusLauncher. |

Las rutas de código abreviadas de la tabla están bajo `app/lib/features/`, salvo `app.dart` y `routing/`. Las pruebas de widget están bajo `app/test/widgets/`.

### 2. Archivos tocados

- `app/lib/app.dart` y `app/lib/routing/app_shell.dart`.
- `app/lib/features/overview/{overview_controller,overview_screen,status_summary,vital_card}.dart`.
- `app/lib/features/body/body_screen.dart` y `app/lib/features/history/{history_controller,history_screen}.dart`.
- `app/lib/features/assistant/recommendations_screen.dart`.
- `app/lib/l10n/app_en.arb` y `generated/{app_localizations,app_localizations_en}.dart`.
- `app/test/fakes/fake_pulsys_api.dart`, `app/test/support/overview_fixture.dart` y `app/test/widgets/{app_shell_test,history_screen_test,responsive_pages_test,pages_revision_test}.dart`; el último es nuevo. El fake admite sustituir su evaluación inyectada, sin calcular umbrales.
- `app/test/goldens/pages_test.dart` y referencias en `baselines/`. Cambian visualmente las páginas Today y Recommendations; se añaden `page_today_bottom.png` y `page_today_bottom_411.png`. La suite completa regenera y compara doce imágenes en diez pruebas.
- `app/integration_test/pulse_flow_test.dart` y el nuevo `android_back_test.ps1`.
- Este informe y las cuatro capturas `reviews/etapa3-{today,today-abajo,history,recomendaciones}-pixel7.png`, sobrescritas.

### 3. Comandos y resultados reales

Entorno de compilación utilizado:

```powershell
$env:ANDROID_HOME = 'C:\Users\Salva\AppData\Local\Android\Sdk'
$env:JAVA_HOME = 'C:\Program Files\Android\Android Studio\jbr'
$env:RUSTUP_TOOLCHAIN = '1.96.0'
$env:Path = "C:\dev\flutter\bin;C:\Users\Salva\.cargo\bin;$env:JAVA_HOME\bin;$env:ANDROID_HOME\platform-tools;$env:Path"
```

| Comando desde `app/`, salvo indicación | Resultado real final |
|---|---|
| `flutter gen-l10n` | Código 0. Catálogo y generados actualizados. |
| `dart format lib test integration_test` | Código 0. Archivos modificados formateados. |
| `dart format --output=none --set-exit-if-changed .` | Código 0. 62 archivos, 0 cambios. |
| `flutter analyze --fatal-infos` | Código 0. No issues found, incluida la ejecución final. |
| `flutter test --exclude-tags golden --reporter expanded` | Código 0. **84/84 aprobados**. |
| `flutter test --update-goldens --tags golden --reporter expanded` | Código 0. **10/10 aprobados**; referencias regeneradas. |
| `flutter test --tags golden --reporter expanded` | Código 0. **10/10 aprobados**, con doce comparaciones PNG. Revisé las referencias de Today, su parte inferior, History y Recommendations. |
| `flutter test integration_test -d emulator-5560 --reporter expanded` | Código 0. **2/2 aprobados sin red**, API 26. |
| `flutter test integration_test -d emulator-5562 --reporter expanded` | Código 0. **2/2 aprobados sin red**, API 37. |
| `flutter build apk --release` | Código 0. APK de 39,7 MB en `build/app/outputs/flutter-apk/app-release.apk`. |
| `adb -s emulator-5560 install -r build/app/outputs/flutter-apk/app-release.apk` y equivalente para 5562 | Success en ambos. Se instaló el release para comprobar Atrás nativo; el paquete del Pixel carece de DEBUGGABLE. |
| `./integration_test/android_back_test.ps1 -Serial emulator-5560` y equivalente para 5562 | **PASS en ambos**. Today seleccionado antes de `KEYCODE_BACK`; después se reanuda `com.google.android.apps.nexuslauncher/.NexusLauncherActivity`. |
| `git diff --check` sobre los archivos de la ronda, desde la raíz | Código 0. Avisos de normalización LF/CRLF en archivos ya presentes, sin errores de espacios. |

Durante la preparación, el primer analyze encontró un import sobrante y un `!` innecesario en `VitalCard`; quedaron corregidos. La primera suite dirigida terminó con 33/35 aprobados: dos aserciones nuevas usaban Show figure y Change en vez de las cadenas existentes Show body figure y Change detected. Corregí las aserciones al catálogo; las ejecuciones finales anteriores incluyen ambos casos aprobados.

Antes de la integración desactivé Wi-Fi y datos. `dumpsys connectivity` dio **Active default network: none** y `settings get global mobile_data` dio **0** en los dos dispositivos; la comprobación posterior al release sigue sin red. Comandos ejecutados:

```powershell
adb -s emulator-5560 shell su 0 svc wifi disable
adb -s emulator-5560 shell su 0 svc data disable
adb -s emulator-5562 shell svc wifi disable
adb -s emulator-5562 shell svc data disable
adb -s emulator-5560 shell dumpsys connectivity
adb -s emulator-5562 shell dumpsys connectivity
adb -s emulator-5560 shell settings get global mobile_data
adb -s emulator-5562 shell settings get global mobile_data
```

**Capturas reales del release.** Pixel `emulator-5562`, 1440 × 3120, densidad 560 (aproximadamente 411,43 × 891,43 dp). Abrí con `am force-stop` y `am start -W`, capturé Today, desplacé para su parte inferior y reinicié la actividad para los gestos laterales. La selección se comprobó con `uiautomator dump` y el atributo `selected` del indicador antes de escribir cada archivo. Gestos ejecutados:

```powershell
adb -s emulator-5562 shell input swipe 720 2500 720 900 450
adb -s emulator-5562 shell input swipe 450 1700 1380 1700 500
adb -s emulator-5562 shell input tap 230 230
adb -s emulator-5562 shell input touchscreen swipe 940 1700 80 1700 500
```

Se utilizó `adb exec-out screencap -p`, escribiendo bytes con Python. Ejemplo exacto desde la raíz, repetido con cada nombre de destino:

```powershell
python -c "import subprocess;from pathlib import Path;Path('reviews/etapa3-today-pixel7.png').write_bytes(subprocess.check_output(['C:/Users/Salva/AppData/Local/Android/Sdk/platform-tools/adb.exe','-s','emulator-5562','exec-out','screencap','-p']))"
```

Capturas sobrescritas y revisadas visualmente:

- [Today superior](etapa3-today-pixel7.png): ambas columnas y figura completas en ancho; barra inferior en una línea.
- [Today inferior](etapa3-today-abajo-pixel7.png): Back/Other, Zone status, contexto y Coming soon único.
- [History](etapa3-history-pixel7.png): estado vacío real, perfil y navegación.
- [Recommendations](etapa3-recomendaciones-pixel7.png): sugerencia al inicio, motivo, fuentes y mediciones. Desplacé también al final y comprobé por imagen y XML la advertencia, ocio y ejercicio; esa captura auxiliar quedó en TEMP.

### 4. Pendiente o en duda

- Los hallazgos **1–9** y la aceptación ejecutable de Ronda 6 están cubiertos. Los dos AVD quedan sin red y con el release instalado.
- La fachada expone persistencia del pulso, no un catálogo adicional de alertas persistentes. Esas alertas requerirían ampliar su contrato. Tampoco expone escenarios ni un comando de selección DEMO: queda su aviso de indisponibilidad únicamente cuando no hay lecturas, según la excepción de la tarea.
- Los estados y metadatos mostrados proceden de Rust. Las evaluaciones del fake son fixtures sintéticos y no constituyen reglas médicas nuevas. Ocio, pautas de ejercicio y sugerencia del asistente permanecen como placeholders.
- **NO EJECUTADO:** recorrido manual con TalkBack en API 26 y 37. Es el hallazgo bloqueante separado de las dos tablas revisadas; no se presenta la comprobación semántica ni Atrás por ADB como sustituto de ese recorrido.

## Ronda 7

### 1. Qué hice

- Leí completo `etapa3-paginas-recheck-astra.md` y corregí los tres hallazgos parciales: **1, 5 y 6**.
- La identidad de la evaluación usa el primer ID de la evidencia de Rust, que identifica la lectura evaluada; los restantes son antecedentes. Una hora coincidente o pertenecer a la lista no basta.
- Recommendations presenta un motivo principal breve antes de la acción. Sus fuentes y las evidencias de cada zona corresponden a sus síntomas; el último pulso independiente tiene su propio bloque cuando no es la lectura usada.
- Añadí la comprobación autoritativa de almacén vacío al DTO, regeneré FRB y probé síntomas y otros datos sin pulsos. Pasan formato/análisis Dart, 90 tests, 10 pruebas golden, 285 pruebas Rust y las integraciones offline en ambos AVD.

| Hallazgo | Cambio → archivo:línea | Prueba |
|---|---|---|
| **1. Un antecedente acepta el estado de otra lectura** | `app/lib/platform/pulsys_api.dart:390` expone `PulseEvaluation.evaluatedRecordId`; `overview_controller.dart:62` exige igualdad con ese ID, además de las dos consultas correctas. No fue necesario ampliar Rust para esta relación. | `pages_recheck_test.dart:12`: dos registros A/B con **la misma hora**, History devuelve B y la evaluación contiene `[A, B]`. B muestra 85 bpm con Evaluation unavailable for this reading; en Recommendations sus bpm quedan fuera de Measurements used. El caso positivo `[B, A]` sí permite el estado Alarm y los 85 bpm en la evidencia usada. Los dos casos pasan con ambas consultas exitosas. |
| **5. Motivo largo y evidencia de Pulso para una zona** | `assistant/recommendation_reason.dart:3` selecciona un único motivo a partir de los estados de Rust, conservando el orden recibido entre candidatos de igual estado. `recommendations_screen.dart:60` presenta solo su etiqueta y hora. `:140` muestra las fuentes del motivo seleccionado y `:201` agrupa los IDs de registros/eventos, hora y fuente de cada síntoma bajo su zona. | `pages_recheck_test.dart:91`: Head, Chest y Abdomen en alarma, todas con síntomas no vacíos. Reason contiene solo el motivo Head y tres textos en total; la acción precede al detalle. Se exigen los IDs `*-current`, `*-prior`, `*-event` y la fuente dentro de la zona correspondiente, y se rechaza el pulso ajeno dentro de Measurements used. Aprobado. Goldens de Recommendations a 560 y 411 dp regeneradas y revisadas. |
| **6. Cero pulsos no prueba almacén vacío** | `crates/store/src/lib.rs:110` consulta existencia en records, events, telemetry_readings y profile. `crates/api/src/records.rs:68` expone `ZoneSnapshot.is_store_empty`, transportado por FRB y `RustAdapter`. `history_controller.dart` consulta esa prueba únicamente cuando no hay pulsos y ofrece DEMO solo con una respuesta afirmativa; ante error queda desconocido. | `history_screen_test.dart:52`: un síntoma y cero pulsos no muestran DEMO. También se prueban otros registros sin pulsos y fallo de la consulta de vacío. Se conserva el caso verdaderamente vacío. `crates/api/tests/zones.rs:147` verifica SQLite real con un síntoma y con temperatura, ambos sin pulso. Los tests de store cubren además eventos, telemetría y perfil, y que las evaluaciones derivadas no llenan el almacén de usuario. La integración verifica el booleano verdadero antes del registro y falso tras reabrir su escritura. |

**Relación exacta para el hallazgo 1.** `crates/core/src/evaluate.rs:165` añade primero `latest.record_id`; `:203` añade después los antecedentes. La fachada, el bridge y el adaptador conservan ese orden. La prueba existente `zones_project_symptom_evidence_and_independent_pulse_evaluation` exige el vector completo en ese orden. Se encapsula la relación en el tipo Dart para impedir que otra tarjeta vuelva a interpretar pertenencia como identidad.

**Por qué fue imprescindible tocar Rust para el hallazgo 6.** El contrato anterior exponía pulso y síntomas evaluados, pero no permitía distinguir un almacén vacío de otro con registros de otros tipos, datos antiguos/no evaluables, eventos, telemetría o perfil. Añadí un único booleano calculado en almacenamiento, sin migración, comandos de escritura ni cambios médicos. No se cuentan las tablas derivadas de evaluaciones/outbox: `zone_status()` puede crearlas incluso sin datos de usuario. El campo es obligatorio en el DTO Dart y se regenera con FRB, sin valor supuesto de reserva.

### 2. Archivos tocados

- `app/lib/platform/pulsys_api.dart`, `rust_adapter.dart` y generados FRB `api.dart` / `frb_generated.dart`.
- `app/lib/features/overview/overview_controller.dart`.
- `app/lib/features/assistant/recommendation_reason.dart` (nuevo) y `recommendations_screen.dart`.
- `app/lib/features/history/history_controller.dart` y `history_screen.dart`.
- `app/lib/l10n/app_en.arb` y sus dos Dart generados.
- `app/test/widgets/pages_recheck_test.dart` (nuevo), `pages_revision_test.dart`, `history_screen_test.dart`; fakes `fake_pulsys_api.dart` y `fake_rust_session.dart`.
- `app/test/goldens/baselines/page_recommendations.png` y `page_recommendations_411.png`; suite de referencias regenerada y comparada.
- `app/integration_test/pulse_flow_test.dart`, con aserciones de vacío real antes y después del guardado/reapertura.
- `crates/store/src/lib.rs`, `crates/api/src/records.rs`, `crates/api/tests/zones.rs`, `crates/bridge/src/{api,dto,frb_generated}.rs`.
- Este informe. Se preservaron los cambios previos de `pulse_history.rs` y sus tests. `core/src/telemetry.rs` y `store/src/records.rs` no tienen cambios frente a HEAD.

### 3. Comandos y resultados reales

Entorno: Flutter en `C:\dev\flutter\bin`, Rust **1.96.0**, FRB CLI **2.11.1**, Android SDK y Java de las rondas anteriores. No se cambiaron dependencias ni locks.

| Comando desde `app/`, salvo indicación | Resultado real |
|---|---|
| `flutter_rust_bridge_codegen --version` | 2.11.1. |
| `flutter_rust_bridge_codegen generate` | Código 0. Campo y codecs actualizados por el generador. |
| `dart run build_runner build` | Código 0. Un output; sin actualización de paquetes. Emitió el aviso conocido de que el SDK Dart es más nuevo que el analyzer usado por el generador. |
| `flutter gen-l10n` | Código 0. |
| `dart format lib test integration_test` | Código 0. |
| `dart format --output=none --set-exit-if-changed lib test integration_test` | Código 0. **64 archivos, 0 cambios**. |
| `dart format --output=none --set-exit-if-changed .`, al terminar integración | Código 0. **64 archivos, 0 cambios**. |
| `flutter analyze --fatal-infos` | Código 0. No issues found. |
| `flutter test --exclude-tags golden --reporter expanded` | Código 0. **90/90 aprobados**, incluidos los seis casos nuevos. |
| `flutter test --update-goldens --tags golden --reporter expanded` | Código 0. **10/10 aprobados**; se revisaron las dos referencias modificadas de Recommendations. |
| `flutter test --tags golden --reporter expanded` | Código 0. **10/10 aprobados**, doce comparaciones PNG. |
| `flutter test integration_test -d emulator-5560 --reporter expanded` | Código 0. **2/2 aprobados sin red**, API 26, con el campo nuevo de Rust. |
| `flutter test integration_test -d emulator-5562 --reporter expanded` | Código 0. **2/2 aprobados sin red**, API 37. |
| `cargo clippy --workspace --all-targets --locked -- -D warnings`, raíz | Código 0. |
| `cargo test --workspace --locked`, raíz | Código 0. **285 aprobados, 0 fallidos, 3 omitidos**. Las omisiones requieren red/token de simulación o el paquete de voz instalado. Incluye dos doctests aprobados. |
| `cargo fmt -p pulsys-api -p pulsys-bridge -- --check`, raíz | Código 0. El módulo generado mantiene el `rustfmt::skip` que ya establece su propietario FRB. |
| `rustfmt --edition 2024 --check --config skip_children=true crates/store/src/lib.rs crates/api/src/records.rs crates/api/tests/zones.rs crates/bridge/src/api.rs crates/bridge/src/dto.rs`, raíz | Código 0. Archivos manuales modificados formateados. |
| `cargo fmt --all -- --check`, raíz | **Código 1 por formato previo ajeno** en `crates/core/src/telemetry.rs` y `crates/store/src/records.rs`; detalle abajo. |
| `git diff --check -- app/lib app/test app/integration_test crates/store/src/lib.rs crates/api crates/bridge`, raíz | Código 0; avisos LF/CRLF, sin errores de espacios. |

La primera integración no llegó a ejecutar tests: al recompilar los crates modificados faltaba `rust-std` para Android en el toolchain 1.96.0. Lo comprobé con `rustup target list --installed --toolchain 1.96.0`, instalé los targets y repetí la serie completa. Comando ejecutado:

```powershell
rustup target add --toolchain 1.96.0 aarch64-linux-android x86_64-linux-android
```

La instalación terminó con código 0. La compilación posterior construyó ambos ABI y las dos integraciones pasaron. El primer control de formato sobre `.` después del intento fallido encontró `build/test/listener_0.dart`, generado temporalmente por Flutter. Tras terminar la integración, el mismo control completo pasó sin cambios.

**Formato global de Rust.** El comando global solicita envolver atributos serde en `core/src/telemetry.rs:14` y líneas siguientes, y dos llamadas a `transaction_with_behavior` en `store/src/records.rs:672` y `:761`. `git diff --exit-code -- crates/core/src/telemetry.rs crates/store/src/records.rs` termina con código 0: los dos archivos conservan su contenido previo. Se retiraron únicamente dos cambios incidentales de formato que `cargo fmt -p pulsys-store` había hecho en el segundo archivo. El chequeo de los archivos modificados y el de api/bridge pasan. No se presenta el formato global como limpio.

**Condición offline.** Antes de ejecutar:

```powershell
adb -s emulator-5560 shell su 0 svc wifi disable
adb -s emulator-5560 shell su 0 svc data disable
adb -s emulator-5562 shell svc wifi disable
adb -s emulator-5562 shell svc data disable
adb -s emulator-5560 shell dumpsys connectivity
adb -s emulator-5562 shell dumpsys connectivity
adb -s emulator-5560 shell settings get global mobile_data
adb -s emulator-5562 shell settings get global mobile_data
```

Ambos devolvieron **Active default network: none** y **mobile_data=0**. Se comprobó de nuevo la ausencia de red al repetir las integraciones. La descarga del toolchain se hizo en el host y no habilitó la red de los AVD.

### 4. Pendiente o en duda

- Los tres hallazgos de esta ronda quedan corregidos con sus regresiones. El aviso DEMO sigue siendo de indisponibilidad de escenarios y ahora se reserva a un almacén de usuario realmente vacío, no a cero pulsos.
- Los IDs, horas, estados y fuentes de síntomas son los que entrega Rust; no se inventan puntuaciones ni valores ausentes en el DTO. Las referencias visuales actualizadas son las goldens de Recommendations.
- Persiste la diferencia de formato global de Rust descrita arriba, fuera de los archivos necesarios para estos arreglos. Los controles Dart, Clippy, pruebas Rust e integraciones pasan.
- **NO EJECUTADO:** recorrido manual de TalkBack, que permanece como verificación independiente de estos tres hallazgos.
