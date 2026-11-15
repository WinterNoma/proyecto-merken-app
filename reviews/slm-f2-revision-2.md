# Fase 2 del SLM — revisión 2 de Sol (commit 3691410)

## Veredicto

**No se acepta todavía la Fase 2 en `3691410`: quedan dos P0 en la conservación del contenido pendiente.**

## Estado de los hallazgos de la revisión 1

| Prioridad original | Hallazgo | Estado en HEAD |
|---|---|---|
| P0 | Citas que eliminan subcadenas de otras palabras o números | **Resuelto para el defecto original.** `occurrences` compara secuencias completas y conserva offsets UTF-8; `"72"` ya no cubre parte de `"172"`. Las ocurrencias repetidas de evidencia se mantienen pendientes conservadoramente. Véase el nuevo problema con signos numéricos. |
| P0 | Cita amplia confundida con cobertura semántica | **Parcialmente resuelto.** Las citas de registro ya no cubren por sí solas toda la cláusula. Sin embargo, cualquier cita de campo sigue acreditando todas sus palabras, incluso cuando el valor solo representa una parte del significado. Persiste mediante `quality` de sueño. |
| P1 | Cancelación sin identidad del trabajo | **Resuelto desde la admisión.** ID validado, token creado antes de encolar y cancelación sobre la entrada concreta. La limpieza comprueba identidad mediante `Arc::ptr_eq`; un cancel tardío de A no alcanza B con otro ID. No encontré fugas de tokens en terminación, rechazo de admisión o desenrollado. |
| P1 | Store bloqueante antes de `spawn_blocking` | **Resuelto.** Adquisición del mutex y `Store::status` están dentro del cierre bloqueante. `with_store` libera la guardia antes de `handle_explain` (`src-tauri/src/lib.rs:153`). |
| P1 | Muerte del trabajador inutiliza respuestas deterministas | **Resuelto para desconexión y pánicos desenrollables.** Propuesta, residuo y hechos se calculan fuera del trabajador; `Pending` conserva el fallback. El relanzamiento exige `handle.is_finished()`, después de destruir la ranura anterior. |
| P2 | Negaciones entendidas provocan inferencias inútiles | **Resueltos los ejemplos originales.** `no headache` y `no tengo fiebre` quedan cubiertos; las negaciones con contenido desconocido vuelven pendientes completas. Quedan falsos pendientes con algunas menciones compuestas. |
| P2 | Espera ilimitada pese al plazo cooperativo | **Resuelto en los comandos.** Se usa `recv_timeout` con tiempo restante más dos segundos. El timeout devuelve el resultado determinista, cancela el token y mantiene `busy` mientras ese trabajo siga vivo. |
| P2 | Contrato TS impreciso | **Resuelto para las respuestas expuestas.** Coinciden `FactId`, etiquetas de `TimeHint`, estados, propiedades serializadas y valores `null`; los argumentos opcionales admiten `null`. |
| P2 | Documentación y mensajes dispersos | **Resuelto**, salvo ajustar el contrato del residuo tras corregir los P0 siguientes. Los mensajes de la frontera Tauri están centralizados en inglés; el documento describe confirmación, cancelación, fallback, liberación y progreso. |

## Hallazgos nuevos

Los resultados siguientes se deducen del recorrido estático del intérprete y del servicio; no son ejecuciones de Rust realizadas durante esta revisión.

| Prioridad | Hallazgo con archivo:línea | Contraejemplo | Corrección propuesta |
|---|---|---|---|
| **P0** | **Una cita amplia de campo escalar todavía oculta contenido no interpretado.** `crates/slm/src/service_residual.rs:194`, `:205`, `:213`. El origen concreto está en `crates/core/src/interpret/extract.rs:1481`: la calidad negativa de sueño cita toda la cláusula. | **`no dormí bien estoy agotado`** produce un registro de sueño con `quality: "poor"` y cita de campo igual a toda la frase. No hay campo que represente `estoy agotado`, pero toda la cita queda cubierta: **`unresolved: []`, `uninterpreted: []`, `layer2: "not_needed"`**. La misma vía afecta a **`I did not sleep well I feel exhausted`**, y sobrevive a mayúsculas y acentos. El texto permanece como evidencia, pero el flujo lo considera resuelto silenciosamente. | Separar los spans realmente consumidos de las citas de evidencia, también para campos. Para `quality`, cubrir únicamente la construcción acreditada de sueño; dejar pendiente la cola desconocida. Añadir regresiones ES/EN con citas reales de capa 1. |
| **P0** | **El residuo pierde signos numéricos al tratarlos como puntuación sin contenido.** `crates/slm/src/service_residual.rs:24`, `:30`, `:266`. | **`temperatura -0,5`**: el núcleo rechaza el número con signo y devuelve el campo `celsius` sin valor. El servicio cubre `temperatura`, pero devuelve **`unresolved: ["0,5"]`**, eliminando el signo del dato pendiente. **`pulse -72`** deja análogamente `["72"]`; el signo multibyte **`−`** también se pierde. La cita original conserva el signo, pero el pendiente presentado queda corrompido. | Conservar signos numéricos en los tokens y slices pendientes, alineándose con la tokenización del núcleo. Añadir regresiones con `-`, `−`, `+` y decimales con punto/coma; comprobar la conservación literal del pendiente. |
| **P2** | **Las menciones mínimas generan residuos espurios en nombres compuestos válidos.** `crates/slm/src/service_residual.rs:153`. | **`blood glucose 95`** se interpreta, pero `glucose` constituye una mención más corta y evita cubrir `blood glucose`; queda **`unresolved: ["blood"]`**. **`estado de ánimo 6`** deja `estado`. Esto puede activar innecesariamente la capa 2 y ensuciar el formulario. | Usar spans de las menciones reconocidas por el núcleo. Como mejora acotada, reconocer expresamente los alias compuestos autorizados, conservando pendientes sus vecinos desconocidos. |

## Riesgos residuales

- **Alcance de la evidencia:** no modifiqué archivos. La creación de procesos fue rechazada con `Access denied`; no pude ejecutar Git, Rust ni TypeScript. Leí los objetos Git y confirmé que los siete archivos coinciden con HEAD, normalizando CRLF. Fmt, Clippy, tests y `tsc` verdes son evidencia aportada por el orquestador.
- La cita amplia de **`description` del síntoma conserva la frase completa como valor del campo** (`extract.rs:1618`). Su ausencia de `unresolved` no demuestra por sí sola pérdida de texto. El P0 de sueño es distinto: el valor `"poor"` no conserva el contenido adicional.
- La cancelación opera desde la admisión al servicio. Antes de ella —por ejemplo, esperando el pool bloqueante o el Store— el ID todavía resulta desconocido. La integración debe ignorar respuestas y eventos de solicitudes abandonadas.
- No encontré nuevas vías de guardado de propuestas sin confirmación, decisiones clínicas del SLM ni prosa libre del modelo hacia la UI. `slm-progress` contiene únicamente ID, fase y contadores.
- `slm_release` descarga la ranura en el trabajador sin cargar otro motor. Una llamada nativa colgada puede impedir esa descarga; el servicio evita reemplazarla mientras siga viva. `panic = "abort"` tampoco permite recuperación. La coordinación con Whisper y la validación de memoria/ciclo de vida Android siguen pendientes de Fase 3.
- Antes de incorporar fichas, continúa pendiente acreditar su aplicabilidad por **indicador y estado**; actualmente el catálogo vacío evita esa exposición.