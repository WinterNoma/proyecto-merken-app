# Requisitos — App móvil de salud para astronautas

Versión 1 · 14-nov-2026 · Repo: https://github.com/WinterNoma/proyecto-merken-app
Base: [reto](https://www.spaceappschallenge.org/2026/challenges/health-monitoring-software/), [investigación](investigacion/00-resumen.md), [revisión de Sol v0](../reviews/requisitos-app-movil.md), [decisión de interfaz](decisiones/001-interfaz-tauri.md).

## 0. Lo que sabemos y lo que falta

| Decidido | Pendiente (§10) |
|---|---|
| App móvil Android hecha con **Tauri 2**, con la lógica en **Rust** | Fecha límite, equipo y experiencia en Rust |
| **Funciona sin conexión**: el móvil es la fuente de verdad | Qué hará exactamente el SLM y qué modelo usamos |
| Cuando hay conexión: **sube datos y recibe análisis** del servidor | Con qué tecnología se hace el servidor y qué análisis devuelve |
| **SLM (modelo de lenguaje pequeño) dentro del móvil** | Si los jueces piden enlace web o aceptan APK y video |

## 1. Objetivo

El astronauta registra sus indicadores en pocos segundos y ve cómo ha cambiado respecto a su referencia personal. Recibe una orientación clara: seguir igual, actuar con un protocolo o pedir ayuda (primero al responsable médico a bordo y luego a Tierra). La app funciona igual sin red. Cuando hay conexión, sincroniza con la base de datos de Tierra, sube los registros y recibe los análisis.

## 2. Principios de diseño

1. **Primero sin conexión.** Todas las funciones de la demo van sin red. La red solo añade dos cosas: copia de seguridad y análisis de Tierra.
2. **Las decisiones de salud las toma el núcleo determinista en Rust.** No las toman ni la interfaz, ni el SLM, ni el servidor. El servidor aporta análisis, pero nunca rebaja una alarma local.
3. **El SLM ayuda, no decide.** Estructura lo que escribe el usuario, explica y redacta. Nunca clasifica la gravedad ni recomienda tratamientos.
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
- **RF-01 (M)** Registro parcial de **pulso, sueño (horas y calidad), ánimo y cefalea (0–10)**. Cada registro guarda la hora de la medición, la hora en que se anotó, su origen (manual, dispositivo, SLM confirmado) y una nota opcional.
- **RF-02 (M)** Un flujo de **eventos de alarma** separado y con un acceso directo, "Necesito ayuda": desmayo, pérdida de campo visual y sospecha de trombo. Distingue entre síntoma, sospecha del usuario y hallazgo confirmado por personal médico.
- **RF-03 (M)** Los registros no se borran ni se sobrescriben. Una corrección se guarda como una versión nueva que enlaza con la anterior.
- **RF-04 (S)** El resto de constantes: presión, temperatura, SpO₂, frecuencia respiratoria, masa e ingesta.
- **RF-05 (S)** Contexto ambiental: CO₂ de cabina y dosis de radiación, separados de la evaluación de síntomas (§5.3).
- **RF-06 (S)** Recordatorios de medición.

### 5.2 Figura humana
- **RF-07 (M)** Figura en SVG con zonas que se pueden tocar. Cada zona muestra el peor estado de sus indicadores, y una alarma pesa más que cualquier estado favorable. Hay una alternativa en forma de lista, y el color siempre va acompañado de un icono y un texto.

### 5.3 Evaluación (núcleo en Rust)
- **RF-08 (M)** **Referencia personal previa al vuelo**, ficticia y fija para la demo. Se define su origen, su ventana y qué pasa cuando hay pocos datos o la dispersión es cero.
- **RF-09 (M)** Cuatro estados: **sin evaluación** (con motivo y fecha), **estable**, **cambio respecto a tu referencia** y **alarma**. El cambio de referencia **no es una alarma clínica**. Las reglas que no tienen evidencia publicada se marcan como **heurística de demo**.
- **RF-10 (M)** Toda alarma por evento (RF-02) produce **alarma**. La acción es "avisar al CMO a bordo, abrir el protocolo sin conexión y además enviar a Tierra".
- **RF-11 (M)** Cada evaluación indica sus datos, su referencia, la regla con su versión y la fuente con el apartado exacto. El tipo de fuente es uno de estos: evidencia publicada, protocolo aprobado o heurística de demo.
- **RF-12 (S)** Exposición ambiental aparte, cada una con su norma y su ámbito: CO₂ como **promedio horario** de la cabina y radiación como **dosis efectiva acumulada de carrera**. No se usan para poner color a los síntomas.
- **RF-13 (C)** Mostrar coincidencias en el tiempo (p. ej., CO₂ alto y cefalea) sin decir que una cosa causa la otra.

### 5.4 Asistente con SLM (en el móvil, sin conexión)
- **RF-14 (M)** **Registro en lenguaje natural:** el usuario escribe, por ejemplo, "me duele la cabeza un 6 desde esta mañana y dormí mal". La app propone campos estructurados, el núcleo los valida y el usuario los **confirma** antes de guardarlos. No se guarda nada sin esa confirmación. El intérprete tiene dos capas:
  1. **Intérprete determinista en Rust** (en `core`, sin modelo): reconoce el vocabulario de los indicadores de la demo (síntoma, zona, número 0–10, horas, "desde esta mañana") y sus negaciones. Es instantáneo y se prueba con tests.
  2. **SLM como respaldo** para lo que la capa 1 no entienda, con la salida **restringida por gramática** (GBNF/JSON Schema de llama.cpp). Así solo puede devolver un JSON con el esquema de `registro`, y los campos dudosos los marca como "sin interpretar".
  **Modelo de extracción aparte, muy pequeño y bilingüe (español/inglés):** de 0,3 a 0,6 mil millones de parámetros, en torno a 200–400 MB cuantizado. Por ejemplo, Gemma 3 270M o Qwen3-0.6B, ajustados con un LoRA de extracción por el compañero del SLM. **Antes de elegir hay que comprobar las versiones vigentes, sus licencias y su consumo en el móvil.** Con la gramática, un modelo pequeño basta, porque no tiene que redactar: solo rellenar campos. El v5 actual no extrae ([revisión](../reviews/slm-space-health-v5.md)).
- **RF-15 (S)** **Explicar la evaluación** con lenguaje sencillo. El SLM solo recibe los motivos y las fuentes que ha dado el núcleo, y su respuesta no puede contradecir el estado. **Candidato: SLM space-health v5** (Qwen3-1.7B + LoRA), que ya hace esto con hallazgos sintéticos. La salida de `core` debe alinearse con su contrato de entrada (`motor`).
- **RF-16 (S)** **Redactar el resumen** del informe para los médicos a partir de los datos estructurados, marcado como "borrador generado por IA" y editable.
- **RF-17 (C)** Preguntas sobre los protocolos y documentos cargados en la app (búsqueda local sobre ellos), citando el fragmento de donde sale la respuesta.
- **RF-18 (C)** Dictado por voz, con un modelo de voz a texto también en local.
- **Límites del SLM (M):** no asigna estados, no cambia colores, no da dosis ni diagnósticos y no responde fuera de su función. Si su salida no pasa la validación, se descarta. Si el modelo falla o no está cargado, la app funciona igual con los formularios.

### 5.5 Sincronización con Tierra
- **RF-19 (M)** La base de datos local (SQLite) es la fuente de verdad. Los cambios se encolan en una **bandeja de salida** persistente.
- **RF-20 (M)** Al detectar conexión, se **suben los registros pendientes** por lotes. Cada registro lleva un UUID y la subida es **idempotente**, para que reintentar no cree duplicados.
- **RF-21 (M)** Se **descargan los análisis** nuevos desde la última sincronización. Cada análisis indica quién lo hizo (médico o modelo del servidor), cuándo, sobre qué datos y con qué versión. Se muestra junto a la evaluación local, **sin sustituirla**, y nunca rebaja una alarma local.
- **RF-22 (M)** Estado visible de cada envío: *pendiente → enviado → recibido por Tierra → revisado*. "Recibido" no significa que alguien lo haya atendido.
- **RF-23 (S)** El envío es explícito: el usuario ve qué se va a subir. El ánimo y las notas privadas solo se suben si el usuario lo marca.
- **RF-24 (S)** Retraso de comunicación simulado (Tierra, Luna o Marte) y modo "sin enlace", para la demo.
- **RF-25 (C)** Actualizar las reglas o los protocolos desde el servidor, con versiones y sin borrar las anteriores.

### 5.6 Historial e informe
- **RF-26 (M)** Una gráfica reutilizable por indicador, con la banda de la referencia y los eventos marcados.
- **RF-27 (M)** Informe en texto con vista previa: datos, evaluaciones, alarmas y análisis recibidos.
- **RF-28 (M)** Datos sintéticos que se pueden reiniciar, con tres escenarios: **seguimiento habitual**, **cambio persistente** y **evento de alarma**.

## 6. Requisitos no funcionales

- **RNF-07** Objetivos táctiles grandes, que se comprueban en el dispositivo. El registro frecuente (RF-01) se hace en ≤ 3 toques desde la pantalla de inicio.
- **RNF-08** El SLM no bloquea nunca la interfaz: se puede cancelar, muestra el progreso y tiene un tiempo máximo. Para la demo funciona en modo determinista (temperatura 0 y semilla fija).
- **RNF-09** El modelo cabe en la memoria del dispositivo de referencia junto con la app. El tamaño y la cuantización se deciden en la prueba técnica.
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

**SLM en Rust:** el motor de inferencia se esconde detrás de un trait, para poder cambiarlo. Candidatos que hay que probar en Android: `llama.cpp` mediante el crate `llama-cpp-2` (modelos GGUF cuantizados), `candle` (Rust puro) y ONNX Runtime con el crate `ort`. Modelos candidatos: familias pequeñas de 0,5 a 4 mil millones de parámetros que se puedan instalar en el móvil (p. ej., Gemma, Qwen, Phi o Llama en tamaños pequeños). **Antes de decidir hay que comprobar las versiones vigentes y sus licencias.**

## 8. Alcance de la demo (M)

Android en un dispositivo acordado → registro manual y en lenguaje natural (SLM) de pulso, sueño, ánimo y cefalea → flujo de alarmas → figura SVG → evaluación explicada → una gráfica → informe en texto → sincronización con subida y descarga de análisis (se puede simular el servidor) → tres escenarios sintéticos. **Fuera de la demo:** 3D, fotos, farmacia, gamificación, iOS.

## 9. Fuera de alcance

- Sensores o wearables reales (se simulan).
- Diagnóstico automático o recomendación de fármacos.
- Validación clínica real: las reglas son de demostración y van marcadas como tales.

## 10. Decisiones pendientes

1. **SLM:** ¿qué funciones entran en la demo (RF-14 a RF-18)? ¿Qué modelo y qué motor de inferencia usamos? Se decide con una prueba en el dispositivo.
2. **Servidor:** la base de datos será **MySQL** ([decisión 002](decisiones/002-base-de-datos.md)). ¿Con qué tecnología hacemos la API de sincronización (Rust/axum + sqlx…)? ¿Quién la construye? ¿Los "análisis" los hace un modelo más grande, un médico o ambos?
3. **Entrega:** fecha límite, equipo y qué móvil Android usamos. ¿Hace falta un enlace web?
4. **Contenido médico:** ¿quién revisa los protocolos? ¿Usamos ejemplos marcados como tales?
5. ¿Qué ideas del Google Doc del equipo hay que añadir?

## 11. Orden de trabajo propuesto

1. **Prueba técnica de integración:** un APK que llame a Rust, escriba y lea SQLite y **ejecute un SLM pequeño** en el móvil, midiendo memoria y tiempo.
2. Contrato de datos: tipos de `core`, esquema SQLite y API de sincronización (borrador OpenAPI).
3. Núcleo de evaluación con sus pruebas y los escenarios sintéticos.
4. Recorrido visual: registrar → entender → actuar → informe.
5. Sincronización con un servidor simulado y después con el real.
