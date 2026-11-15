# Revisión 2 de la Fase 1 del SLM (motor llama.cpp en escritorio) — Sol

Commit revisado: `ba4d059`. Revisión estática en solo lectura.

## Veredicto

**No se acepta todavía la Fase 1 de escritorio: el P0 de LoRA sigue abierto, el silenciamiento GBNF no es absoluto y hay tres P1 nuevos.**

Revisión estática y en solo lectura, sin modificar archivos. El entorno rechazó la creación de procesos (`Access denied`): no pude ejecutar `git show`, Rust ni inferencias. Leí los objetos Git y comprobé que los siete archivos de `ba4d059` coinciden con el árbol de trabajo, normalizando CRLF. Los resultados de fmt, clippy, tests y paridad **40/40** son evidencia comunicada por el orquestador.

Las referencias nativas siguientes corresponden a las fuentes locales de `llama-cpp-sys-2 0.1.161`.

## Estado de los hallazgos de la revisión 1

| Hallazgo anterior | Estado | Comprobación y contraejemplos |
|---|---|---|
| **P0 — GBNF puede imprimir contenido privado** | **Mitigado; no cerrado para la garantía absoluta de privacidad.** | Ambas entradas pasan ahora por el prevalidador: [llama.rs:227](crates/slm/src/llama.rs:227) y `:389`. El contraejemplo `root ::= "dato privado" inexistente` se rechaza antes de entrar en C++. **No encontré una divergencia sintáctica aceptada por Rust y rechazada por el parser nativo.** Sin embargo, `llama-grammar.cpp:719–720` sigue capturando cualquier `std::exception`, incluida `std::bad_alloc`, e imprimiendo `src` completo. Una gramática válida con un marcador y un fallo de asignación durante el parseo conserva esa vía de filtración. Para cerrar la garantía solicitada hay que eliminar o silenciar ese `fprintf` en la dependencia. |
| **P0 — Adaptador aceptado sin aplicar LoRA** | **Parcial; sigue abierto.** | Se rechazan cero tensores, normalizaciones sueltas, componentes incompletos y pares ausentes de la base. Pero [gguf.rs:139](crates/slm/src/gguf.rs:139) comprueba pertenencia a la cabecera, **no uso efectivo por el grafo**. Contraejemplo: únicamente `output_norm.weight.lora_a` y `.lora_b`, con formas compatibles y datos válidos. El cargador los registra, pero Qwen3 aplica `output_norm.weight` mediante `build_norm`, que no consulta LoRA. Se ejecuta la base sin contribución del adaptador. Detalle debajo. |
| **P1 — Constructor público elude bloqueo/revalidación** | **Cerrado dentro de la frontera documentada del almacén.** | `from_verified_with_config` es privado; `load_into` es la única entrada pública de construcción. `store.rs:469–471` mantiene la guardia desde SHA-256 hasta terminar la fábrica; la lectura GGUF está dentro de ella (`llama.rs:160–164`). El escenario anterior de instalación concurrente mediante `ModelStore` queda bloqueado. La garantía no incluye escritores externos que ignoren ese bloqueo. |
| **P2 — Falta control final en carga** | **Cerrado.** | `llama.rs:204` comprueba el control después de destruir el contexto provisional. Los errores de inicialización/asignación priorizan cancelación o plazo mediante `load_error`. |
| **P2 — Transformación silenciosa de salida anómala** | **Cerrado.** | `CONTROL` y `UNKNOWN` no EOG producen error; EOG se comprueba primero. La conversión final usa UTF-8 estricto. Los bytes partidos entre tokens siguen siendo admisibles; un final incompleto falla (`llama.rs:311–348`, `:359–372`). |
| **P2 — Paridad incompleta termina exitosamente** | **Cerrado.** | `v5_parity.rs:211–212` devuelve error si `exact != count`. El contraejemplo 39/40 ya no puede terminar con éxito. |
| **P2 — Descripción incorrecta del mmap del adaptador** | **Cerrado.** | `docs/slm-motor-escritorio.md:38` distingue base mapeada y adaptador copiado a buffers; conserva la limitación del desglose de memoria medido externamente. |

