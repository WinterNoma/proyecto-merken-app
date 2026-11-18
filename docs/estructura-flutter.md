# Pulsys móvil · estructura de Flutter y Rust

**Decisión del 18 de noviembre de 2026, rama `flutter-migration`.** Flutter es el único host de la aplicación Android/iOS. El identificador de ambos destinos es `com.pulsys.app`. Se abre `pulsys.sqlite3` en el directorio privado de Pulsys. La primera instalación empieza limpia. Las actualizaciones que se ensayan son entre versiones de Pulsys. El nombre y la ruta ya aparecen en `src-tauri/tauri.conf.json:3–5` y `src-tauri/src/lib.rs:675–679`.

Este documento fija la arquitectura que se construirá. Sustituye las condiciones sobre continuidad de Tauri, identidad, hardware y decisión de la etapa 3 de `docs/plan-flutter-migracion.md:3–19,123–129`. Se consultan `src-tauri/`, `ui/`, `plugins/` y el antiguo plugin `crates/local-speech/` únicamente para entender comportamiento. El nuevo grafo de compilación, las pruebas y los servicios se construyen desde Flutter y los crates de producto. No hay datos de `com.merken.app` que importar. Las referencias de línea corresponden al árbol local con sus cambios sin commit.

## 1. Carpetas y dependencias

El camino de un botón hasta el disco es **widget → controlador Dart → adaptador Dart → bridge → api → store → SQLite**. El núcleo puro valida y evalúa tanto al escribir como al leer. Las carpetas se crean con su primer consumidor. El árbol muestra el destino completo por responsabilidad, incluidos los recorridos posteriores al pulso. La etapa 2 entrega solo el subconjunto indicado en §7. Véanse `reviews/plan-flutter-astra.md:24–44`, `docs/plan-flutter-migracion.md:31–51` y `ui/src/main.ts:12–32`.

```text
app/
  pubspec.yaml, pubspec.lock       Dependencias Dart, SDK, recursos locales y herramientas de prueba
  flutter_rust_bridge.yaml        Entrada Rust, salidas Rust/Dart y cabecera nativa
  l10n.yaml                       Generación del catálogo inglés
  lib/
    main.dart                     Arranque real, inicialización FRB y apertura de una sesión
    app.dart                      PulsysApp(api), tema, idioma y raíz de navegación
    features/
      pulse/                      Formulario, controlador, aviso, confirmación y corrección de pulso
      overview/                   Daily overview, acceso rápido a registro y Need help
      history/                    Tendencia efectiva, filtros, detalle y tabla accesible
      body/                       Figura, selección, lista de zonas y fichas con fuentes
      records/                    Resto del catálogo y edición de mediciones por tipo
      events/                     Ayuda urgente confirmada y cronología de eventos
      crew/                       Perfil local, selector, escenarios DEMO e importación
      telemetry/                  Series del feed por tripulante, procedencia y verificación
      reports/                    Vista previa del informe y entrega del archivo exportado
      sync/                       Bandeja de salida, recepción y análisis de Tierra
      assistant/                  Dictado, borradores interpretados, explicación y recomendaciones
      settings/                   Preparación de modelos, permisos y recordatorios
    platform/
      pulsys_api.dart             Interfaz Dart y tipos de presentación del adaptador
      rust_adapter.dart           Implementación, mapeo de DTO y errores, ciclo de sesión
      generated/                  Salida Dart completa de FRB, versionada
      device/                     Directorio privado, borrador pendiente y acceso a servicios nativos
    design/
      tokens/                     Equivalencias Dart de los tokens activos del brandbook
      theme.dart                  ThemeData y estilos semánticos a partir de los tokens
      components/                 Controles compartidos al aparecer su segundo uso
    l10n/                         app_en.arb y salida de gen-l10n en generated/
    routing/                      Rutas tipadas y construcción de pantallas con su controlador
  assets/
    fonts/                        Familias locales y licencias para uso sin conexión
    brand/                        Wordmark aprobado
    body/                         Ilustración, contorno y regiones con ID anatómico
  android/                        Runner Kotlin, Gradle, permisos, ciclo de vida y enlace por ABI
  ios/                            Runner Swift, permisos, despliegue y enlace de la biblioteca Rust
  packages/
    local_speech/{lib,android,ios}/  Contrato Dart, micrófono/dictado y reproducción Kotlin/Swift
    slm_assets/{lib,android,ios}/    Contrato Dart y acceso Kotlin/Swift a archivos para modelos
  test/
    fakes/                        Adaptador en memoria con respuestas y fallos programables
    widgets/                      Interacción, navegación, semántica y errores
    golden/                       Pruebas visuales y referencias PNG en baselines/
  integration_test/               Recorridos sobre app instalada con FRB y SQLite reales

crates/
  api/                            Fachada Rust del producto; funciones, DTO y sesión concreta
    src/lib.rs                    open_store, close_store, reloj, App y ApiError
    src/records.rs                Preparación, confirmación, historial y estado de zonas del corte
    tests/                        Contrato público con reloj fijo y base temporal reabrible
  bridge/                         Único límite FFI del código Rust propio
    src/lib.rs                    Enlace de módulos y exportación nativa
    src/api.rs                    Funciones expuestas a FRB, DTO de transporte y sesión opaca
    src/worker.rs                 Cola acotada y propietario de la sesión api::App
    src/frb_generated.rs          Salida Rust de FRB, junto a sus auxiliares generados
  core/                           Dominio puro, catálogo, unidades, reglas y selección efectiva
  store/                          Implementación concreta rusqlite, esquema, transacciones y outbox
  seed/                           Escenarios sintéticos aplicados por store
  sync/                           Transporte y simulación; futuro cliente HTTPS de Tierra
  slm/                            Interpretación asistida, explicación, fichas y control del motor
  voice/                          Transcripción y síntesis offline, sin permisos de plataforma
  local-speech/                   Antiguo plugin Tauri, solo especificación de dictado Android
```

