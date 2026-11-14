# Requisitos — App móvil de salud para astronautas

Versión 1.2 · 14-nov-2026 · Repo: https://github.com/WinterNoma/proyecto-merken-app
Base: [reto](https://www.spaceappschallenge.org/2026/challenges/health-monitoring-software/), [investigación](investigacion/00-resumen.md), [revisión de Sol v0](../reviews/requisitos-app-movil.md), [decisión de interfaz](decisiones/001-interfaz-tauri.md).

## 0. Lo que sabemos y lo que falta

| Decidido | Pendiente (§10) |
|---|---|
| App móvil Android hecha con **Tauri 2**, con la lógica en **Rust** | Fecha límite, equipo y experiencia en Rust |
| **Funciona sin conexión**: el móvil es la fuente de verdad | Confirmar los modelos con la prueba en el móvil (§11) |
| Cuando hay conexión: **sube datos y recibe análisis**; Tierra usa **MySQL** | Con qué tecnología se hace la API y qué análisis devuelve |
| **Modelos en el móvil:** Whisper base (voz), Granite 350M (respaldo del intérprete) y SLM v5 (explicar) | Si los jueces piden enlace web o aceptan APK y video |

## 1. Objetivo

El astronauta registra sus indicadores en pocos segundos y ve cómo ha cambiado respecto a su referencia personal. Recibe una orientación clara: seguir igual, actuar con un protocolo o pedir ayuda (primero al responsable médico a bordo y luego a Tierra). La app funciona igual sin red. Cuando hay conexión, sincroniza con la base de datos de Tierra, sube los registros y recibe los análisis.

## 2. Principios de diseño

1. **Primero sin conexión.** Todas las funciones de la demo van sin red. La red solo añade dos cosas: copia de seguridad y análisis de Tierra.
2. **Las decisiones de salud las toma el núcleo determinista en Rust.** No las toman ni la interfaz, ni el SLM, ni el servidor. El servidor aporta análisis, pero nunca rebaja una alarma local.
3. **El SLM ayuda, no decide.** Estructura lo que escribe el usuario, explica y **sugiere mejoras y prevención** a partir de las fichas de la app (RF-31), citándolas. Nunca asigna estados ni gravedad, y nunca da fármacos, dosis ni diagnósticos.
4. **Que falte información no es estar sano.** Sin datos suficientes, el estado es "sin evaluación", nunca verde.
5. **Todo se puede explicar.** Cada estado indica qué datos, qué referencia, qué regla (con su versión) y qué fuente lo produjeron.

## 3. Qué exige Rust a la app

| Intención | Requisito |
|---|---|
| Sin cuelgues | **RNF-01** En el código de producción del núcleo no se usa `panic!`, `unwrap()`, `expect()` ni indexado que pueda fallar. Lo que puede fallar devuelve `Result` y se traduce a un mensaje útil. En los tests se permiten aserciones. |
| Tipos fuertes | **RNF-02** Los tipos representan unidades y conceptos (`Sistolica`/`Diastolica`, `Bpm`, `Celsius`, dosis frente a tasa, instantes). Se validan al crearlos **y** al entrar desde JSON, SQLite o la salida del SLM (un `Deserialize` derivado no puede saltarse esa validación). Se distingue un dato inválido de un valor extremo que puede ser real. |
| Determinismo | **RNF-03** `evaluar(datos, referencia, contexto, versión de reglas, ahora)` es una función pura. El tiempo entra como argumento. El orden y el redondeo están definidos. |
| Separar el núcleo de la E/S | **RNF-04** `core` contiene solo el dominio y la evaluación, sin E/S. La persistencia, la red, el reloj y el SLM van en adaptadores (`store`, `sync`, `slm`). Solo se exige compilar para Android. WASM es opcional (S). |
| Recursos a bordo | **RNF-05** Todo funciona en local. El tamaño, el arranque y el consumo se miden en el dispositivo de referencia, y los objetivos se fijan después de la prueba técnica. |
| Probar los invariantes | **RNF-06** `cargo test`, `clippy -D warnings` y `fmt`. Con `proptest` se comprueba que: sin datos no hay verde; una alarma por evento no la anula ninguna tendencia ni ningún análisis remoto; ninguna entrada aceptada rompe la evaluación. |

## 4. Usuarios y contexto

- **Astronauta:** tiene poco tiempo, puede llevar guantes y no es médico. Es quien usa la app.
- **Responsable médico a bordo (CMO):** es el primer escalón cuando hay una alarma.
- **Médicos de vuelo en Tierra:** reciben los datos sincronizados y devuelven análisis, con retraso.
- **Contexto:** nave, estación, Luna o Marte. Cambia el retraso de comunicación (hasta 22 min por trayecto en Marte) y puede haber periodos sin enlace.

## 5. Requisitos funcionales

Prioridad: **M** = imprescindible para la demo · **S** = debería estar · **C** = si sobra tiempo.

### 5.1 Registro
- **RF-01 (M)** Registro parcial de **pulso, sueño (horas y calidad), ánimo y cefalea (0–10)**. Cada registro guarda la hora de la medición, la hora en que se anotó, su origen (formulario, dispositivo, texto o voz) y qué lo interpretó (intérprete Rust o Granite, con la versión del modelo), además de una nota opcional.
- **RF-02 (M)** Un flujo de **eventos de alarma** separado y con un acceso directo, "Necesito ayuda": desmayo, pérdida de campo visual y sospecha de trombo. Distingue entre síntoma, sospecha del usuario y hallazgo confirmado por personal médico.
- **RF-03 (M)** Los registros no se borran ni se sobrescriben. Una corrección se guarda como una versión nueva que enlaza con la anterior.
- **RF-04 (S)** El resto de constantes: presión, temperatura, SpO₂, frecuencia respiratoria, masa e ingesta.
- **RF-05 (S)** Contexto ambiental: CO₂ de cabina y dosis de radiación, separados de la evaluación de síntomas (§5.3).
- **RF-06 (S)** Recordatorios de medición.
- **RF-29 (M)** **Catálogo de indicadores ampliable con entrada manual.** Cualquier indicador se puede introducir a mano, incluidos los que normalmente dan sensores o pruebas más complejas: **glucosa (mg/dL)**, resultados de una prueba (p. ej., la conclusión de una ecografía como texto o categoría) y valores leídos en un aparato. Cada indicador del catálogo define su unidad, su rango válido (para rechazar valores imposibles, RNF-02), la zona del cuerpo y si tiene referencia personal. Añadir un indicador no exige cambiar el esquema de la base de datos (columna `datos` en JSON, [decisión 002](decisiones/002-base-de-datos.md)).

### 5.2 Figura humana
- **RF-07 (M)** Figura en SVG con zonas que se pueden tocar. Cada zona muestra el peor estado de sus indicadores, y una alarma pesa más que cualquier estado favorable. Hay una alternativa en forma de lista, y el color siempre va acompañado de un icono y un texto.
- **RF-30 (M)** **Ficha de cada zona** al tocarla. Muestra su **función** (qué hace esa parte del cuerpo), su estado actual, qué peligro del vuelo la afecta (radiación, aislamiento, distancia, gravedad o ambiente hostil, según el HRP), qué se mide a bordo y por qué. Los textos son estáticos y cada uno cita su fuente.

### 5.3 Evaluación (núcleo en Rust)
- **RF-08 (M)** **Referencia personal previa al vuelo**, ficticia y fija para la demo. Se define su origen, su ventana y qué pasa cuando hay pocos datos o la dispersión es cero.
- **RF-09 (M)** Cuatro estados: **sin evaluación** (con motivo y fecha), **estable**, **cambio respecto a tu referencia** y **alarma**. El cambio de referencia **no es una alarma clínica**. Las reglas que no tienen evidencia publicada se marcan como **heurística de demo**.
- **RF-10 (M)** Toda alarma por evento (RF-02) produce **alarma**. La acción es "avisar al CMO a bordo, abrir el protocolo sin conexión y además enviar a Tierra".
- **RF-11 (M)** Cada evaluación indica sus datos, su referencia, la regla con su versión y la fuente con el apartado exacto. El tipo de fuente es uno de estos: evidencia publicada, protocolo aprobado o heurística de demo.
- **RF-12 (S)** Exposición ambiental aparte, cada una con su norma y su ámbito: CO₂ como **promedio horario** de la cabina y radiación como **dosis efectiva acumulada de carrera**. No se usan para poner color a los síntomas.
- **RF-13 (C)** Mostrar coincidencias en el tiempo (p. ej., CO₂ alto y cefalea) sin decir que una cosa causa la otra.
- **RF-32 (M)** **Escalado según el estado**, decidido por el núcleo:
  - **Estable** → seguir igual.
  - **Cambio** → sugerencias del v5 en el móvil (RF-15).
  - **Cambio persistente** → se marca "para revisión de Tierra" y sale en el siguiente envío. La respuesta (del modelo de Tierra o de un médico real) llega por la API cuando haya conexión (RF-33).
  - **Alarma** → RF-10, sin esperar a Tierra.

### 5.4 Asistente con SLM (en el móvil, sin conexión)
- **RF-14 (M)** **Registro en lenguaje natural:** el usuario escribe, por ejemplo, "me duele la cabeza un 6 desde esta mañana y dormí mal". La app propone campos estructurados, el núcleo los valida y el usuario los **confirma** antes de guardarlos. No se guarda nada sin esa confirmación. El intérprete tiene dos capas:
  1. **Intérprete determinista en Rust** (en `core`, sin modelo): reconoce el vocabulario de los indicadores de la demo (síntoma, zona, número 0–10, horas, "desde esta mañana") y sus negaciones. Es instantáneo y se prueba con tests.
  2. **SLM como respaldo** para lo que la capa 1 no entienda, con la salida **restringida por gramática** (GBNF/JSON Schema de llama.cpp). Así solo puede devolver un JSON con el esquema de `registro`, y los campos dudosos los marca como "sin interpretar".
     **Qué aporta frente a la capa 1:** entiende expresiones coloquiales ("no pegué ojo", "la cabeza me explota"), frases mezcladas en español e inglés y frases largas con varios datos, negaciones y tiempos ("ayer dormí mal pero hoy ya no me duele"). Solo se carga cuando la capa 1 falla. Si tampoco entiende la frase, se abre el **formulario manual** con lo que ya se haya entendido rellenado. Que entre en la demo se decide con la prueba de voz.
  **Modelo de extracción aparte, muy pequeño y bilingüe (español/inglés)** ([investigación 07](investigacion/07-voz-interprete.md)): candidato principal **Granite 4.0 350M Q4_K_M** (237 MB, Apache 2.0) y alternativa **Qwen3-0.6B Q4_K_M** (484 MB, Apache 2.0, sin thinking). FunctionGemma 270M solo si el compañero entrena una tarea específica (licencia Gemma Terms). La gramática garantiza el formato, **no que la extracción sea correcta**: hay que evaluarlo con frases nuestras y probablemente ajustar un LoRA. Contrato de salida: claves y enums cerrados, `null` si falta un valor, el fragmento de texto que respalda cada campo y la lista de partes sin interpretar. El modelo no inventa horas: "desde esta mañana" queda ambiguo hasta que el usuario lo confirma. El v5 actual no extrae ([revisión](../reviews/slm-space-health-v5.md)).
- **RF-15 (M)** **Explicar y sugerir en el móvil, en tiempo real y sin conexión.** Lo hace el **SLM space-health v5** (Qwen3-1.7B + LoRA).
  - **Explicar** la evaluación con lenguaje sencillo. El v5 ya lo hace (40/40 con hallazgos sintéticos).
  - **Sugerir mejoras de salud y prevención** según el estado, por ejemplo sueño, ejercicio, hidratación o descanso. El núcleo elige las fichas aplicables (RF-31) y se las pasa en `documents`. El v5 redacta la sugerencia **citando esas fichas** y no puede añadir nada que no esté en ellas.
  - La salida no puede contradecir el estado, se marca como "sugerencia generada por IA" y se descarta si no pasa la validación.
  - **Pendiente con el compañero:** entrenar y evaluar la tarea de *sugerir*, porque hoy solo está probada la de explicar ([revisión](../reviews/slm-space-health-v5.md)). La salida de `core` debe alinearse con su contrato de entrada (`motor`).
- **RF-31 (M)** **Biblioteca local de fichas de contramedidas**, que es lo que alimenta al v5. Son fichas cortas por indicador y estado (qué hacer, por qué y cuándo pedir ayuda), cada una con su fuente NASA/HRP y contrastadas por Sol y Hydrogen. Van dentro de la app y tienen versión.
- **RF-16 (S)** **Redactar el resumen** del informe para los médicos a partir de los datos estructurados, marcado como "borrador generado por IA" y editable.
- **RF-17 (C)** Preguntas sobre los protocolos y documentos cargados en la app (búsqueda local sobre ellos), citando el fragmento de donde sale la respuesta.
- **RF-18 (S)** **Entrada por voz sin conexión**, que alimenta RF-14 ([investigación 07](investigacion/07-voz-interprete.md)):
  - **Interacción tipo Hex:** botón grande, mantener pulsado para hablar y soltar para terminar, con un modo alternativo de tocar para empezar y tocar para parar. Se ve cuándo está grabando, se puede cancelar y se revisa antes de confirmar. Sin palabra de activación.
  - **Motor recomendado:** **Whisper base multilingüe Q5_1** (60 MB, MIT) mediante `whisper-rs`/whisper.cpp o sherpa-onnx. Candidatos que compiten en la prueba: **Moonshine Streaming Tiny ES+EN** (78 MB, MIT, transcribe en tiempo real) y Vosk con vocabulario cerrado para comandos.
  - **Voz del sistema Android** (`createOnDeviceSpeechRecognizer`, API 31+): solo como mejora en móviles comprobados, nunca como única vía. `EXTRA_PREFER_OFFLINE` no basta y el sistema puede no ofrecerlo.
  - **Descartado:** Picovoice (su licencia se valida por Internet) y los modelos de audio grandes (Gemma 3n/4).
  - Se usa detección de voz (VAD) contra las frases que Whisper inventa en silencio. "Necesito ayuda" abre el flujo de ayuda **sin esperar** al modelo, y el botón manual sigue siempre ahí.
  - **Privacidad:** el audio se borra al terminar y ni el audio ni la transcripción se escriben en logs.
  - Combinación recomendada: Whisper base + intérprete Rust + Granite 350M, **≈297 MB**, que se cargan uno detrás de otro (sin tener el explicador v5 en memoria mientras se registra).
- **Límites del SLM (M):** no asigna estados, no cambia colores, no da dosis ni diagnósticos y no responde fuera de su función. Si su salida no pasa la validación, se descarta. Si el modelo falla o no está cargado, la app funciona igual con los formularios.

### 5.5 Sincronización con Tierra
- **RF-19 (M)** La base de datos local (SQLite) es la fuente de verdad. Los cambios se encolan en una **bandeja de salida** persistente.
- **RF-20 (M)** Al detectar conexión, se **suben los registros pendientes** por lotes. Cada registro lleva un UUID y la subida es **idempotente**, para que reintentar no cree duplicados.
- **RF-21 (M)** Se **descargan los análisis** nuevos desde la última sincronización. Cada análisis indica quién lo hizo (médico o modelo del servidor), cuándo, sobre qué datos y con qué versión. Se muestra junto a la evaluación local, **sin sustituirla**, y nunca rebaja una alarma local.
- **RF-22 (M)** Estado visible de cada envío: *pendiente → enviado → recibido por Tierra → revisado*. "Recibido" no significa que alguien lo haya atendido.
- **RF-23 (S)** El envío es explícito: el usuario ve qué se va a subir. El ánimo y las notas privadas solo se suben si el usuario lo marca.
- **RF-24 (S)** Retraso de comunicación simulado (Tierra, Luna o Marte) y modo "sin enlace", para la demo.
- **RF-25 (C)** Actualizar las reglas o los protocolos desde el servidor, con versiones y sin borrar las anteriores.

### 5.5b Análisis en Tierra (backend)
- **RF-33 (M)** **La API de Tierra recibe los datos e interpreta con un modelo propio**, que va aparte y es más pesado que el del móvil.
  - **Flujo:** la API recibe los lotes (RF-20) → los guarda en MySQL → el modelo de Tierra los analiza (sobre todo lo marcado "para revisión", RF-32) → un **médico de vuelo real puede revisar, corregir o escribir** el análisis → el móvil lo descarga (RF-21).
  - **Cada análisis lleva:** autor (`modelo` o `médico`), versión del modelo, datos analizados, resumen, sugerencias y nivel de atención sugerido. En el móvil se muestra junto al estado local y **nunca rebaja una alarma local**.
  - **Pendiente:** el contrato de entrada y salida (OpenAPI), qué modelo usa Tierra y quién lo construye.
- **RF-34 (C, solo concepto)** **Imágenes:** foto de un síntoma (piel, ojo) o imagen de una prueba (ecografía) → se guarda en el móvil → se sube a Tierra cuando hay conexión → se almacena → se analiza después → vuelve una recomendación por RF-21. Fuera de la demo por ahora; solo queda diseñado.

### 5.6 Historial e informe
- **RF-26 (M)** Una gráfica reutilizable por indicador, con la banda de la referencia y los eventos marcados.
- **RF-27 (M)** Informe en texto con vista previa: datos, evaluaciones, alarmas y análisis recibidos.
- **RF-28 (M)** Datos sintéticos que se pueden reiniciar, con tres escenarios: **seguimiento habitual**, **cambio persistente** y **evento de alarma**.
- **RF-35 (C, para después)** **Animaciones** (transiciones de estado en la figura, pulso animado, gráficas que se dibujan progresivamente). Ahora la prioridad es lo funcional; lo visual va después.

## 6. Requisitos no funcionales

- **RNF-07** Objetivos táctiles grandes, que se comprueban en el dispositivo. El registro frecuente (RF-01) se hace en ≤ 3 toques desde la pantalla de inicio.
- **RNF-08** El SLM no bloquea nunca la interfaz: se puede cancelar, muestra el progreso y tiene un tiempo máximo. Para la demo funciona en modo determinista (temperatura 0 y semilla fija).
- **RNF-09** **Memoria:** hay tres modelos (Whisper base ≈60 MB, Granite 350M ≈237 MB y SLM v5 ≈1,1 GB) y se cargan **de uno en uno**, nunca todos a la vez. Mientras se registra, el v5 no está en memoria. El pico de RAM con la app y la WebView se mide en el dispositivo de referencia (§11) y de ahí sale el límite.
- **RNF-13** **Instalación de los modelos:** no van dentro del APK (≈1,4 GB en total). Se descargan o se copian **antes de quedarse sin conexión**, con una pantalla de "preparar para la misión". Se comprueba el **hash** de cada archivo y la versión del par base + LoRA del v5. Si falta un modelo o está incompleto, esa función se desactiva, la app lo indica y sigue funcionando con los formularios.
- **RNF-14 (M)** **Todo lo que se ve en la app está en inglés** (requisito de la NASA): pantallas, botones, fichas (RF-30, RF-31), mensajes de error, estados, informe y la salida de los modelos. Los textos van en un archivo de cadenas (`en`), no repartidos por el código. La documentación interna del equipo puede seguir en español.
  - **Voz e intérprete:** el **inglés es el idioma principal** (M). Entender también español es un extra (S), pero la app responde siempre en inglés.
  - **Consecuencia para el v5:** hoy responde en español (`system.txt`), así que **hay que adaptarlo al inglés** (prompt de sistema y reentrenamiento o evaluación del LoRA). Es una tarea para el compañero del SLM. Whisper y Granite ya funcionan en inglés.
  - El intérprete en Rust empieza por los patrones en inglés.
- **RNF-10** Seguridad: la conexión usa HTTPS con un token por dispositivo, y los datos se cifran en reposo (S, p. ej., SQLCipher).
- **RNF-11** El texto médico se contrasta con Sol y con Hydrogen, y cita fuentes de la NASA. La app muestra el aviso "herramienta de apoyo, no diagnóstico" (que **no sustituye** a tener reglas correctas).
- **RNF-12** Cuando una sincronización falla o se interrumpe, no se pierde ni se duplica nada. Se prueba cortando la red a mitad de una subida.

## 7. Arquitectura

```
proyecto-merken-app/
├─ crates/
│  ├─ core/    # dominio, tipos, referencia, reglas, evaluación (puro, sin E/S)
│  ├─ store/   # SQLite (rusqlite) + bandeja de salida
│  ├─ sync/    # cliente HTTP: subida idempotente y descarga de análisis
│  ├─ slm/     # carga del modelo, inferencia y validación de la salida (trait intercambiable)
│  └─ seed/    # escenarios sintéticos
├─ src-tauri/  # comandos Tauri que unen los crates y la interfaz
├─ ui/         # TypeScript + Vite (solo presentación)
├─ server/     # pendiente: API de sincronización + base de datos de Tierra
└─ docs/
```

**Base de datos** ([decisión 002](decisiones/002-base-de-datos.md), aceptada): SQLite en el móvil y **MySQL** en Tierra. Las credenciales solo las tiene la API. El móvil solo habla con la API de sincronización, nunca con la base de datos de Tierra, así que pasar a NoSQL no obligaría a tocar la app. `store` es un trait de repositorio.

**Modelos en el móvil (crate `slm`, cada motor detrás de un trait para poder cambiarlo):**

| Función | Modelo | Motor | Peso |
|---|---|---|---|
| Voz a texto (RF-18) | Whisper base multilingüe Q5_1 (alternativa: Moonshine Tiny) | whisper.cpp (`whisper-rs`) o sherpa-onnx | ≈60 MB |
| Respaldo del intérprete (RF-14) | Granite 4.0 350M Q4_K_M (alternativa: Qwen3-0.6B) | llama.cpp (`llama-cpp-2`) con gramática GBNF | ≈237 MB |
| Explicar la evaluación (RF-15) | SLM space-health v5: Qwen3-1.7B Q4_K_M + LoRA cargado en tiempo de ejecución, **sin fusionar** | llama.cpp (`llama-cpp-2`) | ≈1,1 GB |

Las versiones exactas, las licencias y los hashes se fijan en la prueba técnica. La licencia del v5 la tiene que confirmar el compañero ([revisión](../reviews/slm-space-health-v5.md)).

## 8. Alcance de la demo (M)

Android en un dispositivo acordado → registro con formulario y en lenguaje natural (intérprete Rust + Granite; voz si la prueba sale bien) de pulso, sueño, ánimo y cefalea → flujo de alarmas → figura SVG → evaluación explicada → una gráfica → informe en texto → sincronización con subida y descarga de análisis (se puede simular el servidor) → tres escenarios sintéticos. **Fuera de la demo:** 3D, fotos, farmacia, gamificación, iOS.

## 9. Fuera de alcance

- Sensores o wearables reales (se simulan).
- Diagnóstico automático, fármacos y dosis (ni el v5 ni el modelo de Tierra los dan en la app).
- Validación clínica real: las reglas son de demostración y van marcadas como tales.

## 10. Decisiones pendientes

1. **Modelos:** la propuesta está en §7. Falta confirmarla con la prueba en el móvil (§11): que Whisper base y Granite rinden bien y que el v5 cabe en memoria. También falta que el compañero entregue la licencia y, si hace falta, un LoRA de extracción para Granite.
2. **Servidor:** la base de datos será **MySQL** ([decisión 002](decisiones/002-base-de-datos.md)). ¿Con qué tecnología hacemos la API de sincronización (Rust/axum + sqlx…)? ¿Quién la construye? ¿Los "análisis" los hace un modelo más grande, un médico o ambos?
3. **Entrega:** fecha límite, equipo y qué móvil Android usamos. ¿Hace falta un enlace web?
4. **Contenido médico:** ¿quién revisa los protocolos? ¿Usamos ejemplos marcados como tales?
5. ¿Qué ideas del Google Doc del equipo hay que añadir?

## 11. Orden de trabajo propuesto

1. **Prueba técnica de integración:** un APK que llame a Rust, escriba y lea SQLite y **ejecute un SLM pequeño** en el móvil, midiendo memoria y tiempo.
   - **Prueba de voz (1 día, [plan en investigación 07](investigacion/07-voz-interprete.md)):** 120 frases grabadas (3 hablantes × 20 frases ES/EN × con y sin ventilador, más silencio y mascarilla). Se comparan Whisper base y small, Moonshine Tiny y la voz del sistema; y Granite frente a Qwen sobre las mismas transcripciones.
   - **Qué se mide:** WER, acierto por campo, números ("seis" frente a "dieciséis"), negaciones, campos inventados, latencia p50/p95 desde que se suelta el botón, RAM, tamaño instalado y temperatura.
   - **Se descarta un motor si:** depende de la red, guarda sin confirmación o acepta en silencio números inventados.
2. Contrato de datos: tipos de `core`, esquema SQLite y API de sincronización (borrador OpenAPI).
3. Núcleo de evaluación con sus pruebas y los escenarios sintéticos.
4. Recorrido visual: registrar → entender → actuar → informe.
5. Sincronización con un servidor simulado y después con el real.
