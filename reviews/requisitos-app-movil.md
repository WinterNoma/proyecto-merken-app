# Revisión de Sol — requisitos app móvil

## Veredicto

**Sí, se puede empezar con un prototipo acotado**, después de corregir la definición de las alertas, la línea base y RNF-04. El borrador cubre el flujo central del [reto: registrar indicadores, evaluar el estado y orientar acciones](https://www.spaceappschallenge.org/2026/challenges/health-monitoring-software/), pero sus M actuales y la doble entrega Android/web son demasiado ambiciosos para aprobarlos sin conocer el tiempo y el equipo disponibles. La demo será más convincente con un recorrido completo y explicable que con muchos indicadores parcialmente resueltos.

## Problemas

| Severidad | Sección o ID | Problema | Propuesta concreta |
|---|---|---|---|
| Alta | RF-09/10 | Una desviación estadística personal no demuestra peligro médico; permanecer cerca de la base tampoco demuestra salud. | Separar **cambio respecto a referencia** de **alarma clínica**. Identificar las reglas experimentales como demostrativas. |
| Alta | RF-09 | “Primeros N registros” puede incorporar enfermedad o adaptación inicial; faltan tamaño mínimo, dispersión y actualización. | Para la demo, usar una referencia preflight ficticia y congelada. Definir origen, ventana, exclusiones y comportamiento con pocos datos o dispersión cero. |
| Alta | RF-10 | Faltan estados para ausencia de datos, mediciones antiguas y referencia insuficiente. | Añadir **sin evaluación disponible**, con motivo y fecha. Nunca convertir ausencia de información en verde. |
| Alta | RF-10; investigación 00 | Presenta CO₂ y radiación como excepciones suficientes para clasificar salud. Sus límites tienen ámbitos diferentes. | Separar exposición ambiental, exposición acumulada y evaluación de síntomas; conservar unidades, periodo y ámbito de cada norma. |
| Alta | RF-02/11 | El formulario 0–10 no permite registrar claramente desmayo, pérdida de campo visual o un hallazgo de trombo. | Añadir eventos explícitos y acceso directo a “Necesito ayuda”. Diferenciar síntoma, sospecha del usuario y hallazgo médico confirmado. |
| Alta | RF-11/14 | “Avisar a médicos” queda incompleto cuando la comunicación tarda o falla. No hay protocolos concretos definidos. | Mostrar escalado inmediato al responsable médico a bordo y acceso al protocolo aplicable sin conexión; indicar la comunicación con Tierra como acción adicional. |
| Alta | RNF-04; §6 | RNF-04 incluye almacenamiento y sincronización dentro de un núcleo puro; el árbol los coloca correctamente en `store`. | Reescribir RNF-04: dominio y evaluación puros en `core`; persistencia, transporte y reloj en adaptadores. |
| Alta | Prioridades M; §8 | Hay 15 RF imprescindibles, además de WASM, aprendizaje de Rust y requisitos operativos aún abiertos. | Congelar una plataforma de presentación y el recorte propuesto abajo antes de repartir implementación. |
| Media | RF-01/03; RNF-08 | Registrar todas las constantes, síntomas, sueño e ingesta en tres toques y 15 s no es un criterio definido ni creíble. | Aplicar esa meta a **un registro frecuente concreto**, desde una pantalla inicial especificada; medirlo con usuarios. |
| Media | Registro; RF-13/15 | Faltan hora de medición frente a hora de ingreso, procedencia, calidad y corrección de registros. | Definir esos campos, unidades y registros parciales. Conservar qué datos y versión de regla produjeron cada evaluación. |
| Media | RF-12 | “Cruzar” radiación con síntomas o sueño con cognición puede acabar atribuyendo causas sin evidencia. | Mostrar coincidencias temporales con sus límites; posponer inferencias causales y puntuaciones compuestas. |
| Media | RNF-10; RF-20 | Exportación voluntaria y sincronización diferida no tienen una política conjunta. “Recibido” podría confundirse con atención médica. | Exigir envío explícito y distinguir preparado, pendiente, entregado y revisado. Rotular toda recepción simulada. |
| Media | RF-16 | “Contacto confidencial” promete una capacidad que no está diseñada; guardar localmente no garantiza confidencialidad. | Posponer el canal real y definir qué información de ánimo se incluye voluntariamente en los informes. Usar datos ficticios en la demo. |
| Media | §6; RNF-04 | El riesgo “bajo” de Tauri no está respaldado por una build Android con persistencia y exportación. | Hacer esa prueba primero, fijando versiones, dispositivo y ABI. Tratar WASM como una segunda integración. |
| Baja | RNF-07; RF-07 | Tamaño táctil y figura coloreada no prueban uso con guantes ni explican cómo agregar estados por zona. | Verificar tamaño efectivo en Android, ofrecer lista accesible y hacer que una alarma tenga precedencia explícita sobre estados favorables. |

## Opinión sobre Rust + Tauri 2

**Mantendría la elección.** Aprender Rust es una motivación válida, y usarlo para validación, referencias personales y evaluación explicable le da una responsabilidad real. La seguridad de memoria no demuestra corrección médica ni convierte el prototipo en software de misión.

- **RNF-01:** razonable como disciplina para código de producción, pero prohibir `panic!` y `unwrap()` no garantiza ausencia de cuelgues. Incluir `expect()` y operaciones que puedan fallar; usar `Result` en operaciones falibles y traducir errores a mensajes útiles. Permitir aserciones en tests.
- **RNF-02:** buena inversión. Los tipos deben representar también conceptos: presión sistólica/diastólica, dosis frente a tasa y tiempo. Validar al entrar desde JSON y SQLite; un `Deserialize` derivado puede eludir constructores. Distinguir datos inválidos de valores extremos potencialmente reales.
- **RNF-03:** es el requisito más valioso. “Mismos datos” debe incluir referencia, contexto, versión de reglas y tiempo de evaluación recibido como argumento. Definir orden y redondeo; comprobar igualdad de decisiones entre nativo y WASM.
- **RNF-04:** conservar portabilidad del núcleo, pero exigir ahora únicamente las plataformas elegidas para entregar. **iOS queda fuera de la validación desde Windows:** Tauri requiere macOS y Xcode para desarrollarlo. [Prerequisitos oficiales](https://v2.tauri.app/start/prerequisites/).
- **RNF-05:** funcionamiento local, sí. “Pequeño” y “bajo consumo” necesitan medidas; arranque inferior a un segundo debería ser objetivo S hasta fijar dispositivo, build release y definición de arranque.
- **RNF-06:** mantener tests, formato y Clippy. Acotar `proptest` a invariantes importantes: datos faltantes no producen verde, una alarma no queda anulada por una tendencia y ninguna entrada aceptada rompe la evaluación. Estos tests verifican implementación, no validez clínica.

La división `core/store/seed/src-tauri/ui` es razonable. `seed` puede empezar como generador sencillo de fixtures reproducibles; no necesita convertirse en otra biblioteca compleja. La interfaz puede gestionar navegación y presentación, mientras Rust determina estados, motivos y acciones.

**Android desde Windows está documentado**, pero requiere SDK, JDK, NDK y targets compatibles. Con `rusqlite bundled`, SQLite se compila desde C mediante `cc`: hay que comprobar el compilador y enlazador del NDK para el target Android; tener MSVC instalado no basta para demostrar esa ruta. El riesgo es de integración, no una incompatibilidad confirmada. [Tauri](https://v2.tauri.app/start/prerequisites/), [rusqlite](https://github.com/rusqlite/rusqlite#notes-on-building-rusqlite-and-libsqlite3-sys).

**WASM necesita un adaptador propio:** el navegador no ejecuta los comandos Tauri. Propondría una API TS común con adaptadores `invoke` y WASM, un wrapper Rust pequeño para `wasm-bindgen` y persistencia web separada. Compartir evaluación no resuelve carga del módulo, exportación, persistencia ni disponibilidad offline del sitio. [Comandos Tauri](https://v2.tauri.app/develop/calling-rust/), [wasm-bindgen](https://wasm-bindgen.github.io/wasm-bindgen/examples/hello-world.html).

Elegir **un** generador: `ts-rs` para DTO compartidos sencillos, o `tauri-specta` si interesa generar también llamadas Tauri. `serde` solo no genera tipos TS y los tipos generados no sustituyen validación en Rust. Fijar versiones compatibles y comprobar el contrato serializado. [ts-rs](https://docs.rs/ts-rs/latest/ts_rs/), [tauri-specta](https://github.com/specta-rs/tauri-specta).

Esta es una revisión documental y de fuentes; **no he ejecutado ni validado builds** de este proyecto.

## Opinión sobre el enfoque médico

La referencia personal es útil para mostrar evolución, pero **no encuentro en los archivos una validación del algoritmo que convertiría mediana, dispersión y tendencia en verde/ámbar/rojo**. No puedo recomendar N, porcentajes de desviación o multiplicadores clínicos: serían parámetros inventados sin una justificación adicional. Tampoco corresponde prohibir todo límite absoluto por principio; cada regla debe justificar su aplicabilidad al indicador y contexto.

Los dos valores citados existen, pero RF-10 pierde matices importantes:

- **CO₂: 3 mmHg** corresponde al promedio de una hora de presión parcial en el volumen habitable. Una lectura manual aislada no demuestra ese promedio ni diagnostica la causa de una cefalea. [NASA, requisito V2 6004](https://www.nasa.gov/reference/6-0-natural-and-induced-environments-vol-2/).
- **Radiación: 600 mSv** corresponde a dosis efectiva acumulada de carrera durante exposición espacial. No es un umbral de síntomas agudos, ni permite convertir directamente una lectura en mGy a mSv. Además, la fuente contempla otros límites: “los únicos claros” es una conclusión demasiado amplia. [NASA, protección radiológica](https://www.nasa.gov/wp-content/uploads/2023/12/ochmo-tb-020-radiation-protection.pdf).

Los eventos seleccionados tienen fundamento: NASA describe síncope asociado a intolerancia ortostática, pérdida de campo visual como afectación funcional en SANS y trombosis que puede ser asintomática. Eso **no valida automáticamente un protocolo universal de triaje implementado por la app**. [Intolerancia ortostática](https://www.nasa.gov/wp-content/uploads/2023/12/ochmo-tb-019-orthostatic-intolerance.pdf), [SANS](https://ntrs.nasa.gov/citations/20220014606), [trombosis](https://www.nasa.gov/organizations/ochmo/risk-of-venous-thromboembolism-during-spaceflight/).

Cada regla debería incluir fuente y apartado exacto, ámbito, versión y condición de uso: **evidencia publicada, protocolo aprobado o heurística de demo**. Citar una enfermedad no demuestra que el disparador elegido esté validado. El contraste entre Sol e Hydrogen ayuda a revisar textos; no sustituye revisión clínica. El aviso “no diagnóstico” tampoco corrige una recomendación incorrecta.

## Recorte recomendado para la demo

**M final propuesto:**

- Android en un dispositivo acordado, con funcionamiento y persistencia local comprobados al cerrar y reabrir.
- Registro parcial de pulso, sueño, ánimo y cefalea; eventos de alarma en un flujo separado. Instrucciones breves de registro.
- Figura humana SVG sencilla con alternativa en lista, texto e iconos; sin 3D.
- Referencia preflight ficticia, tendencia descriptiva y estado “datos insuficientes”; reglas demostrativas identificadas y alarmas por evento con precedencia.
- Explicación de cada evaluación, fuente disponible offline y acción de escalado claramente presentada.
- Historial y una gráfica reutilizable para los indicadores seleccionados.
- Informe **en texto**, con vista previa y exportación voluntaria; sin PDF ni recepción ficticia presentada como real.
- Datos sintéticos reiniciables y tres escenarios: seguimiento habitual, cambio persistente y evento de alarma.

Bajaría a S las constantes restantes, masa/ingesta, CO₂, recordatorios y WASM; pospondría cruces automáticos, sincronización y selector de entornos. Fotos, farmacia, gamificación y 3D quedan fuera de esta entrega. **Si se exige un enlace web ejecutable, WASM sube a M y habrá que reducir nuevamente indicadores o entregables.**

## Orden de trabajo sugerido

1. **Cerrar entrega y alcance:** plazo, integrantes, dispositivo y necesidad de enlace web. Convertir el recorrido de presentación en criterios de aceptación.
2. **Resolver la integración más incierta:** APK desde Windows que llama Rust, escribe y recupera SQLite y exporta texto. Si web es obligatoria, probar también una evaluación WASM desde Vite.
3. **Definir el contrato médico y de datos:** referencia, calidad, tiempos, eventos, precedencias y fuentes; separar claramente demostración de protocolo clínico.
4. **Implementar el núcleo y sus pruebas:** fixtures, validación y evaluaciones explicables; comprobar los mismos escenarios en cada plataforma comprometida.
5. **Completar el recorrido visual:** registrar → entender → actuar → preparar informe. Ensayarlo offline, con reinicio, datos incompletos y fallo de guardado, antes de añadir extras.

## Dudas para el equipo

- ¿Cuál es la fecha y hora límite, cuántas personas implementan y qué experiencia tienen con Rust/Tauri?
- ¿Los jueces necesitan un enlace funcional o aceptan APK y video? ¿Qué Android concreto usarán?
- ¿Quién revisará clínicamente las acciones? ¿Hay protocolos utilizables o serán ejemplos claramente rotulados?
- ¿Existe una referencia preflight disponible o toda la información será sintética?
- ¿Qué aportes del Google Doc son imprescindibles y qué escenario quieren que recuerden los jueces?