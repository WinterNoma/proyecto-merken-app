# Fase 2 del SLM — revisión 3 de Sol (commit a12bcec)

**Veredicto: no acepto la Fase 2 en `a12bcec`: persiste un P0 de pérdida del signo numérico cuando está separado por espacios.**

## Estado de los hallazgos de la revisión 2

| Hallazgo | Estado |
|---|---|
| **P0: cita escalar amplia oculta contenido desconocido** | **Resuelto.** `field_evidence` acredita expresiones que respaldan el valor, sin consumir automáticamente toda la cita (`service_residual.rs:246`). `no dormí bien estoy agotado` conserva `estoy agotado`; también hay regresiones EN, mayúsculas y acentos combinados. |
| **P0: pérdida de signos numéricos** | **Parcialmente resuelto.** Se conservan `-0,5`, `−72` y `+72`. Los signos separados siguen desapareciendo del pendiente; detalle abajo. |
| **P2: residuos espurios de alias compuestos** | **Resuelto para los alias incorporados.** La lista cerrada amplía menciones como `blood glucose` y `estado de ánimo`, conservando vecinos desconocidos (`service_residual.rs:158`, `:209`). Queda mantenerla sincronizada con el núcleo. |

Las correcciones anteriores de cancelación por identidad, Store dentro de `spawn_blocking`, recuperación del trabajador, espera acotada y contrato de respuesta permanecen presentes. No identifiqué regresiones en esas rutas.

## Hallazgos nuevos

Los contraejemplos siguientes son **deducciones estáticas**, salvo las cifras de evaluación aportadas por el orquestador.

| Prioridad | Archivo:línea | Contraejemplo | Corrección |
|---|---|---|---|
| **P0 — variante pendiente del hallazgo anterior** | `crates/slm/src/service_residual.rs:31`, `:46`, `:401`; contraste: `crates/core/src/interpret/numbers.rs:481` | **`temperatura - 0,5`** deja `celsius: null`, `uninterpreted: []` y **`unresolved: ["0,5"]`**. **`pulse − 72`** deja análogamente `["72"]`. El núcleo reconoce y rechaza el signo separado; el residuo solo tokeniza el signo si el siguiente carácter es un dígito. El signo queda fuera del slice pendiente. La cita original lo conserva, pero esto reproduce la corrupción del pendiente considerada P0 en la revisión 2. | Conservar el span de signo, espacio y número cuando el núcleo lo trata como expresión numérica rechazada. Añadir regresiones con `-`, `−`, `+`, espacios y tabulaciones, comprobando citas reales y conservación literal del pendiente. |
| **P1 — utilidad insuficiente del intérprete elegido** | `crates/slm/src/service.rs:51`; `src-tauri/src/lib.rs:532` | El intérprete predeterminado es Granite, cuya medición aportada recupera **2/40 valores esperados**. El saneado evita presentar sus invenciones, pero la mayoría de los valores esperados no se recupera. Qwen3 base logra 13/40 y tampoco acredita suficiencia. | Establecer un umbral de utilidad y latencia para integración; mejorar o sustituir el intérprete mediante un nuevo paquete de confianza evaluado. Mantener disponible la resolución manual del pendiente. |
| **P2 — falta medición del aporte efectivo del servicio** | `crates/slm/examples/granite_eval.rs:245`; `crates/slm/src/service.rs:573` | El ejemplo mide `SlmInterpreter` directamente. Un acierto sobre un indicador ya reconocido por reglas cuenta en su recall, aunque `filter_layer2` lo excluiría de la respuesta del servicio. Por tanto, **2/40 y 13/40 no miden la recuperación adicional efectiva**. | Añadir evaluación de `InterpretResponse`: propuesta de reglas, candidatos saneados, candidatos admitidos y pendientes conservados. Mantener separada la medición directa del motor. |

## Riesgos residuales y pendientes para UI/Android

**Plantillas y confianza.** No encontré una apertura nueva: la selección admite exactamente `qwen3` y `granite`; otras arquitecturas fallan antes de cargar el modelo nativo (`llama.rs:104`). La lectura rechaza claves duplicadas, tipos incorrectos y estructuras fuera de sus límites. LoRA sigue exigiendo base Qwen3 (`gguf.rs:262`), por lo que Granite con adaptador se rechaza. La aplicación usa el manifiesto embebido y verifica tamaño/SHA-256 antes de leer la arquitectura y cargar bajo el bloqueo del almacén. La cabecera selecciona la plantilla; no sustituye la autoridad del manifiesto.

Permanece la frontera ya documentada: ese bloqueo protege frente a escritores cooperantes. Android debe mantener los modelos en almacenamiento privado.

**Prompt y saneado.** No encontré una vía nueva para introducir valores sin respaldo. El prompt no constituye la garantía: la aportan `Proposal::check`, el saneado y la comprobación final del filtro. Se comprueban citas, números, unidades, atribución, negaciones y enums; las ventanas de sueño no pueden fijarse desde el modelo. La capa 2 conserva separados los resultados deterministas y no elimina sus pendientes. Los **cero valores numéricos inventados finales** medidos respaldan esa muestra, sin demostrar cobertura universal.

**Integración pendiente:**

- Mostrar el texto original, citas, ambas propuestas, `unresolved` y `proposal.uninterpreted`; exigir confirmación de cada registro antes de guardar.
- Ignorar respuestas y eventos de solicitudes abandonadas; mantener IDs nuevos por solicitud.
- Validar memoria, latencia, cancelación y ciclo de vida en Android, incluida la coordinación con Whisper. Una llamada nativa colgada sigue pudiendo retrasar la descarga.
- Incorporar pruebas negativas específicas de cabeceras manipuladas y LoRA sobre Granite; la implementación contiene los rechazos, pero la matriz nueva de pruebas es limitada.
- Mantener pendiente la aplicabilidad de fichas por indicador y estado antes de poblar el catálogo.

**Alcance de verificación:** revisión íntegramente en solo lectura. La creación de procesos fue rechazada con `Access denied`; no ejecuté Git, Rust ni pruebas. Leí HEAD y su árbol desde los objetos Git y comprobé que los archivos revisados coinciden, normalizando CRLF. Fmt, Clippy y los **85/92 tests verdes** son evidencia aportada por el orquestador; las regresiones actuales de signos cubren formas pegadas, sin cubrir el P0 separado descrito arriba.