Los seis crates reutilizados ya se llaman `pulsys-*` en este checkout. Se conservan sus nombres y los nombres históricos `merken-*` que aún pudieran aparecer no motivan un renombrado adicional. Los dos nuevos paquetes serán `pulsys-api` y `pulsys-bridge`. En E1 el integrador sustituye los miembros comodín por la lista explícita de los seis crates y `api`, y excluye `src-tauri`, `plugins/slm-assets` y `crates/local-speech`. En E2 añade `bridge`. Esto importa porque el manifiesto actual incorpora directamente el host y ambos plugins. Véanse `Cargo.toml:1–21`, `crates/local-speech/Cargo.toml:1–16` y las líneas 1–3 de `crates/{core,store,seed,sync,slm,voice}/Cargo.toml`.

`api` usa `store` y `core`, y añade los motores opcionales por recorrido. `store` conserva su dependencia en `seed`. No se introduce un trait de repositorio ni uno por comando. Todo el Rust propio hereda la prohibición de `unsafe` salvo `bridge`. Ese paquete conserva los límites de Clippy, usa `deny(unsafe_code)` y permite la excepción local solo en el módulo FFI generado que la necesite. Su manifiesto no hereda el `forbid` que impediría esa excepción. El perfil básico enlaza SQLite bundled y deja llama y sherpa desactivados. Referencias en `Cargo.toml:19,25–35`, `crates/store/Cargo.toml:8–18`, `crates/slm/Cargo.toml:8–24` y `crates/voice/Cargo.toml:8–17`.

## 2. Estado, navegación e inyección en Flutter

**Estado y navegación usan solo el SDK de Flutter**, sin paquetes externos ni piso de versión de paquetes para esas dos funciones. Cada recorrido con operaciones asíncronas tiene un `ChangeNotifier` que recibe `PulsysApi` en el constructor. `ListenableBuilder` actualiza el fragmento afectado. Foco, campo activo y selección visual viven en el widget. SQLite conserva el estado durable. El controlador de pulso distingue edición, validación, confirmación, guardado en curso, guardado y error por campo. Bloquea doble envío y conserva el borrador ante un fallo. Cambiar de persona invalida las respuestas en vuelo de la selección anterior. Véanse `docs/brand/brandbook.md:265–279` y `crates/store/src/api.rs:39–54`.

La raíz usa `routing/app_shell.dart` con un `PageView` horizontal de tres páginas en el orden **History → Today → Recommendations**. Abre en Today, índice 1. Los tres indicadores inferiores muestran el nombre, exponen selección y acción semántica y permiten cambiar de página con un toque. Desde History o Recommendations, Atrás del sistema vuelve a Today. Desde Today permite salir de la app. La cabecera permanece fija con PULSYS (vuelve a Today), perfil local y Need help. El selector es un placeholder del perfil local y la ayuda abre el aviso de disponibilidad en la etapa 4.

History coloca el espacio de alertas, el resumen del perfil, la tendencia con filtros de 6 h, 24 h y 7 días, su tabla y los indicadores en ese orden. La consulta trae hasta 2 000 puntos efectivos y el filtro UTC inclusivo se aplica en Dart antes del muestreo de hasta 30 puntos. Today reutiliza la figura y las fichas de zonas entre las dos columnas de signos, seguidas del contexto y la entrada del asistente deshabilitada. Recommendations muestra los estados de pulso y zonas de Rust, el aviso de contactar a los médicos solo ante alarma, la advertencia de orientación y la sugerencia pendiente.

`routing/app_router.dart` construye la ruta tipada `MaterialPageRoute<void>` del formulario de pulso, que se abre desde su tarjeta. Al cerrarlo, el contenedor refresca la lectura y el estado y solicita refrescar History. `PopScope` bloquea salir del formulario durante envío y confirmación para conservar la ruta propietaria del diálogo. Volver desde una confirmación conserva el formulario y devuelve el foco al control que la abrió. La figura y la lista reutilizan las mismas fichas, incluidas `back` y `other`. El color lleva icono y texto. La distribución sigue [pantallas-flutter.md](pantallas-flutter.md) y las secciones 1–3 de [inventario-ui-anterior.md](../reviews/inventario-ui-anterior.md).

`main.dart` inicializa FRB, pide el directorio privado al runner mediante un canal de plataforma y construye un solo `RustAdapter`. Pasa el adaptador a `PulsysApp(api: adapter)`; los constructores de rutas lo entregan a sus controladores. El cierre explícito pertenece a la raíz. `app.dart` y los widgets dependen únicamente de `pulsys_api.dart`. Una prueba monta `PulsysApp(api: FakePulsysApi(...))` directamente, programa un aviso y luego un recibo, pulsa **Confirm** y comprueba la misma clave en ambas llamadas. El fake registra invocaciones y permite demorar o fallar una respuesta sin cargar bibliotecas nativas, abrir SQLite ni repetir umbrales de Rust.