El contraejemplo LoRA no requiere modificar la base: para `output_norm.weight` de forma `[D]`, un A F32 de forma `[D,1]` y un B F32 `[1,1]` superan las comprobaciones de `llama-adapter.cpp:358–378`. El adaptador queda registrado y `llama_set_adapters_lora` devuelve cero (`llama-context.cpp:4135–4146`). Sin embargo, `src/models/qwen3.cpp:145–147` utiliza ese peso mediante `build_norm`; `llama-graph.cpp:1634–1666` solo hace normalización y multiplicación ordinaria.

**Contar pares cargados tampoco basta:** debe existir al menos un destino que el grafo de esa arquitectura consuma mediante las operaciones que incorporan LoRA. Conviene imponer destinos admitidos para Qwen3 o exponer esa comprobación desde la dependencia, con regresiones de adaptadores que solo contienen normalizaciones.

## Hallazgos nuevos

Son contraejemplos estáticos; no los ejecuté contra el binario.

| Prioridad | Hallazgo y referencias | Contraejemplo e impacto | Corrección |
|---|---|---|---|
| **P1** | **Metadatos de tipo incorrecto alcanzan aserciones nativas.** [gguf.rs:101](crates/slm/src/gguf.rs:101); `llama-adapter.cpp:195–221`; `gguf.cpp:194–199`, `:1180–1183`. | Adaptador con pares válidos y `general.architecture` codificado como `UINT32`, manteniendo `general.type="adapter"` y `adapter.type="lora"`. Rust descarta aquel valor y acepta. C++ llama a `gguf_get_val_str`, cuya comprobación de tipo ejecuta `GGML_ASSERT`: **termina el proceso**, sin devolver `SlmError`. Lo mismo ocurre con un tipo incorrecto para `adapter.lora.alpha`. | Validar los tipos y cardinalidades de los metadatos consumidos por el cargador; preferiblemente convertir también esos accesos nativos en errores recuperables. Probar fallos en un proceso separado. |
| **P1** | **El límite de cabecera no limita las reservas nativas del adaptador.** [gguf.rs:120](crates/slm/src/gguf.rs:120); `llama-adapter.cpp:358–415`. | El lector descarta dimensiones, tipo y offsets después de leerlos. Un par para una matriz `[D,O]`, con A `[D,R]` y B `[R,O]`, puede declarar un rango enorme y offsets coherentes en un archivo pequeño, sin sus datos. El lector nativo con `no_alloc` crea los descriptores; el cargador **intenta reservar los buffers antes de comprobar los límites de los datos** (`:388` frente a `:403`). Puede intentar consumir muchos GB antes de rechazar el archivo. | Calcular tamaños con aritmética comprobada, validar rangos de datos contra el tamaño real y aplicar un presupuesto de memoria antes de reservar. Adelantar también la comprobación nativa de límites. |
| **P1** | **Una gramática acíclica pequeña puede bloquear la inicialización durante un tiempo exponencial.** [gbnf.rs:261](crates/slm/src/gbnf.rs:261); `llama-grammar.cpp:963–1013`, `:1255–1266`. | `root ::= n0`; para `i=0..29`, `ni ::= n(i+1) "a" \| n(i+1) "b"`; finalmente `n30 ::= "x"`. Son **776 bytes, 32 reglas, cero ciclos y cero cuantificadores**: Rust acepta. La detección nativa vuelve a recorrer las referencias ya visitadas y alcanza `n30` **2³⁰ veces** durante la primera comprobación. No hay comprobaciones de `Control` en ese recorrido. | Memoizar correctamente el análisis nativo o imponer previamente un presupuesto de expansión/trabajo sobre el grafo de reglas. Añadir una regresión con plazo externo; reducir solo profundidad de paréntesis no resuelve este caso. |

