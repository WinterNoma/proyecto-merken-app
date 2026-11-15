# Revisión 3 (final) de la Fase 1 del SLM (motor llama.cpp en escritorio) — Sol

Commit revisado: `b6e7609`. Revisión estática en solo lectura, con el modelo de amenazas acordado (manifiestos de confianza).

## Veredicto

**Sí, se acepta la Fase 1 de escritorio bajo el modelo de amenazas acordado; no identifico P0 ni P1 abiertos.**

Revisión estática y en solo lectura, sin modificar archivos. El entorno rechazó la creación de procesos: no pude ejecutar `git show`, Rust ni inferencias. Leí los objetos Git de `b6e7609` y comprobé que sus ocho archivos modificados coinciden con el árbol de trabajo, normalizando CRLF. Fmt, clippy, tests y paridad **40/40**, incluidos los **196 pares LoRA F16**, son evidencia comunicada por el orquestador, no ejecuciones propias.

## Estado de los hallazgos de la revisión 2

| Hallazgo | Estado bajo el modelo acordado | Comprobación |
|---|---|---|
| **P0 — GBNF puede imprimir contenido privado** | **Cerrado como P0; residual P2 aceptado.** | Ambas entradas comprueban `is_known_grammar` antes del prevalidador y del parser nativo. Solo pasan la propuesta exacta y selecciones canónicas con IDs acotados. El `fprintf` por `bad_alloc` permanece, pero las gramáticas autorizadas no contienen datos del usuario. |
| **P0 — Adaptador aceptado sin aplicar LoRA** | **Cerrado.** | Todos los pares deben apuntar a una de las siete matrices de atención/FFN de una capa Qwen3 existente. El contraejemplo de `output_norm` queda rechazado. Contrasté los destinos con `qwen3.cpp` y las llamadas a `build_lora_mm` en `llama-graph.cpp`; se exige al menos un par completo. |
| **P1 — Metadatos con tipos incorrectos alcanzan aserciones** | **Cerrado.** | Se exigen strings para arquitectura y tipos de paquete/adaptador; UINT32 para alineación y bloques; FLOAT32 para alpha; ARRAY de UINT32 para invocación aLoRA. Coinciden con los accesos de `llama-adapter.cpp`. |
| **P1 — Reservas nativas desproporcionadas antes de comprobar los datos** | **Cerrado para el adaptador admitido.** | Antes de C++ se calculan tamaños F16/F32 con aritmética comprobada, se verifican extremos contra el archivo y se aplica el presupuesto agregado de 256 MiB y rango 1–256. Ya no pasa el contraejemplo de rango enorme sin datos. |
| **P1 — Inicialización exponencial de una gramática acíclica** | **Cerrado.** | El contraejemplo `n0…n30` no pertenece al conjunto canónico y se rechaza antes del prevalidador y del nativo. Existe una regresión específica en `known_grammar_tests`. |
| **P1 — Constructor público elude bloqueo/revalidación** | **Sigue cerrado.** | El constructor continúa privado. `load_into` mantiene la guardia desde SHA-256 hasta finalizar la carga. Los escritores externos quedan fuera de esa garantía, como se acordó. |
| **P2 — Falta control final en carga** | **Sigue cerrado.** | Se conserva la comprobación después de destruir el contexto provisional y la prioridad de cancelación/plazo en errores de carga. |
| **P2 — Transformación silenciosa de salida anómala** | **Sigue cerrado.** | EOG se comprueba primero; CONTROL/UNKNOWN inesperados fallan y la conversión final exige UTF-8 completo. |
| **P2 — Paridad incompleta termina exitosamente** | **Sigue cerrado.** | `v5_parity` devuelve error cuando `exact != count`. |
| **P2 — Descripción incorrecta del mmap del adaptador** | **Sigue cerrado.** | El documento distingue base mapeada y adaptador copiado a buffers. |
| **Residuales GGUF — NUL y arrays inválidos** | **Cerrados los casos señalados.** | Se rechaza NUL en strings, arrays anidados y tipos de elemento inválidos incluso con cardinalidad cero. |

El orden de las barreras se confirma en [llama.rs:227](crates/slm/src/llama.rs:227) y `build_sampler`. El reconocimiento reconstruye la selección y exige igualdad **byte a byte**: añadir reglas, cambiar espacios o insertar referencias no permite alcanzar C++. Esto reconoce un conjunto de formas; una copia idéntica construida externamente también sería admitida, sin ampliar ese conjunto.