El catálogo `app_en.arb` contiene las cadenas visibles, las etiquetas de accesibilidad, las unidades de presentación y la traducción de códigos del núcleo. Los textos aprobados de fichas e informes y la salida de los modelos también se entregan en inglés. La entrada admite composición y pegado con teclado español. El tema usa **Basalt Standard + Flight Documentation**, Barlow Semi Condensed, Public Sans y Roboto Mono, los cuatro estados semánticos y movimiento reducido. Las variantes comentadas del CSS quedan fuera de la conversión. Fuentes en `docs/brand/tokens.css:1–36,50–85`, `docs/brand/brandbook.md:63–87,269–287` y `docs/requisitos-app-movil.md:124–130`.

## 3. Contrato de la fachada y del puente

### Funciones del primer recorrido

Estas son **firmas propuestas**, no funciones disponibles hoy. En E1 `App` contiene un `Store`, la versión de aplicación y un reloj. El llamador Rust elige el reloj. Producción usa UTC del sistema y las pruebas inyectan una función fija o controlada. El reloj se lee una vez por operación. Las mutaciones de la base se realizan exclusivamente en `store`.

```rust
// pulsys_api (lib.rs reexporta las funciones de records.rs)
pub type Clock = Box<dyn Fn() -> Result<Timestamp, ApiError> + Send>;
pub fn open_store(private_dir: &Path, app_version: String, clock: Clock) -> Result<App, ApiError>;
pub fn close_store(app: App) -> Result<(), ApiError>;
pub fn prepare_pulse(app: &App, input: PulseInput) -> Result<PreparedPulse, ApiError>;
pub fn confirm_pulse(app: &mut App, action: PreparedPulse) -> Result<SaveOutcome, ApiError>;
pub fn effective_pulse_history(app: &App, query: HistoryQuery) -> Result<Vec<PulsePoint>, ApiError>;
pub fn zone_status(app: &mut App) -> Result<ZoneSnapshot, ApiError>;
```

`open_store` valida el directorio absoluto privado, crea la carpeta, añade internamente el nombre fijo, migra y devuelve una sesión. Elige WAL y claves foráneas como el almacén actual. En el puente `open_store(private_dir: String, app_version: String) → Session` instala el reloj del sistema. Las demás funciones tienen el mismo nombre y argumentos por valor, con `Session` opaca en lugar de `App`. Dart recibe un `Future` por operación, incluido `closeStore`. El reloj, la conexión y las rutas SQL quedan en Rust. Véanse `crates/store/src/lib.rs:68–96` y `crates/core/src/time.rs:6–31,61–70`.

| DTO público | Campos y semántica |
|---|---|
| `PulseInput` | `key: String` (UUID por acción), `bpm: f64`, `measured_at_ms: Option<i64>`, `note: Option<String>`, `replaces: Option<String>`. El primer recorrido usa `heart_rate`, unidad `bpm` y origen `form`. En una corrección se verifica que el original sea pulso y se conserva su raíz. |
| `PreparedPulse` | Entrada normalizada, clave, hora medida resuelta, destino (perfil local o ID y generación DEMO), avisos `{code, rule_id}` y consecuencia de reemplazar DEMO si procede. El destino queda fijado para impedir que una acción pendiente se aplique a otra persona o a otra DEMO. La preparación valida siempre y escribe cero filas. Una acción sin avisos pasa directamente a guardar tras pulsar **Save**. Con avisos o sustitución de DEMO la UI pide confirmación explícita. |
| `SaveOutcome` | Unión `NeedsConfirmation(PreparedPulse)` o `Saved(PulseReceipt)`. El recibo durable incluye clave, ID de registro, raíz, reemplazado opcional, bpm, unidad, origen, horas medida/anotada, `is_demo` y estado local de salida al guardar. Se obtiene la evaluación actual con una lectura independiente; un error al refrescarla mantiene visible el recibo. |
| `HistoryQuery / PulsePoint` | Intervalo UTC inclusivo `from_ms?, to_ms?` y `limit` (100 por defecto, 1–2 000). Punto con ID, raíz, reemplazado opcional, bpm, unidad, horas medida/anotada, origen y marca DEMO. Solo la versión efectiva de cada raíz. Resultado vacío significa que no hay puntos en ese intervalo. |
| `ZoneSnapshot` | `evaluated_at_ms, is_demo, zones: Vec<ZoneState>`. Cada zona tiene `zone_id`, el estado que devuelve Rust y referencias a sus evaluaciones de síntoma con motivo opcional, fecha, regla/versión, fuente e IDs de evidencia. Incluye la evaluación del pulso como campo separado para el formulario (estado, motivo, fecha, fuente, regla, recuentos y referencia personal). |
| `ApiError` | `code: String, field: Option<String>` y mensaje técnico en inglés sin datos personales. Ejemplos de contrato: `invalid_value/value.bpm`, `not_found/replaces`, `idempotency_conflict/key`, `invalid_schema`, `storage_error`, `closed`, `busy`, `cancelled`, `deadline_exceeded`. La UI decide texto, foco y reintento por código/campo; los avisos son resultados normales. |

