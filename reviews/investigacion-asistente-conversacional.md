# Investigación de viabilidad del asistente conversacional por voz

Fecha de consulta y revisión local 18 de noviembre de 2026. Incluye la ampliación sobre voces naturales solicitada tras la interrupción por cuota.

## 1. Qué hice

Revisé el flujo anterior, los contratos, las gramáticas, los modelos locales y la propuesta de migración Flutter.
Contrasté las capacidades de conversación, reconocimiento y síntesis con model cards y documentación primaria.
Propongo un diálogo de varios turnos gobernado por Rust, con extracción local acotada y contenido médico autorizado.
La viabilidad en 2–3 GB depende de reducir el modelo, secuenciar los motores y medir en los dispositivos objetivo.

## 2. Los archivos que toqué

- Únicamente `reviews/investigacion-asistente-conversacional.md`.

## 3. Los comandos y comprobaciones que ejecuté

| Operación real | Resultado |
|---|---|
| Lectura inicial de `AGENTS.md`, sección Asistente de `docs/pantallas-flutter.md` y sección 4 de `reviews/inventario-ui-anterior.md` | Completada antes del análisis. |
| Primeros intentos con `exec_command`, usando `rg` y PowerShell | Fallaron al crear el proceso, con `CreateProcessAsUserW failed: 5 (Access is denied.)`. Continué mediante `node_repl` y `fs.readFile`. Tras el cambio de cuenta, PowerShell y `rg` funcionaron. |
| `git status --short` | Pasa. Registró modificaciones y archivos ajenos ya presentes, incluidos cambios en `app/`, `crates/store/` y `docs/estructura-flutter.md`. |
| `rg -n` sobre `crates/slm/`, `crates/local-speech/`, `crates/voice/`, documentación, informes y empaquetado | Completado. Una búsqueda conjunta devolvió código 1 al incluir `app/packages/local_speech`, que todavía no existe. Las coincidencias de los demás archivos se obtuvieron. |
| Lectura binaria acotada de cabeceras GGUF y SHA-256 incremental con Node `fs.createReadStream` y `crypto.createHash("sha256")` | Pasa para los tres GGUF locales. Los tamaños y hashes coinciden con los manifiestos embebidos de V5, base intérprete y Granite. La lectura de cabecera no ejecuta inferencia. |
| `JSON.parse` de `crates/slm/assets/policy-vectors.json` y recuento de `granite_eval.jsonl` | Pasa. 30 vectores, todos con `all_values_synthetic=true`. Hay 10 de cada procedencia declarada MANUAL, SIMULATED y GARMIN_BACKEND_SAMPLE. El archivo de evaluación del intérprete contiene 47 casos. |
| `fs.stat` de los assets locales de Kokoro | Pasa. V1.0 int8 tiene 114.202.993 B de modelo y 28.200.960 B de voces. V0.19 int8 tiene 134.186.977 B y 5.755.904 B respectivamente. |
| Consultas web de documentación NASA, Android, Apple, Flutter, WHO, NICE, model cards y repositorios de los motores | Completadas. La página completa oficial del reto devolvió 502 en varios intentos. Su resultado indexado sí identifica objetivo y título. Se indica esta limitación en el punto 1. |
| Validación documental mediante Node, con lectura UTF-8 y comprobación de rutas/rangos, títulos y espacios finales | Pasa. Cuatro bloques de informe, siete temas solicitados, 64 referencias locales válidas, cero espacios al final de línea y cero caracteres Unicode inválidos o separadores invisibles. |
| `git diff --check -- reviews/investigacion-asistente-conversacional.md` | Código 0, sin diagnósticos. Al ser un archivo nuevo, se complementó con la validación directa anterior. |
| `git diff --no-index --check -- NUL reviews/investigacion-asistente-conversacional.md` | Código 1 en la comparación con el archivo vacío. Única salida, aviso de conversión futura LF a CRLF. Sin diagnósticos de espacios. |
| `git status --short -- reviews/investigacion-asistente-conversacional.md` | Código 0. Aparece como archivo nuevo sin seguimiento. |

**NO EJECUTADO** — Inferencias nuevas, benchmarks, escucha comparativa, pruebas en modo avión, RAM, temperatura y latencia móvil. Esta entrega es una investigación documental.

**NO EJECUTADO** — Tests, Cargo fmt/Clippy y Dart format/analyze. El único archivo creado es Markdown. Los resultados históricos que se citan abajo pertenecen a los informes y fichas revisados.

La revisión final comprobó los siete puntos solicitados y 64 referencias locales con número de línea. Todas apuntaban a archivos y rangos existentes. Se comprobó UTF-8 y se corrigió un carácter de separación invisible.

## 4. Investigación, decisiones y asuntos pendientes

### 1. Qué modelos y datos tenemos realmente

**El asistente solicitado es viable como desarrollo adicional. El repositorio contiene componentes de extracción y recomendación que pueden reutilizarse, pero falta el estado de conversación clínica y el contenido que lo sostenga.** La propuesta de pantalla ya pide diálogo unificado, voz offline, carga bajo demanda y reglas médicas en Rust. La fachada Flutter actual todavía carece de la integración del SLM y de voz. Véanse `docs/pantallas-flutter.md:40–54`, `docs/estructura-flutter.md:138`, `docs/estructura-flutter.md:187`, `crates/api/Cargo.toml:8–11` y `crates/bridge/Cargo.toml:11–15`.

#### Inventario y capacidad

| Pieza | Tamaño y cuantización | Contexto y tarea comprobada | Alcance conversacional |
|---|---|---|---|
| Space-health V5 | Qwen3-1.7B, unos 1.700 millones de parámetros. Base GGUF Q4_K_M de 1.107.409.120 B y LoRA F16 de 34.892.576 B, escala 1. Total 1,142 GB decimales / 1,064 GiB. | Perfil evaluado de 1.024 tokens y hasta 220 nuevos, temperatura 0, thinking desactivado. Explica en español un hallazgo estructurado del motor por llamada. | Falta entrenamiento y evaluación para entrevista, seguimiento o conversación clínica abierta. |
| Intérprete usado por el host anterior | La misma base Qwen3 Q4_K_M, sin el adaptador V5. Comparte archivo en disco, 1,107 GB. | JSON Proposal limitado por GBNF, una entrada de texto por llamada. La configuración general tiene contexto 1.024 y máximo de salida 220. | Extrae registros en una pasada. La aplicación tendría que enlazar los turnos. |
| Granite 4.0 350M disponible localmente | GGUF Q4_K_M, 236.985.760 B, unos 237 MB / 226 MiB. | Candidato alternativo de extracción. La ficha y cabecera declaran 32.768 tokens, pero el motor local usa 1.024 por defecto. | Capacidad general de instrucciones y roles. Su utilidad de extracción en Pulsys sigue siendo insuficiente según la evaluación histórica. |
| Wellbeing V7 | Misma base Qwen3-1.7B y otro LoRA F16 declarado de 34.892.576 B. | Runtime de 1.536 tokens, reserva 160 tokens de salida, greedy. Elige/copía una recomendación de un catálogo cerrado en inglés. | El contrato exige un snapshot y una tarjeta. Faltan historial conversacional y preguntas de síntomas. |

Evidencia de tamaños, identidades y perfiles en `crates/slm/src/service.rs:25–81`, `crates/slm/src/engine.rs:10–14`, `crates/slm/src/llama.rs:34–64`, `crates/slm/src/runtime.rs:18–30,61–95` y `externo/space-health-slm-v5-20261114/runtime.json:4–17`. La Q4_K_M combina tipos de cuantización y conserva metadatos y tensores de distinta precisión. Su tamaño en disco no equivale a cuatro bits por cada byte de todo el paquete.

Comprobé los hashes locales de la base `4fce4f8510d16a058a9e6eef90d6ffd992135259e0fcadd42e69b06d1243dea1`, V5 `ca3d2b23e39547d0ebafb2e47793b4fba3a2943978e59205328dac47f5dd750f` y Granite `771c588a49607f274a2bba3185733607ebe6f74b996ab90e2d6bee0d98bcec52`. El hash de V7 `521290d5207de280a7fc1ebc7517138e2dd994405347cc451a16fa60d5ffc2ef` está declarado en `crates/slm/src/runtime.rs:27–30`, pero su GGUF no apareció en los directorios de modelos inspeccionados. **NO VERIFICADO** — Contenido e integridad de los pesos V7 en esta copia.