Los IDs presentes en las fichas de prueba del explicador —incluidos `hr-rest-1/compare-baseline`, `safety/no-aspirin` y `cmo-alarm/notify-cmo`— cumplen el alfabeto y miden entre 17 y 26 bytes. Sus gramáticas canónicas pasan el reconocimiento por construcción; no encontré incompatibilidad sintáctica con el prevalidador ni con el parser nativo.

En GGUF, la alineación potencia de dos, el comienzo de datos y los offsets secuenciales con padding coinciden con `gguf.cpp`. Para un destino `[D,O]`, las formas A `[D,R]` y B `[R,O]` coinciden con `llama-adapter.cpp`. Merken añade restricciones deliberadas de arquitectura, destinos, tipos y presupuesto. No encontré incompatibilidad con el v5; hay una diferencia de representación dimensional descrita abajo.

No encontré regresiones en ChatML, BOS, greedy, aceptación única de tokens, contexto, aplicación del adaptador por solicitud ni validación posterior. Almacén, slot, manifiestos, intérprete y ejemplo de paridad permanecen sin cambios respecto de `ba4d059`. El explicador continúa renderizando texto curado tras validar la selección.

## Hallazgos nuevos

| Prioridad | Hallazgo | Alcance y plan |
|---|---|---|
| **P2** | **Los límites individuales de IDs no garantizan que su combinación sea admisible.** Cada componente permite 40 bytes, pero `ficha/frase` se limita a 64: dos IDs válidos de 40 producen 81 y `offered_statements` falla. Véase [selection.rs:400](crates/slm/src/explain/selection.rs:400). | No afecta a los IDs presentes ni permite salida clínica sin validar. Alinear el contrato de carga de fichas con el límite combinado y añadir comprobaciones de frontera antes de incorporar otro catálogo. |
| **P2** | **Se rechaza una representación nativa válida de rango 1.** [gguf.rs:307](crates/slm/src/gguf.rs:307) exige exactamente dos dimensiones declaradas. El escritor nativo puede serializar A `[D,1]` como `[D]`; su lector repone la dimensión final a uno y el cargador LoRA la admite. | Caso límite de compatibilidad; no afecta al v5 probado. Normalizar dimensiones finales implícitas de valor uno antes de comparar formas y añadir una regresión de rango 1 conservando las restricciones de matrices. |

## Riesgos residuales

| Prioridad | Riesgo | Plan |
|---|---|---|
| **P2, aceptado** | `fprintf` nativo ante `bad_alloc` al analizar una gramática válida. | Mantener gramáticas sin datos del usuario; eliminar esa impresión al actualizar o parchear la dependencia. |
| **P2, aceptado** | Sustitución de archivos por escritores externos que ignoren el bloqueo. | Almacenamiento privado y escrituras mediante `ModelStore`; si cambia esa frontera, revisar carga por descriptor o copia inmutable. |
| **P2** | Los presupuestos de cabecera y tensores no equivalen al pico total de RAM. Las llamadas nativas tampoco son interrumpibles en cualquier punto. | Medir memoria y cancelación durante carga/generación; mantener un único propietario del backend y sus logs. |
| **P2** | Compatibilidad futura de catálogos y otras serializaciones válidas. | Resolver los dos casos de la tabla anterior antes de ampliar paquetes admitidos y conservar el v5 como regresión. |

## Qué queda para Tauri y Android

- **Tauri:** trabajador único, cola acotada, cancelación accesible mientras trabaja y exposición al frontend únicamente de `Proposal`/`Explanation` validadas. Integrar recuperación ante errores sin registrar prompts ni salidas.
- **Distribución:** fijar manifiestos confiables, versiones nativas y política de actualización; verificar tamaño y SHA-256 antes de cargar.
- **Android:** fijar NDK/ABI y comprobar carga, LoRA y ambas gramáticas en dispositivo real. Medir PSS/RSS, picos, latencia, temperatura, cancelación y recuperación ante presión de memoria.
- La aceptación corresponde al **motor de escritorio**. La paridad 40/40 no acredita validación clínica ni viabilidad móvil.