En la frontera FFI los instantes son milisegundos Unix UTC con signo, dentro del rango representable de RFC 3339 del núcleo. `None` en la hora medida pide fijarla durante la preparación; `logged_at` lo fija Rust al guardar por primera vez. Mostrar hora local es decisión de formato. El adaptador traduce DTO tipados y un error tipado FRB; conserva campos opcionales, enums y unidades. Ningún `serde_json::Value` ni texto de excepción se usa como protocolo Dart. El pulso admite decimales finitos positivos según `Bpm`. El campo ausente en el formulario, el valor inválido, el aviso de plausibilidad y una serie antigua/sin datos tienen representaciones distintas. Véanse `crates/store/src/api.rs:19–73,92–120`, `crates/store/src/lib.rs:23–59`, `crates/core/src/units.rs:4–30`, `crates/core/src/plausibility.rs:17–36,51–64` y `crates/core/src/evaluate.rs:13–25,39–60`.

### Qué ocurre si se pierde la respuesta al guardar

1. Dart asigna una clave aleatoria a la intención y la conserva al confirmar o reintentar. Antes de despachar la **escritura autorizada**, guarda de forma atómica en un archivo privado la acción preparada y la decisión del usuario. Hay una sola acción pendiente de envío en el corte. Tras un reinicio la recupera hasta recibir el recibo o una respuesta definitiva. Editar el contenido crea otra intención. Cancelar el diálogo termina el borrador. No se persisten borradores de dictado en esta ruta.
2. `store` añade una migración **propuesta v6** con una tabla de operaciones, clave única, huella canónica versionada de los campos normalizados y recibo. El esquema local leído llega a v5. `confirm_pulse` comienza una transacción de escritura y **busca la clave antes de recalcular avisos o cambiar DEMO**. Misma huella devuelve el recibo original incluso si pasó el tiempo o hubo una corrección posterior. Otra huella devuelve `idempotency_conflict`. La huella incluye tipo de operación, destino/raíz de perfil aplicable, reemplazado, bpm, nota, origen y hora medida fijada; excluye hora del reintento y avisos derivados.
3. Para una clave nueva, Rust revalida valores, destino y consecuencias con el estado actual. Si han cambiado los avisos o la consecuencia DEMO, devuelve una preparación actualizada y revierte la transacción. El usuario decide de nuevo. Para una acción aprobada crea el registro, conserva el original al corregir, limpia DEMO si corresponde, encola el registro real y guarda el recibo en **la misma transacción**. Una corrección DEMO sigue siendo DEMO y queda fuera de la outbox. La limpieza de DEMO purga sus recibos con contenido sintético y conserva una marca de clave retirada para impedir que un reintento antiguo lo restaure. El adaptador trata esa clave como acción expirada.
4. Un fallo previo al commit revierte todo. Si se pierde la respuesta posterior, reenviar la misma clave recupera el mismo ID, hora y resultado. Dart elimina su acción pendiente tras conocer el resultado definitivo. Un fallo de lectura de zonas se reintenta por separado. La prueba cuenta una fila de registro y una salida **de entidad registro para ese ID**; el cálculo de estado puede generar salidas de evaluación propias.

Hoy `save_record` efectúa el commit **antes** de `status` y `load_rows`, y la unicidad de la outbox solo cubre el ID de entidad ya asignado. Un UUID nuevo en cada intento deja abierto el duplicado. La clave debe entrar hasta la transacción de `store`. Fuentes en `crates/store/src/records.rs:283–357,381–424,453–485`, `crates/store/src/lib.rs:95–105,304–310,393–401` y `reviews/plan-flutter-astra.md:87–91`.

**Consulta efectiva.** Resolver el ganador de cada raíz con la precedencia del núcleo (incluidos registros futuros y correcciones a ambos lados del intervalo), aplicar los filtros y después ordenar por `measured_at` y `local_order` descendentes. Implementar una consulta indexada en `store` que limite los puntos materializados y cotejar su salida con `EffectiveSet` en fixtures con empate, ramas y corrección fuera de rango. El listado actual carga todas las filas antes de limitar. La tendencia clínica usa registros de pulso, mientras el feed conserva su historial de telemetría por persona. Fuentes en `crates/core/src/effective.rs:35–52,71–103`, `crates/store/src/records.rs:28–31,427–450` y `reviews/plan-flutter-astra.md:46–49`.

**Zonas.** Leer `Store::status` una vez y proyectar sus evaluaciones y zonas. La operación requiere acceso mutable porque persiste evaluaciones cambiadas. El núcleo actual agrega **síntomas por zona** y devuelve `NotEvaluated` donde falta una evaluación. Registrar un pulso por sí solo no cambia el color del pecho. RF-07 describe un alcance más amplio; ampliar la asociación exige una decisión de dominio en Rust. La UI conserva alarmas persistentes y separa evaluación, disponibilidad y recepción por Tierra. Fuentes en `crates/core/src/evaluate.rs:405–417`, `crates/store/src/records.rs:453–485,488–518`, `docs/requisitos-app-movil.md:58–66` y `docs/brand/brandbook.md:65–85`.

### Hilo, cierre y tareas largas