Hay tres límites de contexto diferentes. La model card oficial de Qwen3-1.7B anuncia 32.768 tokens, la cabecera de la base local contiene `qwen3.context_length=40960` y el contrato de ejecución V5 fue probado con 1.024. Mantendría el presupuesto evaluado durante el piloto. Llevarlo a decenas de miles aumenta memoria y cambia las condiciones de prueba. La ficha oficial acredita 28 capas y 8 cabezas KV. [Qwen3-1.7B](https://huggingface.co/Qwen/Qwen3-1.7B).

#### Entrenamiento y significado de space-health

La base Qwen3 tiene preentrenamiento general multilingüe. El equipo publica alrededor de 36 billones de tokens y 119 idiomas o dialectos para la familia. Ese corpus general incluye datos web, documentos y material sintético, y requiere distinguirse del ajuste propio de Pulsys. La mezcla exacta de textos médicos recibidos por esta base concreta queda **NO VERIFICADO**. [Informe técnico de Qwen3](https://arxiv.org/abs/2505.09388), [publicación del equipo Qwen](https://qwenlm.github.io/blog/qwen3/).

V5 añade dos etapas, QLoRA NF4 y ajuste de paridad sobre valores de la base Q4 expandidos a BF16. El ajuste tiene 17.432.576 parámetros entrenables. Declara 965 ejemplos train, 64 valid y 40 test, todos SIMULATED. El objetivo es preservar hechos, unidades, procedencia, acciones del motor y citas suministradas. La ficha afirma que los documentos son políticas locales y que se entrenó sin incorporar material clínico NASA/CSA a los pesos. Véase `externo/space-health-slm-v5-20261114/MODEL_CARD.md:71–87,89–140`.

Los 40/40 del adaptado, frente a 2/40 de la base, corresponden al explicador y a verificaciones técnicas acotadas de patrones sintéticos. El paquete contiene ejemplos, predicciones y métricas, pero el corpus completo de entrenamiento y su generador no aparecieron entre sus archivos. **NO VERIFICADO** — Reproducción independiente de los 965 ejemplos, diversidad clínica y capacidad de entrevista. La paridad Rust 40/40 y el pico cercano a 2 GB constan como pruebas de escritorio históricas en `docs/slm-motor-escritorio.md:25–38`.

Granite publica un modelo pequeño de instrucciones, soporte EN/ES y entrenamiento supervisado con datos públicos permisivos, sintéticos internos y curación humana. Su licencia es Apache-2.0. En esta copia hay pesos y manifiesto, sin un ajuste propio de extracción de síntomas. El cierre de F2 comunica recall de 2/40 valores esperados para Granite y 13/40 para Qwen3 base. Es una medida directa del intérprete sobre una versión del conjunto, y falta medir la recuperación adicional del servicio después del filtrado. Estos resultados impiden dar por suficiente cualquiera de los dos. Véanse `reviews/slm-f2-cierre.md:22–23`, `reviews/slm-f2-revision-3.md:22–23` y `crates/slm/examples/data/granite_eval.jsonl`. [Model card de Granite 350M](https://huggingface.co/ibm-granite/granite-4.0-350m).

V7 contiene diez tarjetas de ocio, planes de ejercicio previamente autorizados y escalado. Rust normaliza los datos, decide la respuesta esperada y exige igualdad exacta de la salida. Sus 30 vectores congelados son sintéticos incluso cuando la etiqueta dice MANUAL o GARMIN_BACKEND_SAMPLE. Son evidencia del contrato. **NO VERIFICADO** — Tamaño, particiones, generador, licencia del corpus y métricas de entrenamiento del adaptador V7, cuya model card no apareció aquí. Véanse `crates/slm/assets/contract-v7.json:2–69`, `crates/slm/assets/policy-vectors.json:1`, `crates/slm/src/contract.rs:172–219` y `docs/offline-recommendations.md:22–34`.

El NOTICE atribuye Apache-2.0 a la base y describe V7 como entrenamiento sintético. El archivo QWEN-LICENSE contiene la licencia de la base. **NO VERIFICADO** — Concesión explícita de licencia para redistribuir los adaptadores V5/V7 y sus corpus. El nombre del proyecto y el NOTICE no resuelven por sí solos esa entrega. Véanse `plugins/slm-assets/android/src/main/assets/slm/NOTICE.txt:1–4` y `plugins/slm-assets/android/src/main/assets/slm/QWEN-LICENSE.txt`.

#### El dataset del reto

El reto oficial pide reunir indicadores y ayudar al astronauta a evaluar y actuar sobre su salud. Su resultado indexado confirma ese objetivo. La descarga de la página completa y su pestaña de recursos falló con 502, así que la lista oficial completa queda **NO VERIFICADO** en esta consulta. La reproducción del organizador HackAthlone enumera cinco recursos documentales, tres NASA HRP y dos CSA, y pide instrucciones comprensibles para quien carece de conocimientos médicos. Constituye evidencia secundaria de la lista. [Reto oficial](https://www.spaceappschallenge.org/2026/challenges/health-monitoring-software/), [página del organizador local](https://hackathlone.com/challenges/health-monitoring-software).

En el repositorio revisado, space-health identifica el paquete sintético del compañero. Falta evidencia de que V5 o V7 procedan de un dataset clínico oficial del reto, o de conversaciones de astronautas con médicos. OSDR ofrece datos de investigación con condiciones por estudio y acceso controlado para parte de los datos humanos. Habría que seleccionar estudios concretos, documentar procedencia y evaluar su pertinencia para la tarea. [OSDR, preguntas frecuentes](https://science.nasa.gov/reference/osdr-faq/), [términos de OSDR](https://science.nasa.gov/reference/osdr-help-terms-and-conditions/).

#### Qué hace hoy cada llamada

El flujo antiguo de chat pide la hora por reglas TypeScript y solo comprende pulso. El servicio Rust interpreta una entrada, identifica residuos y llama al SLM condicionalmente. La gramática admite síntomas con zona, puntuación y descripción, además de indicadores, negaciones y tiempo ambiguo. Faltan campos tipados específicos para inicio, evolución, síntomas asociados y la pregunta pendiente. Véanse `reviews/inventario-ui-anterior.md:183–218`, `crates/slm/src/service.rs:434–479` y `crates/slm/src/interpreter.rs:36–110,430–499`.

El explicador integrado actualmente selecciona hasta tres IDs de frases autorizadas. Rust renderiza sus textos después de validar la selección. `crates/slm/fichas/fichas.json:1` sigue siendo `[]`, por lo que devuelve hechos con `no_fichas`. Esta ruta difiere de la prosa libre con la que se entrenó el V5. La evaluación V5 40/40 tampoco acredita que el adaptador elija bien IDs bajo la gramática actual. Véanse `crates/slm/src/explain/prompt.rs:9–15`, `crates/slm/src/explain.rs:131–157` y `crates/slm/src/service.rs:482–505,927–943`.

La base puede recibir mensajes con roles e historial por su formato general, pero el runtime actual crea un contexto nuevo por generación y recibe system + una petición. La conversación de varios turnos necesita una nueva política y un estado explícito. La gramática garantiza formato, mientras la validación y las pruebas deben comprobar significado. Véanse `crates/slm/src/llama.rs:93–109,299–339`, `crates/slm/src/interpreter.rs:106–110` y `crates/slm/src/gbnf.rs`. [Gramáticas de llama.cpp](https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md).

### 2. Diseño recomendado del diálogo y del triaje

**Recomiendo que Rust mantenga la entrevista y la decisión de urgencia, y que un modelo pequeño convierta cada respuesta libre en candidatos verificables.** Las preguntas, explicaciones y consejos pueden componerse con frases autorizadas y variaciones revisadas. La sensación de consultor depende de recordar lo ya dicho, hacer una pregunta relevante cada vez, explicar por qué se necesita y permitir corregirla.

| Responsabilidad | Diseño propuesto |
|---|---|
| Flutter y voz | Una conversación compartida entre texto y audio, transcripción visible y editable, pregunta actual, resumen de lo entendido, Guardar/Corregir y acceso permanente a ayuda. |
| Estado Rust | Un episodio con síntoma y zona, inicio aproximado/confirmado, intensidad y escala, evolución, síntomas asociados, positivos/negativos/desconocidos, contexto de misión y signos vitales con fecha, unidad, calidad y procedencia. |
| Extracción | Reglas existentes primero. SLM sobre el turno y la pregunta pendiente si hay ambigüedad. Cada campo conserva cita, turno de origen y confirmación. Una respuesta “desde anoche” permanece aproximada hasta aclararla. |
| Política de preguntas | Rust decide qué falta para la rama activa. Prioriza señales de alarma y después los datos que cambian la orientación. Conserva lo ya respondido y permite “no sé” y “no puedo medirlo”. |
| Evaluación | Reglas versionadas con fuente, población, condiciones de aplicación y resultado trazable. El estado desconocido impide una valoración tranquilizadora automática. |
| Lenguaje de salida | Selección de IDs o plantillas autorizadas. Una reformulación neuronal opcional necesita validación adicional y evaluación de fidelidad. Para urgencias, el mensaje y la acción se muestran inmediatamente desde Rust. |
| Persistencia y Tierra | El borrador se confirma antes de convertirlo en registros. El paquete de consulta conserva síntomas, tiempos, evidencia, reglas y pendientes. El usuario ve el estado real de envío y recepción. |

Esto encaja con la separación de dominio y E/S, SQLite y el puente definidos en `docs/decisiones/002-base-de-datos.md:6–14`, `docs/decisiones/003-nucleo-rust.md:7–14`, `docs/estructura-flutter.md:91–138` y `docs/requisitos-app-movil.md:23–25,97–103`. El diseño requiere extender los contratos. El documento de estructura ya fija `app/packages/local_speech/`, pero faltan allí el DTO del episodio conversacional y sus operaciones. Deben acordarse los nombres antes de implementarlos.

#### Recorrido ilustrativo con cefalea y mareo

1. El astronauta dice “Me duele la cabeza desde anoche y estoy mareado”. Se conserva la transcripción y se extraen dos síntomas, zona de cabeza y comienzo aproximado. La hora actual se usa para anotar el encuentro. El inicio del dolor conserva la expresión “desde anoche”.
2. El asistente reconoce ambos síntomas y pregunta primero por características de alarma de la rama de cefalea, con vocabulario cotidiano. Por ejemplo, comienzo brusco, desmayo, dificultad nueva para hablar o debilidad nueva. Las preguntas concretas deben pertenecer a una ficha revisada.
3. Si aparece una señal de alarma, Rust interrumpe la entrevista rutinaria, presenta el protocolo local aprobado, indica avisar al responsable médico a bordo y prepara el contacto con los médicos de vuelo. La interacción sigue disponible mientras se intenta comunicar.
4. Si esa rama permite continuar, pregunta por intensidad, evolución y qué significa “mareado” para esa persona, por ejemplo sensación de giro o de desvanecimiento. Después solicita solo las mediciones o antecedentes que la regla aplicable necesite, indicando cómo medirlos desde una ficha.
5. Si faltan datos, ofrece una aclaración concreta. Si la persona sigue sin poder explicarlo o el síntoma queda fuera de las fichas, mantiene “información insuficiente” y propone revisión médica. La entrevista dispone de salida manual y evita repetir indefinidamente la misma pregunta.
6. Al cerrar muestra el resumen confirmado, la valoración orientativa, consejos permitidos y qué cambio obliga a pedir ayuda. El resultado incluye las preguntas sin responder y las fuentes utilizadas.

El orden es una propuesta de producto pendiente de validación médica. NICE CG150, sección 1.1.1, respalda investigar o derivar ante cefalea brusca, nuevos déficits neurológicos, alteración de conciencia y otros signos. NASA-STD-3001 Vol. 1 Rev. C, secciones 3.1.1 y 3.1.5, fundamenta la atención bajo el equipo de medicina de vuelo y según las limitaciones de misión. **NO VERIFICADO** — Tiempos de escalado, ventanas de reevaluación y protocolo concreto de misión para estas ramas. [NICE CG150, recomendaciones](https://www.nice.org.uk/guidance/cg150/chapter/Recommendations), [NASA-STD-3001 Vol. 1 Rev. C](https://standards.nasa.gov/sites/default/files/standards/NASA/C/nasa-std-3001-vol-1-rev-c-signature.pdf).

Un dato vital normal aislado aporta contexto, pero las reglas de alarma del episodio siguen activas según su protocolo. Las reglas de puntuación de síntomas existentes están marcadas `demo_heuristic`. Su uso como triaje clínico requiere revisión. El documento también señala limitaciones de postura, ejercicio, glucosa y resolución de eventos. Véase `docs/api/umbrales-core.md:7–8,52–61,77–85`.

#### Comparación con conversación libre

| Aspecto | Rust con extracción acotada | Modelo que dirige libremente la conversación |
|---|---|---|
| Memoria entre turnos | Episodio compacto y pregunta pendiente. El prompt puede mantenerse pequeño. | Historial creciente, más KV y coste de relectura. Resumirlo puede perder detalles. |
| Preguntas | Seleccionadas por la rama y los datos que cambian la orientación. | La cobertura depende del modelo y del prompt. Puede omitir o repetir preguntas. |
| Urgencia | Regla trazable y accesible incluso si falla el modelo. | Decisión probabilística que exige validación clínica abierta mucho más amplia. |
| Consejos | Textos con fuente, versión y condiciones de uso. | Riesgo de añadir causas, dosis, cifras o certezas ajenas a las fichas. |
| Naturalidad | Requiere redactar bien las preguntas y transiciones y evaluar comprensión. | Puede ser más variada, con fidelidad y consistencia pendientes de demostrar. |
| Validación | Casos por rama, campos, negaciones, contradicciones, urgencia y transiciones. | Espacio de diálogo abierto y más difícil de cubrir. |

V5 y V7 ya delimitan la responsabilidad médica del modelo. Ampliarlos a conversación libre exige un corpus, un contrato y una evaluación diferentes. Para 2–3 GB empezaría por comparar Granite 350M ajustado y Qwen3-0.6B Q4_K_M con thinking desactivado, manteniendo las reglas y plantillas durante la evaluación. Qwen3-0.6B ronda 0,48 GB según la variante Q4_K_M publicada por Unsloth. Su precisión en esta nueva tarea queda **NO VERIFICADO**. [Paquete Qwen3-0.6B](https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/blob/main/Qwen3-0.6B-Q4_K_M.gguf), [model card y licencia](https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/raw/main/README.md).

#### Uso de la palabra diagnóstico

La salida de la app debería llamarse “Health assessment” o “Guidance”, según la decisión de idioma. Si el usuario pide un diagnóstico, se explica que se puede ordenar lo que cuenta, señalar signos que requieren ayuda y ofrecer orientación con las fichas disponibles. El cierre describe observaciones y grado de información disponible. Un diagnóstico confirmado por un médico puede mostrarse atribuido a ese profesional y con su fecha.

Las posibles causas solo se ofrecen cuando una ficha revisada autoriza esa explicación y documenta sus condiciones. Por ejemplo, una ficha puede explicar la relación entre adaptación espacial y síntomas, o entre CO₂ y cefalea. La aplicación debe evitar deducir que ese astronauta tiene esa causa a partir de la coincidencia. El Medical Technical Brief OCHMO-MTB-004 Rev. A se presenta expresamente como referencia de conocimiento. RF-15 y los límites del SLM ya restringen diagnósticos y dosis. Véanse `docs/requisitos-app-movil.md:81–97`. [SAS, OCHMO-MTB-004 Rev. A, 25-09-2025](https://www.nasa.gov/wp-content/uploads/2025/09/ochmo-mtb-004-space-adaptation-sickness-sas.pdf), [CO₂, OCHMO-TB-004](https://www.nasa.gov/ochmo-tb-004-carbon-dioxide-2/).

### 3. Fuentes médicas, fichas y permisos de reutilización

| Fuente | Uso adecuado en Pulsys | Reutilización y límites confirmados |
|---|---|---|
| NASA HRP Risks y Five Hazards | Fichas educativas sobre radiación, gravedad alterada, aislamiento, distancia y ambiente cerrado. Enlaces a evidencia por riesgo. | Atribuir NASA y revisar autoría y avisos de cada documento. Las páginas generales aportan contexto y asociaciones. Las reglas de urgencia necesitan una fuente clínica o protocolo específico. |
| NASA-STD-3001 Vol. 1 Rev. C, 15-09-2023 | Responsabilidad médica, capacidades por misión, historia del encuentro, comunicación y formación. Secciones 3.1.1, 3.1.5 y 5.2.1. | Documento gubernamental público. Aplicabilidad y adaptación dependen del programa. Permite fundamentar el marco de atención, con umbrales individuales pendientes de protocolo. |
| OCHMO Technical and Medical Briefs | Fichas sobre adaptación espacial, CO₂, intolerancia ortostática, trombosis y otras condiciones. Registrar número, revisión, fecha y página. | Cotejar figuras y material de terceros. OCHMO-MTB-004 Rev. A declara finalidad de referencia. Los consejos para personas requieren revisión y adaptación. |
| NASA ExMC Medical Scenarios e IMPACT/AMCL | Seleccionar condiciones, escenarios de prueba y recursos de misión. ExMC 20230006550 ofrece escenarios de capacidades médicas. | Son fuentes públicas de planificación. La lista de condiciones requiere complementarse con protocolos asistenciales vigentes. La disponibilidad y licencia de una checklist operacional completa ISS quedan NO VERIFICADO. |
| WHO-ICRC Basic Emergency Care, 2018 | Estructurar historia SAMPLE, evaluación inicial y preguntas comprensibles adaptadas con supervisión clínica. | El PDF declara CC BY-NC-SA 3.0 IGO. Exige atribución, compartir adaptaciones bajo sus condiciones y resolver el uso comercial por separado. |
| WHO/ICRC/MSF Interagency Integrated Triage Tool | Prioridad y signos de alto riesgo como referencia para revisión médica. | Diseñado para triaje en centros sanitarios. La ficha FAQ exige personal sanitario capacitado. Licencia del archivo IITT exacto que se incorporaría queda NO VERIFICADO y debe leerse en ese recurso. |
| NICE CG150 | Cefalea y criterios públicos de evaluación/derivación, especialmente 1.1.1. | Se puede citar la recomendación y enlazarla. La licencia abierta publicada cubre uso en Reino Unido. La incorporación, adaptación, traducción o uso con IA fuera de ese ámbito requiere verificar la autorización aplicable. |
| CSA, recursos enlazados por el reto | Educación sobre radiación e investigación espacial. | Los términos CSA distinguen reproducción no comercial, comercial y derechos de terceros. La autorización para la adaptación concreta de fichas queda NO VERIFICADO. |

Fuentes de la tabla en [NASA Risks](https://www.nasa.gov/hrp/risks/), [NASA Hazards](https://www.nasa.gov/hrp/hazards/), [registro del estándar](https://standards.nasa.gov/standard/NASA/NASA-STD-3001_VOL_1), [colección OCHMO](https://www.nasa.gov/ochmo/hsa-standards/ochmo-technical-briefs/), [ExMC Medical Scenarios](https://ntrs.nasa.gov/citations/20230006550), [AMCL y antecedentes de checklists](https://ntrs.nasa.gov/api/citations/20220003136/downloads/NASA-T_1.pdf), [WHO Basic Emergency Care y PDF](https://www.who.int/publications/i/item/9789241513081), [licencia del manual BEC](https://hlh.who.int/docs/librariesprovider4/clinical-care/who-icrc-basic-emergency-care.pdf), [IITT](https://www.who.int/tools/triage), [FAQ de IITT](https://cdn.who.int/media/docs/default-source/integrated-health-services-%28ihs%29/csy/iitt/iitt-faq.pdf), [NICE, términos](https://www.nice.org.uk/terms-and-conditions), [NICE, reutilización e IA](https://www.nice.org.uk/reusing-our-content/nice-uk-open-content-licence) y [CSA, términos](https://www.asc-csa.gc.ca/eng/terms.asp).

Los trabajos del Gobierno de EE. UU. realizados por sus empleados en sus funciones oficiales carecen de protección de copyright estadounidense según 17 USC 105. La publicación en un sitio NASA también puede incluir material de contratistas o terceros con derechos propios. Por ello guardaría el aviso del documento concreto, atribuiría la fuente y evitaría presentar la app como avalada por NASA. La licencia de un artículo médico alojado o citado en NTRS requiere comprobarse por separado. [17 USC 105](https://uscode.house.gov/us/usc/t17/s105/), [directrices de uso NASA](https://www.nasa.gov/nasa-brand-center/images-and-media/).

Propongo fichas con ID estable, idioma, texto aprobado, población, condiciones de aplicación, síntomas y contraindicaciones pertinentes, preguntas autorizadas, acción y destino de escalado, fuente con sección/página, versión, licencia y fecha/responsable de revisión. Las reglas incluyen además unidades, calidad y antigüedad admisibles de la medida y tratamiento de desconocidos. La estructura actual de `Ficha` conserva ID y frases aplicables por Status, así que necesita ampliarse o vincularse a metadatos para esta trazabilidad. Véanse `crates/slm/src/explain/selection.rs:223–327` y `docs/requisitos-app-movil.md:86`.

Las investigaciones 01–06 del repositorio sirven de índice de riesgos y referencias. El resumen ya advierte que numerosas condiciones carecen de un umbral simple. Las frecuencias poblacionales y los límites ambientales deben conservar su contexto. En particular, un límite de cabina para CO₂ no permite diagnosticar una causa individual de cefalea. Esta investigación evita trasladar automáticamente sus cifras a reglas nuevas. Véanse `docs/investigacion/00-resumen.md:20–36`, `docs/investigacion/03-gravedad.md:3–18` y `docs/investigacion/04-ambiente-cerrado.md:21–31`.

**NO VERIFICADO** — Conjunto clínicamente aprobado de umbrales por síntomas, duración tolerable, plazo de contacto con Tierra, intervalo de reevaluación y posibles causas autorizadas para este asistente. La aprobación exige un responsable médico y un contexto de misión. La investigación determina las fuentes y la arquitectura para incorporarlos.

### 4. Reconocimiento y síntesis nativos offline desde Flutter

Los mínimos configurados son Android API 26 e iOS 15.0. Son límites del sistema operativo, mientras la capacidad offline de voz depende también de motor, idioma, modelo instalado y dispositivo. Véanse `app/android/app/build.gradle.kts:18` y `app/ios/Runner.xcodeproj/project.pbxproj:381,517,569`.

| Plataforma y operación | Disponibilidad en el mínimo | Inglés y español offline | Comprobación necesaria |
|---|---|---|---|
| Android SpeechRecognizer | La API genérica existe en API 26. `createOnDeviceSpeechRecognizer` e `isOnDeviceRecognitionAvailable` llegan en API 31. | En API 26 el servicio puede disponer de paquetes locales, pero `EXTRA_PREFER_OFFLINE` es una preferencia que el proveedor puede ignorar. Disponibilidad garantizada EN/ES en cualquier móvil API 26 queda NO VERIFICADO. | API 31+ fuerza el servicio on-device y comprueba idioma. API 33+ permite consultar soporte instalado/pendiente y descargar modelos antes de desconectarse. API 26 requiere un conjunto de dispositivos previamente comprobado o ASR empaquetado. |
| iOS SFSpeechRecognizer | La capacidad on-device y la obligación de mantener el audio local están disponibles desde iOS 13. | Comprobar `supportsOnDeviceRecognition` por locale y establecer `requiresOnDeviceRecognition=true`. Las notas de iOS 15 listan EN de AU/CA/GB/US y chino mandarín, y omiten ES en su limitación conocida. Soporte español en el iOS 15 concreto queda NO VERIFICADO. | Comprobar autorizaciones, disponibilidad, locale y funcionamiento real desconectado. El catálogo de dictado del teclado o Siri requiere distinguirse del Speech framework usado por la app. |
| Android TextToSpeech | Sí en API 26, incluido catálogo Voice desde API 21. | Puede sintetizar EN/ES localmente con una voz instalada cuyo `isNetworkConnectionRequired` sea false. El catálogo depende del motor. | Inicialización, locale, instalación, selección explícita de voz y prueba desconectada. Calidad y paquetes se detallan en el punto 7. |
| iOS AVSpeechSynthesizer | Sí en iOS 15. `enhanced` existe desde iOS 9 y `premium` desde iOS 16. | Elegir una voz EN/ES disponible en `speechVoices()` y preparar sus assets antes del viaje. La voz instalada y su rendimiento en el teléfono quedan NO VERIFICADO. | Selección por identificador y locale, prueba offline, interrupciones y salida de audio. |

Documentación en [SpeechRecognizer](https://developer.android.com/reference/android/speech/SpeechRecognizer), [RecognizerIntent](https://developer.android.com/reference/android/speech/RecognizerIntent), [RecognitionSupport](https://developer.android.com/reference/android/speech/RecognitionSupport), [Apple, capacidad local](https://developer.apple.com/documentation/speech/sfspeechrecognizer/supportsondevicerecognition), [Apple, solicitud local](https://developer.apple.com/documentation/speech/sfspeechrecognitionrequest/requiresondevicerecognition), [notas originales de iOS 15](https://developer.apple.com/documentation/ios-ipados-release-notes/ios-ipados-15-release-notes.md), [TextToSpeech](https://developer.android.com/reference/android/speech/tts/TextToSpeech), [AVSpeechSynthesizer](https://developer.apple.com/documentation/avfaudio/avspeechsynthesizer/) y [catálogo de voces Apple](https://developer.apple.com/documentation/avfaudio/avspeechsynthesisvoice/speechvoices()).

En Flutter usaría el paquete local previsto `app/packages/local_speech/`, con métodos y eventos tipados por sesión, Kotlin/Swift y canales de plataforma. El audio permanece en la capa nativa. Flutter entrega el texto final al dominio Rust por el puente ya adoptado. Rust conserva el estado del episodio. Es una integración pendiente, con ubicación definida en `docs/estructura-flutter.md:50,138,187`. [Canales de plataforma Flutter](https://docs.flutter.dev/platform-integration/platform-channels).

Android requiere RECORD_AUDIO, hilo principal para SpeechRecognizer, una sesión por turno y `destroy()` al acabar. iOS requiere permiso de micrófono y autorización de Speech, incluidas NSMicrophoneUsageDescription y NSSpeechRecognitionUsageDescription. La segunda clave se exige también con reconocimiento on-device. Al terminar se cancela la tarea, se detiene captura y se liberan el tap y los recursos de audio. Los eventos tardíos deben ignorarse por ID de sesión. [Contrato Android](https://developer.android.com/reference/android/speech/SpeechRecognizer), [permiso de Speech Apple](https://developer.apple.com/documentation/speech/asking-permission-to-use-speech-recognition), [clave obligatoria de Speech](https://developer.apple.com/documentation/bundleresources/information-property-list/nsspeechrecognitionusagedescription).

#### Qué existe en crates/local-speech y crates/voice

- `crates/local-speech` es un plugin Tauri Android. La implementación Kotlin exige API 31, comprueba servicio local, usa `createOnDeviceSpeechRecognizer`, entrega parciales y texto final, limita la sesión a 25 segundos y destruye el reconocedor. Al pausar cancela. Véase `crates/local-speech/android/src/main/java/com/pulsys/speech/LocalSpeechPlugin.kt:54–74,86–123,137–173`.
- Su envoltorio Rust expone start y finish. Para destinos distintos de Android devuelve indisponibilidad. Falta integración Flutter, implementación iOS y síntesis nativa en este crate. Véase `crates/local-speech/src/lib.rs:10–69`.
- `crates/voice` contiene STT con sherpa-onnx, Whisper base/small y Canary, y TTS Kokoro. El reconocimiento acepta audio mono f32 acotado de 0,1 a 30 segundos. Su README comunica dificultades de Whisper base con español en las pruebas del proyecto, sin una evaluación móvil bilingüe suficiente. Véanse `crates/voice/README.md:40–65` y `crates/voice/src/stt.rs`.
- Las entradas antiguas `speak_text` y `speak_cancel` existen en el host Tauri. La síntesis actual se analiza en el punto 7. Véase `src-tauri/src/voice.rs:137,246`.

Para cumplir voz offline EN/ES en todos los mínimos, el piloto debe evaluar un ASR local empaquetado. La investigación previa propone Whisper base multilingüe Q5_1 de unos 59,7 MB y Moonshine como competidor. Los assets ONNX int8 que ya se empaquetan para Whisper base suman 160.609.290 B con tokens, por lo que esas variantes tienen costes distintos. La precisión de la transcripción se debe medir antes de elegir. Véanse `docs/investigacion/07-voz-interprete.md:5–11,28–38` y `.github/scripts/bundle_offline_models.py:15–18`. [Archivo Whisper Q5_1](https://huggingface.co/ggerganov/whisper.cpp/blob/main/ggml-base-q5_1.bin).

### 5. Gestión de memoria y riesgo en 2–3 GB

**El V5/V7 actual consume un presupuesto demasiado alto para prometer estabilidad en un teléfono de 2–3 GB.** El almacenamiento de 1,14 GB se suma a KV, buffers de cómputo, allocator, bibliotecas, UI y servicios de voz. Android puede terminar procesos por presión de memoria y iOS aplica límites dinámicos por proceso. El tamaño total anunciado del teléfono está compartido con el sistema y otras aplicaciones. [Gestión de memoria Android](https://developer.android.com/topic/performance/memory/manage-app-memory), [memoria disponible por proceso en Apple](https://developer.apple.com/documentation/os/os_proc_available_memory), [informes jetsam](https://developer.apple.com/documentation/xcode/identifying-high-memory-use-with-jetsam-event-reports).

| Modelo o motor | Datos de memoria disponibles | Conclusión para el piloto |
|---|---|---|
| Qwen3-1.7B Q4 + V5 | Pesos de 1,064 GiB. KV F16 calculada para 1.024 tokens de 112 MiB. Presupuesto orientativo de inferencia 1,5–2,5 GiB, más la UI. Medida histórica Windows cercana a 2,0 GB working set / 1,3 GB privada. | Riesgo alto en 2–3 GB. RAM móvil real NO VERIFICADO. Excluirlo del perfil ligero inicial. |
| Qwen3-1.7B + V7 | Igual orden de pesos. A 1.536 tokens, KV F16 calculada de 168 MiB. | También riesgo alto. Compartir base en disco ahorra almacenamiento, mientras el control de carga debe garantizar una sola instancia residente. |
| Granite 350M Q4_K_M | 226 MiB de pesos. Su configuración pública implica unos 28 MiB de KV F16 a 1.024 tokens. Estimación de planificación de 0,4–0,8 GiB por motor, sujeta a buffers y configuración. | Mejor candidato de memoria. Utilidad de extracción necesita ajuste y evaluación. RAM móvil NO VERIFICADO. |
| Qwen3-0.6B Q4_K_M | Pesos alrededor de 0,48 GB. Su configuración mantiene 28 capas, 8 cabezas KV y head_dim 128, por lo que KV F16 a 1.024 también ronda 112 MiB. Presupuesto inicial estimado de 0,7–1,2 GiB por motor. | Posible competidor de extracción, con calidad y RAM móvil NO VERIFICADO. Reducir parámetros no reduce automáticamente todas las cachés. |
| Whisper base | Referencia publicada por whisper.cpp de unos 388 MB para la variante de su tabla, sin demostrar RAM de Q5_1 ni del ONNX int8 instalado. | Medir la variante seleccionada, cargarla durante captura/transcripción y liberarla antes del SLM. RAM móvil de esa variante NO VERIFICADO. |
| TTS | Nativo, Piper, Kokoro y Kitten tienen perfiles distintos. El punto 7 indica pesos y límites de la evidencia. | Contabilizar también los servicios externos de voz y evitar tres motores activos a la vez. |

Las estimaciones son presupuestos de ingeniería y quedan **NO VERIFICADO** como medidas de dispositivos. La fórmula KV F16 es `2 × capas × tokens × cabezas_KV × dimensión_cabeza × 2 bytes`. Depende del tipo real de caché y backend. Fuentes locales en `reviews/slm-space-health-v5.md:93–101` y `docs/slm-motor-escritorio.md:25–38`. Configuraciones en [Granite 350M](https://huggingface.co/ibm-granite/granite-4.0-350m/blob/main/config.json), [Qwen3-0.6B](https://huggingface.co/Qwen/Qwen3-0.6B/raw/main/config.json) y [tabla de memoria whisper.cpp](https://github.com/ggml-org/whisper.cpp#memory-usage).

#### Ciclo de vida propuesto

1. Al abrir el asistente se comprueban idioma, paquetes verificados y presupuesto de memoria. La carga neuronal se inicia cuando el turno la requiere. Las alertas y fichas quedan accesibles con reglas y texto.
2. Se turnan captura/ASR, extracción y respuesta. En 2–3 GB se liberan los motores propios entre fases. El estado del episodio se conserva en tipos Rust, independiente del contexto neuronal. En hardware validado puede mantenerse caliente un motor pequeño durante un intervalo medido.
3. Una sola tarea de inferencia usa el recurso de modelos. Se conserva la idea de ModelSlot, con liberación antes de cambiar modelo. El coordinador debe abarcar también las rutas V7, ASR y TTS. Compartir un backend no evita que dos modelos distintos asignen memoria.
4. El prompt contiene la pregunta pendiente, los campos necesarios, el turno y las fichas relevantes. Se acota entrada y salida antes de inferir. Si falta contexto se devuelve un error recuperable y se mantiene el episodio.
5. El progreso actual expone fase y cantidad de tokens. Falta entregar texto incremental al usuario. Para voz se sintetizan únicamente frases ya validadas. El JSON parcial y las recomendaciones aún sin validar permanecen fuera de la salida hablada.
6. Cancelar propaga ID y token. Se detienen escucha y reproducción inmediatamente, se solicita cancelar el motor y se espera su retorno antes de liberar recursos nativos. Las llamadas actuales a decode/generate pueden terminar su lote o frase antes de observar la cancelación.
7. Al salir, pausar o recibir presión de memoria se cancelan tareas y se liberan contexto, adaptador, pesos, motor de voz y buffers. Android TTS requiere stop/shutdown y Apple detener la cola y liberar objetos. Si el sistema termina la app, el siguiente arranque debe recuperar únicamente el estado cuya persistencia haya autorizado el usuario.

Las piezas reutilizables están en `crates/slm/src/slot.rs:68–94`, `crates/slm/src/service.rs:543–551,812–833`, `crates/slm/src/control.rs:155–159`, `crates/slm/src/llama.rs:299–383` y `docs/estructura-flutter.md:138`. V7 verifica y carga dentro de la llamada, con cancelación comprobada durante los lotes y generación y plazo de 180 segundos, pero sin chequeos equivalentes durante toda la carga inicial. Véase `crates/slm/src/runtime.rs:61–154`.

Usaría mmap cuando lo soporte el backend y evitaría copiar el GGUF completo a Dart o a un Vec. El mapeo sigue consumiendo páginas residentes al acceder a los pesos. La liberación depende también de cachés, referencias y allocator, así que debe verificarse con métricas. Para audio usaría búfer PCM binario acotado y contrapresión, evitando Base64. A 24 kHz mono f32, 10 segundos suman 960.000 bytes por copia, calculados como `24.000 × 10 × 4`. El coste suele estar dominado por el modelo, pero las colas sin límite acumulan memoria.

La aceptación requiere release físico API 26 de 2 GB, otro de 3 GB, Android actual, iOS 15 y actual. Medir frío/caliente, primer audio/token, tiempo total, pico y memoria después de cerrar, repeticiones, interrupciones y presión de memoria. Android permite `adb shell dumpsys meminfo -d <package>` y eventos de salida donde la versión los soporte. iOS requiere Instruments y revisión de jetsam. Los presupuestos finales se fijan con esos resultados. [dumpsys](https://developer.android.com/tools/dumpsys), [Instruments y memoria](https://developer.apple.com/documentation/xcode/gathering-information-about-memory-use), [respuesta a baja memoria](https://developer.apple.com/documentation/xcode/responding-to-low-memory-warnings).

### 6. Plan por fases, estimación, riesgos y decisiones

Las estimaciones son días de trabajo de una persona con acceso a dispositivos y SDK de ambas plataformas. Incluyen integración y pruebas del alcance indicado. La revisión médica y los permisos de contenido dependen de terceros y pueden ampliar el calendario. Son propuestas de planificación, **NO VERIFICADO** como plazos comprometidos.

| Fase | Trabajo y estimación | Criterio para avanzar | Riesgo principal |
|---|---|---|---|
| A. Contrato y prueba de dispositivos, 3–5 días | Elegir idiomas/locales, dispositivo 2 GB y 3 GB, probar STT/TTS nativos desconectados, medir carga de modelos pequeños y seleccionar variantes. | Matriz real de voz y memoria, con perfil ligero definido y costes de paquetes. | Falta de voz local por idioma, permisos, iOS/Xcode y cierre por memoria. |
| B. Contenido y política, 5–10 días técnicos | Un episodio inicial de cefalea/mareo, preguntas, fichas, fuente y licencia, reglas y salidas. Coordinar revisión médica en paralelo. | Ramas y escalados revisados, desconocidos explícitos, corpus de casos positivos/negativos. | Guías generales insuficientes para la misión y ausencia de revisor. |
| C. Diálogo Rust y texto, 5–8 días | Estado, fusión de turnos, contradicciones, preguntas pendientes, confirmación y puente tipado. | Casos multiturno reproducibles, correcciones conservadas, urgencia accesible sin esperar al SLM. | Contratos nuevos y manejo incorrecto de tiempos o negaciones. |
| D. Extracción pequeña, 5–10 días | Comparar reglas, Granite ajustado y Qwen0.6, preparar entrenamiento si hace falta y evaluar el pipeline completo. | Medición por campo, negación, omisión, invención, recuperación adicional y urgencia. Umbrales de aceptación acordados antes del test. | Corpus sintético repetido, bajo recall, extracción plausible pero incorrecta. |
| E. Voz Flutter y ciclo de vida, 5–8 días | Native STT/TTS, ASR empaquetado donde haga falta, turnos de audio, cancelación, interrupciones y liberación. | Diálogo completo offline en las combinaciones anunciadas, texto editable y recuperación de errores. | Eco, ruido, acento, paquetes ausentes y coexistencia de motores. |
| F. Voces naturales y cierre, 4–7 días | Comparación nativa mejorada/Piper/Kokoro, normalización oral, escucha EN/ES y perfilado de sesiones. | Voz entendible, cifras correctas, tiempos y RAM aceptables en físico y sin crecimiento sostenido. | Prosodia médica, latencia, compatibilidad de voces y licencias. |

Total orientativo secuencial de **27–48 días técnicos**, unas 6–10 semanas laborales para el piloto acotado de ambos sistemas. Mantener las preguntas y respuestas con plantillas en la primera entrega permite validar la conversación mientras se evalúa el extractor. El plan se integra con las etapas 4–6 de migración existentes, especialmente los servicios de plataforma de la etapa 5. Véase `docs/plan-flutter-migracion.md:85–99`.

El conjunto de evaluación debe incluir historias nuevas por hablante y escenario, cambios de tema, negaciones, inicio aproximado, respuestas cortas dependientes de la pregunta, contradicciones, síntomas múltiples, silencio, ruido de ventilación y petición de ayuda. Medir el servicio final después de sus filtros. Para la síntesis, verificar que números, decimales, unidades y negaciones se escuchan como se aprobaron. La regla de urgencia debe conservarse si falla, tarda o se cancela cualquiera de los modelos.

Decisiones que necesita tomar el usuario antes de implementación.

- Confirmar el alcance del primer piloto y qué síntomas se cubren. Propongo cefalea/mareo como episodio inicial por el caso solicitado.
- Elegir los teléfonos de 2 y 3 GB, las versiones mínimas y las ABI que se distribuirán. La disponibilidad de voz debe anunciarse por capacidad comprobada.
- Definir EN como salida inicial conforme al requisito local, o aprobar salida bilingüe. RNF-14 atribuye el inglés al reto, pero esa obligación en la página oficial completa queda **NO VERIFICADO** por el fallo de acceso. Véase `docs/requisitos-app-movil.md:127–130`.
- Designar responsable médico para fichas, posibles causas, prioridad, contacto con Tierra y reevaluación. También definir quién recibe el escalado a bordo cuando la comunicación esté demorada.
- Acordar persistencia de borradores, consentimiento para guardar o enviar el episodio y estado visible de recepción. Las notas privadas y audio necesitan una decisión explícita conforme al flujo previsto.
- Aprobar un perfil ligero con modelo de extracción pequeño y voz secuenciada, y decidir si V5/V7 quedan reservados para teléfonos que superen la prueba.
- Escoger voces y licencia de distribución. Propongo voz nativa mejorada preparada, con el plan B del punto 7 y paquetes instalados antes de quedar offline.

### 7. Voces más naturales offline

**Recomiendo como primera opción las mejores voces nativas locales que superen una prueba auditiva EN/ES en los teléfonos elegidos.** Evitan añadir otro runtime y pesos propios a Pulsys, y ya ofrecen selección de calidad y control de reproducción. El consumo del servicio de voz sigue formando parte del presupuesto total. La naturalidad de una voz concreta en el teléfono de 2–3 GB queda **NO VERIFICADO** hasta escucharla.

#### Comparación de síntesis nativa

| Aspecto | Android TextToSpeech en API 26 | iOS AVSpeechSynthesizer en iOS 15 |
|---|---|---|
| Calidad mejorada | Voice ofrece calidad y latencia declaradas por el motor desde API 21. Filtrar voces locales y comparar las de calidad alta/muy alta. El nivel es metadato del proveedor, con calidad perceptual pendiente de audición. | La calidad enhanced existe desde iOS 9. Las voces premium requieren iOS 16, así que el piloto iOS 15 debe usar default/enhanced. |
| Preparación | El motor puede requerir paquetes de voz. `KEY_FEATURE_NOT_INSTALLED` indica datos faltantes. Instalar antes del viaje y volver a comprobar el resultado. | Apple indica que enhanced se descarga. Prepararla en Ajustes con red y comprobar que aparece en el catálogo. La app enumera y selecciona voces ya disponibles. |
| Inglés y español | Disponibles cuando el motor instalado expone voces de los locales elegidos. Elegir `isNetworkConnectionRequired=false` y confirmar que los datos están instalados. La garantía universal por fabricante queda NO VERIFICADO. | Elegir voces EN/ES que devuelva `speechVoices()`. Idiomas, variantes y descargas exactas en cada dispositivo iOS 15 quedan NO VERIFICADO. Se valida reproduciendo con red desactivada después de reiniciar. |
| Tamaño y RAM | Tamaño de paquete y pico RAM del motor elegido NO VERIFICADO. Los pesos quedan gestionados por el servicio y pueden compartirse con otras apps. | Tamaño de la voz enhanced y pico RAM NO VERIFICADO para el dispositivo objetivo. Los assets los administra el sistema. |
| Latencia móvil | NO VERIFICADO. Medir inicialización, primera muestra audible y finalización por utterance. | NO VERIFICADO. Medir primer audio frío/caliente y manejo de interrupciones. |
| Integración | Kotlin en `local_speech`, selección por nombre/locale, callbacks, stop y shutdown. | Swift en `local_speech`, selección por identificador, cola de utterances y delegado de final/cancelación. |
| Licencia | API del sistema y condiciones del motor instalado. La app llama al servicio. Redistribución de sus paquetes por cuenta propia queda NO VERIFICADO. | API y voces del sistema bajo las condiciones de Apple. La app usa sus voces instaladas. Redistribución de los assets por cuenta propia queda NO VERIFICADO. |

Fuentes en [Android Voice](https://developer.android.com/reference/android/speech/tts/Voice), [instalación y comprobación de datos TTS](https://developer.android.com/reference/android/speech/tts/TextToSpeech.Engine), [TextToSpeech](https://developer.android.com/reference/android/speech/tts/TextToSpeech), [Apple enhanced y disponibilidad desde iOS 9](https://developer.apple.com/documentation/avfaudio/avspeechsynthesisvoicequality/enhanced.md), [Apple premium desde iOS 16](https://developer.apple.com/documentation/avfaudio/avspeechsynthesisvoicequality/premium.md), [selección de voces Apple](https://developer.apple.com/documentation/avfaudio/avspeechsynthesisvoice/speechvoices()) y [descarga de voces mejoradas](https://support.apple.com/en-us/111798).

La preparación de misión debe reproducir al menos un texto de cada locale sin red, guardar el identificador elegido y detectar después si la voz desaparece o cambia. La calidad mejorada no garantiza que se pronuncien bien abreviaturas clínicas. El texto hablado necesita normalización aprobada, pausas naturales y frases cortas. Permitir interrumpir la respuesta para contestar mejora la interacción. Para evitar que el micrófono transcriba al propio asistente, el piloto puede alternar escucha y habla.

#### Comparación de voces neuronales propias

MB son decimales. Los tamaños de archivos incluyen solo lo que se especifica. **NO VERIFICADO** en RAM y latencia significa que falta una medición comparable en un móvil de 2–3 GB con Pulsys. Las afirmaciones de calidad de los autores y sus demos requieren una prueba auditiva propia EN/ES.

| Opción | Modelo y descarga | EN/ES y calidad | RAM y latencia móvil | Licencia e integración |
|---|---|---|---|---|
| Piper/VITS | Voz medium, por ejemplo en_US-joe o es_ES-davefx, unos 63,2 MB ONNX por voz, más config, fonemizador y runtime. Dos voces suman unos 126,4 MB de pesos en disco. | Voces específicas por locale. Habla neuronal local con muestras públicas. Naturalidad y pronunciación de cada voz en Pulsys NO VERIFICADO. | RAM y tiempo al primer audio NO VERIFICADO. Los 63,2 MB de pesos por voz excluyen activaciones y runtime. Cargar solo el idioma utilizado y medir el pico. | Piper actual GPL-3.0-or-later. La cadena de datos se comprueba por model card. Joe y davefx retrenados declaran CC0 y base rdh CC0, y exigen Piper 1.5+. Integración C/C++ propia o una variante compatible con VITS de sherpa-onnx. Compatibilidad del retrenado con sherpa 2.0.1 NO VERIFICADO. |
| Kokoro-82M V1.0 int8 local | 114.202.993 B de modelo + 28.200.960 B de voces, unos 142,4 MB antes de léxicos y datos. El paquete publicado completo ronda 190 MB. La exportación int8 V1.1 publicada ronda 215 MB de paquete. | El checkpoint original tiene voces inglesas y españolas, entre ellas ef_dora, em_alex y em_santa. El crate actual valida léxico inglés y solo configura voces inglesas. La ruta ES necesita fonemización/lang y revisión. | RAM móvil NO VERIFICADO. Los 142,4 MB locales de modelo y voces excluyen buffers y runtime. En el PC de 12 núcleos del README, V1.0 int8 tuvo 1,8–2,0 s al primer bloque y RTF 1,29 con 4 hilos / 1,43 con 2. | Pesos Apache-2.0 y atribuciones de corpus señaladas en la ficha. sherpa-onnx Apache-2.0 y ONNX Runtime MIT. Revisar eSpeak-ng y avisos de la distribución. Ya existe envoltorio Rust reutilizable. |
| Kitten TTS Nano 0.8 int8 | 15 millones de parámetros y unos 25 MB publicados, más voces y dependencias del paquete concreto. Micro 40M y Mini 80M son variantes mayores. | Variante pequeña documentada con voces inglesas. Español en este checkpoint NO VERIFICADO. Puede competir para salida inglesa de bajo consumo. Calidad percibida NO VERIFICADO. | RAM, latencia y RTF en móvil NO VERIFICADO. El tamaño de 25 MB representa archivo, por lo que exige medir activaciones y runtime. | Ficha Nano 0.8 int8 declara Apache-2.0. sherpa tiene paquetes de ejemplo Android. La integración de este modelo todavía falta en pulsys-voice. |

Fuentes de Piper en [motor mantenido](https://github.com/OHF-Voice/piper1-gpl), [licencias y cadena de entrenamiento](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/LICENSING.md), [model card Joe](https://huggingface.co/rhasspy/piper-voices/blob/main/en/en_US/joe/medium/MODEL_CARD), [model card Davefx](https://huggingface.co/rhasspy/piper-voices/blob/main/es/es_ES/davefx/medium/MODEL_CARD), [archivo Joe](https://huggingface.co/rhasspy/piper-voices/tree/main/en/en_US/joe/medium) y [archivo Davefx](https://huggingface.co/rhasspy/piper-voices/tree/main/es/es_ES/davefx/medium).

Fuentes de Kokoro en [model card](https://huggingface.co/hexgrad/Kokoro-82M), [voces y fonemización por idioma](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md), [paquete int8 V1.0](https://huggingface.co/csukuangfj/kokoro-int8-multi-lang-v1_0/tree/main) y [paquete int8 V1.1](https://huggingface.co/csukuangfj/kokoro-int8-multi-lang-v1_1/tree/main). Medida histórica en `crates/voice/README.md:18–36`. Datos de Kitten en [ficha Nano 0.8 int8](https://huggingface.co/KittenML/kitten-tts-nano-0.8-int8) y [catálogo de APK de sherpa](https://k2-fsa.github.io/sherpa/onnx/tts/apk.html).

El mantenimiento actual de Piper distingue claramente licencia de motor y derechos de cada voz. Hay voces con nombres iguales que cambiaron de base de entrenamiento, y copias antiguas todavía derivan de Lessac. El espejo sherpa de Davefx consultado tiene una model card antigua y un paquete propio de unos 81,2 MB. La licencia y compatibilidad del modelo que se distribuya requieren fijar revisión/hash y conservar su ficha completa. Véanse [política de Piper](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/LICENSING.md), [lista revisada de voces](https://github.com/OHF-Voice/piper1-gpl/blob/main/docs/COMMERCIAL_USE.md) y [paquete sherpa Davefx](https://huggingface.co/csukuangfj/vits-piper-es_ES-davefx-medium/tree/main).

Kokoro publica voces ES, pero la documentación de su paquete sherpa V1.0 describe soporte EN/ZH. El código de configuración de sherpa 2.0.1 sí expone `lang`, con español como ejemplo, y permite fonemas externos. Esto permite investigar su integración ES sin darla por hecha. La asociación de IDs de voces, tokenizador, normalización española y exportación deben probarse con el paquete exacto. El README del paquete local documenta una tabla de 54 voces con em_santa en el ID 53, sin que eso acredite el recorrido español completo de Pulsys. Véase `externo/tts-kokoro/kokoro-int8-multi-lang-v1_0/README.md:3–20`. [Documentación Kokoro de sherpa](https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/kokoro.html), [configuración Kokoro en sherpa 2.0.1](https://github.com/k2-fsa/sherpa-onnx/blob/v2.0.1/sherpa-onnx/csrc/offline-tts-kokoro-model-config.h).

Las variantes más nuevas de Kitten publicitan más idiomas, pero cambian modelos, formatos y condiciones de licencia. La comparación anterior fija Nano 0.8 int8, cuya ficha sí consulté, y deja la nueva familia fuera de la selección del piloto hasta verificar sus propios pesos, binarios y licencia. [Repositorio Kitten](https://github.com/KittenML/KittenTTS), [ficha de la variante evaluable](https://huggingface.co/KittenML/kitten-tts-nano-0.8-int8).

#### Qué falta en la integración actual

El crate `pulsys-voice` ya fija sherpa-onnx 2.0.1 y ofrece TtsEngine, normalización inglesa y síntesis por frases. El normalizador limita a 600 caracteres y el generador acumula al final todas las muestras de las frases además de entregarlas al callback. Su cancelación se comprueba entre frases. Para una salida continua con memoria acotada conviene adaptar esa API al protocolo de tareas y a una cola PCM con contrapresión. Véanse `crates/voice/Cargo.toml:8–14`, `crates/voice/src/lib.rs:322–445` y `crates/voice/README.md:18–24`.

Desde Flutter hay dos rutas factibles. La primera conserva inferencia en `crates/voice`, la expone por `api/bridge` y entrega PCM a reproducción nativa del paquete local_speech. La segunda usa el binding Flutter sherpa_onnx en un isolate y su gestión de recursos. Recomiendo aprovechar el wrapper Rust y un único coordinador de memoria para evitar instancias duplicadas. El paquete Flutter publicado cubre Android e iOS, pero API 26, iOS 15, ABI y bibliotecas concretas deben validarse en el artefacto. [API Rust sherpa 2.0.1](https://docs.rs/sherpa-onnx/2.0.1/sherpa_onnx/), [binding Flutter](https://pub.dev/packages/sherpa_onnx).

Kokoro y Piper suelen necesitar conversión de grafemas a fonemas. El pipeline español de Kokoro cita eSpeak-ng. Su licencia GPL y los avisos de los componentes incluidos requieren comprobarse al redistribuir el paquete. Usar sherpa-onnx como runtime conserva las obligaciones aplicables de los pesos y componentes. [Fonemización de Kokoro por idioma](https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md), [licencia eSpeak-ng](https://github.com/espeak-ng/espeak-ng/blob/master/COPYING).

#### Coexistencia y elección con plan B

| Combinación | Evaluación en un teléfono de 2–3 GB |
|---|---|
| V5/V7 Qwen1.7B + Kokoro o Piper residentes a la vez | Riesgo alto. El lenguaje ya puede consumir 1,5–2,5 GiB estimados antes de UI y TTS. Descartaría esta combinación del perfil ligero. |
| Extractor 350M + TTS propio secuenciados | Candidato viable para pruebas. El pico se aproxima al mayor de los motores más app, buffers y sistema. La estabilidad real queda NO VERIFICADO. |
| Qwen0.6 + Kokoro simultáneos | El presupuesto estimado de Qwen0.6 sería de 0,7–1,2 GiB, al que se añaden al menos los archivos activos de Kokoro y su coste de inferencia aún sin medir. La estabilidad de la combinación queda NO VERIFICADO en 2 GB y 3 GB. |
| Extractor pequeño + TTS nativo | Primera opción. El coste extra del servicio debe medirse en el sistema completo. Puede descargarse el extractor antes de reproducir en el perfil de 2 GB. |
| Preguntas/fichas con frases pregrabadas y extractor opcional | Recurso de continuidad cuando falta una voz instalada o se detecta presión de memoria. El vocabulario de la voz queda limitado a contenido aprobado. |

**Opción recomendada.** Preparar una voz nativa local mejorada EN y otra ES antes de la misión. En iOS 15 seleccionar enhanced disponible y en Android seleccionar la mejor Voice local instalada que pase la prueba. La selección final depende de comprensión de síntomas, números y negaciones en una escucha comparativa. Esto debe funcionar con reglas y frases autorizadas mientras el extractor pequeño se evalúa.

**Plan B.** Evaluar Piper medium con una voz EN y otra ES fijadas por revisión/hash, cargando una sola después de liberar el modelo de lenguaje. Comenzaría por Joe/Davefx retrenados por su cadena documental CC0, resolviendo antes el runtime Piper 1.5+ y sus obligaciones GPL o la compatibilidad real de conversión con sherpa. Si el piloto de integración o la escucha falla, se mantiene la voz nativa preparada. Kokoro V1.0 int8 es el segundo candidato de calidad para comparar, aprovechando Rust, con salida española y latencia pendientes. En 2–3 GB lo habilitaría solo de forma secuenciada tras superar mediciones.

El plan B es una recomendación de investigación, pendiente de selección de licencia y prueba. La contingencia inmediata del producto conserva texto y frases aprobadas pregrabadas para ayuda. Para anunciar voz offline siempre disponible hace falta preparar y validar al menos una ruta en cada teléfono admitido.

La prueba de voces de la fase F debe comparar las mismas frases en EN/ES con hablantes del idioma y ruido de ventilación. Registrar comprensión de cifras y unidades, omisiones, inteligibilidad, naturalidad percibida, tiempo hasta primer audio, RTF, RAM máxima y cancelación. RTF es tiempo de síntesis dividido por duración del audio. Un RTF mayor que 1 implica que generar tarda más que reproducir esa duración y puede producir pausas. **NO VERIFICADO** — Ganador auditivo, RAM y latencia móvil de las voces concretas hasta ejecutar esta prueba.

#### Pendientes de cierre técnico y médico

Faltan la matriz física EN/ES, el corpus conversacional, los contratos de episodio, las fichas revisadas, los permisos concretos de redistribución y un benchmark integrado bajo presión de memoria. También falta verificar la lista completa de recursos del reto desde su página oficial accesible. Estas limitaciones condicionan la implementación y la aceptación del asistente, y quedan identificadas para las decisiones del punto 6.