## Riesgos residuales

**GBNF.** Contrasté nombres de regla, escapes, clases y rangos, espacios, comentarios, finales de línea/archivo, grupos, alternancias y repeticiones con `llama-grammar.cpp:94–183` y `:439–725`. Los nombres ASCII alfanuméricos y `-` coinciden; también los escapes admitidos y sus longitudes hexadecimales. El filtro rechaza extensiones nativas como `.`, terminales de token y recursión productiva: es un subconjunto deliberadamente conservador.

No encontré rechazo del `PROPOSAL_GBNF` actual ni de las selecciones para listas vacías e IDs plausibles `ficha/frase` con los límites actuales. Los escapes de comillas y barras del constructor también encajan. Esto es conclusión estática, reforzada por la ejecución comunicada de ambas gramáticas; **no es una prueba diferencial exhaustiva**. Mantendría una prueba que capture stderr con un marcador, errores sintácticos y fallos de asignación.

**GGUF.** El lector Rust sí tiene lectura acotada, resta comprobada del presupuesto, strings de hasta 1 MiB, límites de elementos/tensores y profundidad de arrays. No multiplica dimensiones ni reserva según ellas. **64 MiB de cabecera no equivalen a 64 MiB de RAM**, por el coste de strings y conjuntos. Admite versiones 2 y 3; rechaza truncamientos, duplicados y tipos escalares desconocidos. No es todavía un validador completo: acepta arrays anidados y arrays vacíos con tipo de elemento inválido que el nativo rechazará.

Los sufijos `.lora_a`/`.lora_b` y el rechazo de apariciones internas evitan discrepancias con el `replace_all` nativo. Las normalizaciones sueltas no cuentan como pares. Falta además rechazar **NUL embebido en nombres**: Rust conserva el string completo y C++ lo trunca al convertirlo a nombre C. No puede asumirse equivalencia de nombres mientras se permita.

**Bloqueo y TOCTOU.** Las cabeceras y las aperturas nativas están bajo la misma guardia del almacén. No hay ventana para instalaciones cooperantes de `ModelStore`. Sí hay reapertura por ruta: Rust cierra sus lectores y C++ vuelve a abrir los archivos. Un escritor externo —o código que escriba directamente— puede sustituirlos. La afirmación correcta es ausencia de TOCTOU **dentro de la frontera documentada**, no identidad garantizada por un mismo descriptor abierto.

**Regresiones.** No encontré cambios adversos en ChatML, BOS, greedy, aceptación única del token, límites de contexto, control entre tokens/lotes ni validación posterior. El intérprete sigue saneando y comprobando `Proposal`; el explicador valida IDs y obtiene texto curado. Contextos y samplers siguen siendo nuevos por solicitud; el modelo nativo libera sus adaptadores registrados. Persisten las limitaciones de interrupción de llamadas nativas y la necesidad de un propietario único del backend y sus logs.

La paridad 40/40 respalda el paquete probado; no demuestra que todos los adaptadores aceptados aporten pesos ni constituye validación clínica.

## Qué queda para Tauri y Android

1. **Cerrar primero estos bloqueos**, incluyendo rechazo de adaptadores sin contribución efectiva, tipos GGUF, reservas previas a validación y coste de inicialización GBNF.
2. En Tauri, centralizar un trabajador y una ranura, usar cola acotada y permitir cancelar sin esperar al trabajador. Entregar al frontend únicamente resultados validados.
3. Fijar almacenamiento privado, política de escritores y manifiestos confiables. SHA-256 verifica correspondencia con el manifiesto, no la procedencia de este.
4. En Android, fijar NDK, ABI y bibliotecas nativas; verificar el paquete instalado en dispositivo real. Medir PSS/RSS, picos de carga/contexto, latencia, temperatura, cancelación y recuperación tras terminación por presión de memoria. Los resultados de Windows todavía no acreditan viabilidad móvil.