La sesión del bridge posee un emisor hacia **un hilo Rust** que abre y conserva el único `App/Store/Connection`. Cada mensaje lleva su canal de respuesta. El hilo ordena lecturas y escrituras. Cola llena devuelve `busy` y sesión cerrada devuelve `closed`. Volver a primer plano reutiliza la sesión. Cerrar impide nuevas entradas, drena operaciones SQLite ya aceptadas y espera la terminación del hilo fuera de la UI. El sistema puede matar el proceso sin ejecutar el cierre; la garantía de guardado es el commit y su recibo. En E1 las pruebas llaman directamente a la fachada síncrona. Véanse `crates/store/src/lib.rs:68–85` y `reviews/plan-flutter-astra.md:42–44`.

En E4/E5 una descarga, exportación extensa o inferencia obtiene un `TaskId` desde `api::tasks`. El bridge ofrece `watch_task(id) → Stream<TaskEvent>` y `cancel_task(id) → Future<CancelAck>`. La suscripción entrega el estado actual incluso si la tarea ya terminó. Cada evento lleva ID, fase, progreso cuando se conoce y un solo resultado terminal. Cancelar dos veces es válido. Un trabajador aparte toma una instantánea o lotes acotados, suelta SQLite durante red/inferencia y encola solo la persistencia final. Comprueba token y plazo entre lotes. La cancelación anterior al commit limpia temporales; un commit ya completado se informa como completado. Al salir del recorrido se ignoran eventos tardíos y se libera el micrófono. Al cerrar la sesión se cancelan y esperan los trabajadores antes de cerrar la base. Aprovechar el control existente del SLM y cargar un solo motor pesado a la vez. Fuentes en `src-tauri/src/lib.rs:633–656`, `crates/slm/src/service.rs:543–567` y `docs/requisitos-app-movil.md:123–126`.

## 4. Destino de los 32 comandos registrados

Cada fila corresponde a una entrada de `src-tauri/src/lib.rs:723–756`, en el orden del registro. Las funciones de destino son propuestas dentro de `pulsys_api`. **E1** fija la fachada de pulso, **E2** la conecta con Flutter, **E3** termina la pantalla vertical, **E4** cubre los recorridos de datos y **E5** incorpora los motores y servicios nativos. La columna «Destino» indica qué comportamiento se conserva, se fusiona o se descarta; todos los envoltorios de comandos del host se retiran del producto.

| Línea | Comando Tauri | Función de `crates/api` | Etapa | Destino |
|---:|---|---|---|---|
| 724 | `voice::speak_text` | `voice::synthesize` | E5 | Conservar síntesis local; el plugin reproduce el audio. |
| 725 | `voice::speak_cancel` | `tasks::cancel` | E5 | Fusionar cancelación por ID y detener la reproducción asociada. |
| 726 | `voice::voice_release` | `voice::release_tts` | E5 | Conservar liberación del motor de síntesis. |
| 727 | `voice::transcribe_audio` | `voice::transcribe` | E5 | Conservar Whisper con audio binario acotado. |
| 728 | `voice::stt_release` | `voice::release_stt` | E5 | Conservar liberación del reconocedor. |
| 729 | `interpret_text` | `assistant::interpret` | E5 | Proponer campos con intérprete determinista y respaldo opcional; guardar exige confirmación. |
| 730 | `explain_indicator` | `assistant::explain_indicator` | E5 | Explicar la evaluación seleccionada con sus fuentes. |
| 731 | `slm_status` | `assistant::model_status` | E5 | Conservar disponibilidad y estado del motor. |
| 732 | `slm_cancel` | `tasks::cancel` | E5 | Fusionar en cancelación por ID. |
| 733 | `slm_release` | `assistant::release_model` | E5 | Descargar el modelo y liberar memoria. |
| 734 | `core_status` | Ninguna | E2 | Descartar la cadena diagnóstica. Abrir, guardar y releer prueba el enlace. |
| 735 | `get_profile` | `crew::local_profile` | E4 | Conservar perfil opcional, marca DEMO y versiones en DTO tipado. |
| 736 | `log_record` | `records::{prepare_pulse, confirm_pulse}` → `records::{prepare_record, confirm_record}` | E1 / E4 | Pulso primero; extender por indicador con el mismo guardado transaccional. |
| 737 | `correct_record` | `records::{prepare_pulse, confirm_pulse}` con `replaces` → `records::{prepare_record, confirm_record}` | E1 / E4 | Fusionar la corrección con su flujo de guardado; conservar cadena y original. |
| 738 | `list_records` | `records::effective_pulse_history` → `records::list_records` | E1 / E4 | Primero tendencia efectiva; después filtros y consulta explícita de versiones sustituidas. |
| 739 | `get_status` | `records::zone_status` → `status::overview` | E1 / E4 | Proyección inicial de zonas y pulso; después estado general, alertas, indicadores y sincronización. |
| 740 | `raise_event` | `events::raise` | E4 | Evento urgente confirmado, ID estable y recibo local. |
| 741 | `list_events` | `events::list` | E4 | Cronología con filtros y límite. |
| 742 | `load_demo` | `demo::load_scenario` | E4 | Sustituir el escenario en una transacción. |
| 743 | `clear_demo` | `demo::clear` | E4 | Limpiar solo el conjunto sintético. |
| 744 | `sim_list_astronauts` | `simulation::list_astronauts` | E4 | Conservar el cliente de simulación. |
| 745 | `sim_import` | `simulation::import_as_demo` | E4 | Descargar y validar antes de aplicar el lote DEMO. |
| 746 | `import_crew_member` | `crew::import_local_as_demo` | E4 | Incorporar la telemetría ya guardada del tripulante como DEMO. |
| 747 | `record_telemetry` | `telemetry::record_batch` | E4 | Conservar lote y procedencia en la tabla de telemetría. |
| 748 | `list_telemetry` | `telemetry::list` | E4 | Consultar por tripulante, indicador e intervalo. |
| 749 | `verify_telemetry` | `telemetry::verify_chain` | E4 | Verificar la cadena local y devolver el resultado tipado. |
| 750 | `recommendations::recommendations_cached_crew` | `ground::cached_snapshot` | E4 | Leer última copia y su hora de recepción sin red. |
| 751 | `recommendations::recommendations_sync_backend` | `ground::sign_in_and_fetch_snapshot` | E4 | Conservar inicio de sesión y descarga del roster del servicio DEMO. |
| 752 | `recommendations::recommendations_refresh_backend` | `ground::refresh_snapshot` | E4 | Refrescar con sesión válida y conservar la última copia completa ante fallo. |
| 753 | `recommendations::recommendations_generate` | `assistant::generate_recommendation` | E5 | Generar para la persona y el contexto seleccionados con modelo preparado. |
| 754 | `recommendations::recommendations_cancel` | `tasks::cancel` | E5 | Fusionar en cancelación por ID. |
| 755 | `export_json` | `reports::export_json` | E4 | Escribir incrementalmente a un temporal privado y entregar el archivo completo. |

Los módulos de destino se abren conforme llega su etapa. La autenticación y descarga de roster actual son distintas de la futura subida de la outbox y descarga de análisis por cursor. Se reservan `ground::push_pending` y `ground::pull_changes` para ese contrato. Se preservan por separado perfil local, DEMO y copia de Tierra, con hora y procedencia visibles. El lector HTTP limita bytes durante la descarga antes de deserializar. Referencias en `src-tauri/src/lib.rs:490–536,608–672`, `src-tauri/src/recommendations.rs:125–179,186–209,242–254`, `crates/sync/Cargo.toml:8–15` y `docs/requisitos-app-movil.md:99–112`.

## 5. Servicios de Android e iOS

Los dos plugins serán **paquetes Flutter propios**, con contrato Dart y realizaciones Kotlin/Swift nuevas. Los comandos de plugin se registraban fuera de la lista de 32. Las rutas privadas y el enlace de Rust pertenecen al runner desde E2; los plugins llegan en E5. Véase `src-tauri/src/lib.rs:683–692`.

| Paquete | Contrato y responsabilidad |
|---|---|
| `app/packages/local_speech/` | Métodos de disponibilidad local, permiso, inicio, fin y cancelación; eventos tipados por sesión (escuchando, parcial, procesando, final, error). Kotlin comprueba que existe reconocimiento **en el dispositivo** y libera el micrófono al pausar. Swift pide autorización y exige reconocimiento local disponible para el idioma. Ambos manejan interrupción, plazo, final único y resultados tardíos. En API 26 el modo de reconocimiento Android usado por la referencia no está disponible. El formulario siempre funciona; la captura PCM para Whisper se ofrece solo con el motor y paquete preparados. La transcripción queda en borrador y el usuario confirma los campos. Captura/reproducción pertenecen al plugin; STT/TTS offline pertenecen a `voice`. |
| `app/packages/slm_assets/` | Métodos para elegir un origen local, abrirlo con permiso del SO y copiar por bloques a staging privado; eventos de progreso y cancelación. Kotlin maneja URIs del selector Android y Swift el acceso temporal a archivos seleccionados. `api::models` centraliza manifiesto, descarga previa opcional, espacio, tamaño, SHA-256 y compatibilidad base/adaptador. Activa **el conjunto completo** cambiando atómicamente una referencia de versión en el mismo volumen. Conserva la versión anterior ante interrupción y libera staging fallido. Cada capacidad instala sus propios archivos: Whisper se puede preparar sin GGUF. Los bytes del modelo no atraviesan Dart. |

En E5 los recordatorios locales se programan en Kotlin/Swift desde `platform/device/`, con ID estable para cancelar o reprogramar; la prueba comprueba entrega con la app cerrada. Un resultado de síntesis/transcripción tampoco exige transferir audio en Base64: usar bloques PCM binarios con formato, frecuencia, límite de bytes y duración validados antes de reservar memoria. Véanse `crates/local-speech/android/src/main/java/com/pulsys/speech/LocalSpeechPlugin.kt:55–103,137–173`, `plugins/slm-assets/android/src/main/java/SlmAssetsPlugin.kt:16–35,40–65`, `src-tauri/src/voice.rs:377–404,463–501` y `docs/requisitos-app-movil.md:55,123–129`.

## 6. Pruebas y comandos de aceptación

Son comandos **para la implementación futura**. Su ejecución en Pulsys Flutter está **NO VERIFICADA**. Los comandos Cargo parten de la raíz. Los comandos Flutter y el generador parten de `app/`. En los comandos de integración se sustituye cada parámetro entre ángulos por el ID real de `flutter devices`. La prueba de reinicio requiere dos lanzamientos que usen el mismo directorio privado y la misma instalación.

| Dónde | Comando | Qué debe demostrar |
|---|---|---|
| Rust, E1 | `cargo test --locked -p pulsys-core -p pulsys-store -p pulsys-api` | Reloj fijo; sin datos, inválido, antiguo, aviso y cancelación con cero escrituras; reabrir archivo real; mismo ID tras respuesta perdida o dos confirmaciones concurrentes; conflicto con igual clave y carga distinta; corrección cruzando el filtro y original conservado; v5 sintética → v6 y rechazo de esquema futuro. |
| Rust, E2 en adelante | `cargo test --workspace --locked`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` | Probar también el worker, cierre con operaciones pendientes y mapeo de error. Ejecutar sobre los miembros explícitos de §1. En E5 probar cancelación, liberación, tiempo máximo y activación interrumpida de modelos. |
| Widgets | `flutter test test/widgets` | Fake sin Rust: entrada, doble toque, fallo/reintento con misma clave, foco y confirmación, navegación atrás, semántica de zona, tabla de tendencia y texto ampliado. |
| Golden | `flutter test test/golden` | Baselines versionados con fuentes locales, tamaño lógico, escala, reloj y animación fijos. Cubrir aviso, dato ausente, antiguo, alarma y selección de zona. Solo ante cambio visual revisado se regeneran con `flutter test --update-goldens test/golden` en la misma imagen CI. |
| Android antiguo | `flutter test integration_test/pulse_flow_test.dart -d <avd26-arm64>` | AVD **API 26 arm64 de poca RAM**. Registro manual offline → confirmar → corregir → historial efectivo → zona. Comprobar persistencia real y formulario disponible sin voz/modelos. |
| Android actual | `flutter test integration_test/pulse_flow_test.dart -d <avd-actual>` | Mismo recorrido en la imagen estable actual fijada por CI. Registrar nivel API, ABI y RAM de ambos AVD. El binario admite arm64 y x86_64 para emulación. |
| Reinicio real Android | `flutter build apk --debug`, `adb -s <serial> install -r build/app/outputs/flutter-apk/app-debug.apk`, `flutter run -d <serial> --use-application-binary=build/app/outputs/flutter-apk/app-debug.apk`, `adb -s <serial> shell am force-stop com.pulsys.app`, `adb -s <serial> shell monkey -p com.pulsys.app 1` | En cada AVD guardar un pulso sintético de valor y hora conocidos, detener el proceso desde otra terminal y verificar el mismo registro en History al relanzar. Repetir tras instalación sobre una versión previa de Pulsys y con red deshabilitada. |
| CI macOS arm64 | `flutter build ios --simulator --debug`, `flutter test integration_test/pulse_flow_test.dart -d <udid-simulador>`, `flutter build ios --release --no-codesign` | Arrancar previamente el simulador disponible en Xcode. Ejecutar la ruta offline y reabrir el almacén. La última orden prueba enlace para dispositivo arm64. Firma e instalación en iPhone XS son investigación aparte. |
| Bindings y perfil básico, CI limpia | `flutter pub get --enforce-lockfile`, `flutter_rust_bridge_codegen generate`, `flutter gen-l10n`, `flutter analyze`. Desde la raíz, `git status --porcelain --untracked-files=all` y `cargo tree -p pulsys-bridge -e features --locked`. | Fallar CI ante diferencias o archivos generados nuevos en un checkout limpio. El grafo básico enlaza SQLite y excluye motores pesados. En trabajos separados compilar perfil equipado por ABI y guardar el resultado de enlace de llama/sherpa. |

El recorrido E3 añade TalkBack en ambos emuladores, selección de zonas pequeñas o por lista y teclado español. Se revisan los anuncios y el orden de foco manualmente además de los tests de semántica. Registrar API y memoria del AVD, arranque, pico de RAM, p95 de fotograma y tamaño por perfil. Las cifras describen el emulador y su máquina anfitriona. VoiceOver y las condiciones de audio/memoria del XS requieren evidencia específica. Referencias en `reviews/plan-flutter-astra.md:46–50`, `docs/requisitos-app-movil.md:29–36,123–129` y la [guía de pruebas de integración de Flutter](https://docs.flutter.dev/testing/integration-tests).

## 7. Propiedad de archivos y orden de creación

Cada fila tiene **un único escritor por archivo** durante la etapa. Los nombres designan responsabilidades asignables, no personas confirmadas. El integrador entrega primero el esqueleto y el contrato. Un cambio de firma vuelve al dueño de la fachada y el integrador regenera después. Las otras tareas usan el contrato existente y entregan las dependencias que necesitan al integrador. Fundamento en `docs/plan-flutter-migracion.md:63–75,138–145` y `reviews/plan-flutter-astra.md:24–44`.

| Escritor | Archivos en propiedad |
|---|---|
| Integrador de compilación y puente | Todos los `Cargo.toml`, `Cargo.lock`, todos los `pubspec.yaml` y sus locks, configuración y versiones de Flutter/FRB, `crates/bridge/**`, `app/flutter_rust_bridge.yaml`, `app/lib/platform/generated/**`, cabeceras FFI, configuración Gradle/Xcode, locks nativos, CI y `app/lib/main.dart`. Solo este rol ejecuta generadores que tocan esos archivos. |
| Responsable Rust de datos | `crates/api/{src,tests}/`, `crates/store/src/` (incluye migración y pruebas), cambios de dominio en `crates/core/src/` cuando corresponda. Los módulos de E4/E5 dentro de `api` los integra este mismo rol. |
| Responsable del adaptador Dart | `app/lib/platform/pulsys_api.dart`, `rust_adapter.dart` y `device/`. Cierra con UI el contrato Dart antes de que se implemente el fake. |
| Responsable de UI y pruebas de presentación | `app/lib/app.dart`, `features/`, `routing/`, `design/`, `l10n.yaml`, `lib/l10n/**`, assets y `test/**` (fake y PNG incluidos). Solo este rol actualiza las referencias golden. |
| Responsable de integración móvil | `app/integration_test/**`, código Kotlin/Swift del runner y fuente de los dos plugins. Solicita al integrador el registro de paquetes, permisos declarativos y cambios de proyecto. |

**E1 termina con Rust comprobable desde Cargo.** Primero el integrador registra `pulsys-api` y fija el workspace. Después el responsable de datos implementa apertura, preparación/confirmación, historial y zonas, la migración de recibos y las pruebas de §6. Entrega firmas públicas y ejemplos tipados de éxito, aviso y fallo. Criterio de cierre: repetir tras reabrir produce el mismo recibo con una sola escritura clínica; una corrección conserva el original. Flutter y los generados empiezan en E2.

**E2 termina con una app instalada que lee su propia escritura sin red.** Con las firmas E1 cerradas, el integrador crea `app/{android,ios}`, `pulsys-bridge`, configuración, enlace nativo y CI. El bridge produce biblioteca compartida para Android y biblioteca estática para iOS. El integrador fija **FRB 2.11.1 exacta** en paquete Dart, crate Rust y CLI (`cargo install flutter_rust_bridge_codegen --version 2.11.1 --locked`). Las tres publicaciones existen; la compatibilidad de la combinación con el Flutter, Rust, NDK y Xcode de este proyecto está **NO VERIFICADA** hasta E2. Se versionan los generados Dart, Rust y cabecera iOS, con locks y SDK fijados; CI regenera con la misma versión. Fuentes: [paquete Dart](https://pub.dev/packages/flutter_rust_bridge/versions/2.11.1), [crate Rust](https://docs.rs/flutter_rust_bridge/2.11.1/flutter_rust_bridge/) y [generador](https://docs.rs/crate/flutter_rust_bridge_codegen/2.11.1).

Con el adaptador Dart acordado, UI puede construir la pantalla mínima de pulso con el fake mientras el integrador conecta FRB. Al reunirlos deben pasar widgets, una primera referencia golden e integración en los dos AVD y en el simulador de CI macOS. Entregables de E2: librería enlazada, sesión abierta, guardado/lectura tras cierre, app básica con recursos ingleses, pruebas y resultado por arquitectura de los enlaces opcionales. E3 completa tendencia y figura accesible; E4 incorpora los demás recorridos de datos; E5 añade voz, modelos y recordatorios; E6 empaqueta los perfiles probados.

## 8. Riesgos y decisiones pendientes

| Asunto | Qué falta comprobar o acordar |
|---|---|
| Entorno de ejecución | **NO VERIFICADO:** runner que pueda ejecutar la imagen Android API 26 arm64 de poca RAM, API/ABI exactas del AVD actual, RAM asignada, versiones finales de Flutter/Dart, NDK, Rust y Xcode. El integrador registra IDs de imagen y versiones antes de validar E2. Android parte de minSdk 26; el objetivo iOS es 15, sujeto a enlace y runtime disponibles en CI. |
| iPhone XS | **NO VERIFICADO:** versión de iOS instalada, identidad de firma disponible y vía de sideload usable. La evidencia de CI distingue ejecución en simulador y compilación arm64 sin firma. El responsable de la investigación del XS documentará instalación, VoiceOver, micrófono y memoria por separado. |
| Integridad y escala | Ensayar en base Pulsys sintética la migración, respuesta perdida, DEMO reemplazada y guardado interrumpido. Medir consulta de historia con 30 y 2.000 entradas, el mismo límite de puntos y correcciones fuera de intervalo. La lectura de estado aún puede cargar la misión completa. `crates/store/src/records.rs:427–469`. |
| Cobertura de zonas | Acordar con el dueño del dominio qué indicadores adicionales pertenecen a qué zona si se amplía RF-07. Hasta entonces se usa el agregador de síntomas existente y se ofrece la lista de 14 zonas. `docs/requisitos-app-movil.md:58–60`, `crates/core/src/evaluate.rs:405–417`, `ui/src/ui/body-map.ts:11–27`. |
| Voz y modelos | **NO VERIFICADO:** RAM/latencia y binarios por arquitectura, capacidad de dictado inglés offline, licencia y salida inglesa de las versiones elegidas. Entregar medida por capacidad y por perfil. La falta de un paquete deja accesible el registro manual. `docs/requisitos-app-movil.md:124–130,155–161`. |
| Tierra | El equipo de Tierra debe asignar dueño al contrato de autenticación, subida idempotente y análisis por cursor, y especificar los acuses que permiten mostrar **Received** y **Reviewed**. Hasta verificar esos acuses, una acción local informa **Saved locally** y el estado real de la cola. `docs/requisitos-app-movil.md:99–112`, `docs/brand/brandbook.md:54–61`. |
