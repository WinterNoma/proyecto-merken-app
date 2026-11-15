# Revisión de Sol — F0 SLM, sexta ronda

HEAD `3202d0a` (15-nov-2026).

**Veredicto: no se acepta todavía la Fase 0: los dos P1 de la revisión 5 están corregidos, pero quedan dos P1 adicionales en la resolución de correcciones; no encontré P0 nuevos.**

## Estado de los P1

Revisé **HEAD `3202d0a`**, sus cuatro archivos modificados y las revisiones anteriores. Los archivos de ambos crates coinciden con los objetos Git, normalizando CRLF. **No modifiqué archivos.**

El entorno bloquea la creación de procesos (`Access denied`/`EPERM`): los resultados siguientes proceden de **recorrido estático**, sin ejecución Rust propia. Clippy limpio y los **79 tests de core y 52 de slm** son evidencia comunicada por el orquestador. En las referencias, `core/` significa `crates/core/src/`.

| P1 de la revisión 5 | Estado | Evidencia y variantes |
|---|---|---|
| **N1 — Corrección explícita que no alcanza una mención anterior del indicador** | **Cerrado para el hallazgo** | En `radiación 0.42 uSv/h y pulso 72, perdón radiación 0.84 uSv/h`, ambas radiaciones quedan pendientes y pulso conserva 72. `previous_mention` busca hacia atrás el indicador nombrado, independientemente de los indicadores intermedios (`core/interpret/extract.rs:2359`, `:2610`). El recorrido concuerda para ambos órdenes, varios indicadores intermedios, correcciones sin coma y marcadores separados por comas. Los tests contrastan los once indicadores y preservan valores ajenos (`core/proposal.rs:1320`, `:1645`). |
| **Una puntuación de dolor se convierte en temperatura** | **Cerrado** | `dolor de cabeza grado 6` conserva `body_zone=head`, `score=6` y descripción literal, sin temperatura. También quedan cubiertos `grados`, `nivel`, `punto`, `degree`, `grade`, `level` y `rate`. Una propuesta SLM de Celsius 6 con cita recortada se rechaza. Se conservan positivos como `temperatura 37 grados`, `37 °C`, `37 degrees`, `frecuencia cardíaca 72` y `presión 120/80` (`core/interpret/lexicon.rs:1993`; `core/interpret/tests.rs:879`, `:1006`; `core/proposal.rs:1870`). |

**El ajuste del test es aceptable.** En `pulso 72 y glucosa 95 mg/dL, perdón 82 o glucosa 105 mg/dL`, aplicar la corrección sin etiqueta a la última mención deja glucosa pendiente y conserva pulso 72. No se elige entre 82 y 105 ni se inventa una referencia al pulso. Mantendría esa regla, mostrando el fragmento ambiguo durante la confirmación por campo.

**Repaso de los cierres anteriores:**

- Se mantienen los resultados esperados para atribución local, signos, números fragmentarios, fracciones, horas completas e incompletas y unidades incompatibles: `tras 6 ejercicios pulso 72`, `pulso desconocido tras 6`, `20 minutos`, `98.6 F`, `5,5 mmol/L`, `37 °C/kg` y `fahrenheits`.
- Se conservan negación local, sueño `good` en `dormí bien sin interrupciones`, sueño `poor` en `no dormí bien`, rechazo de ventanas inventadas y validación individual aunque existan campos `null`.
- `merken-slm` no cambió respecto de la revisión 5. Se mantienen selección literal de frases curadas, tipos verificados opacos, confinamiento, par base–adaptador, exclusión de raíces solapadas, prevención del interbloqueo cruzado, eliminación cancelable y errores constantes.

No encontré reaparición de los contraejemplos cerrados. Las variantes adicionales de corrección se detallan a continuación.

## Hallazgos nuevos

Son **defectos preexistentes recién identificados**, también presentes en el padre del commit; no atribuyo su introducción a `3202d0a`. Ambos dejan valores rectificados como campos completos aceptados por `check`, por lo que cumplen el criterio P1 pese a requerirse confirmación.

| Prioridad | Archivo:línea | Problema y contraejemplo | Corrección y criterio de cierre |
|---|---|---|---|
| **P1 — Una corrección de síntoma invalida otra zona corporal** | `core/interpret/extract.rs:2368`, `:2613`; `core/proposal.rs:227` | **`dolor de cabeza 6 y dolor de espalda 4, perdón dolor de cabeza 7`**. El destino se busca únicamente por `Indicator::Symptom`: selecciona la espalda, por ser el último síntoma. Se borran sus campos y se descarta la nueva puntuación de cabeza, pero **cabeza 6 permanece completa y respaldada por `check`**, aunque acaba de corregirse. La variante inversa —espalda primero, cabeza después y corrección explícita de espalda— presenta el mismo problema. | Considerar la identidad explícita del síntoma, incluida la zona corporal. Si no puede resolverse, dejar pendientes los candidatos afectados. Regresión dirigida: rechazar las puntuaciones de cabeza corregidas y conservar espalda 4; repetir con órdenes inversos y equivalentes ingleses. |
| **P1 — Una rectificación enumerada solo alcanza al primer indicador** | `core/interpret/extract.rs:476`, `:2572`, `:2600`; `core/proposal.rs:488` | **`pulso 72 y glucosa 95 mg/dL, corrijo pulso 82 y glucosa 105 mg/dL`**. La conjunción divide la rectificación en dos cláusulas. Solo la primera conserva el marcador: pulso queda pendiente, pero **glucosa 95 y glucosa 105 permanecen como dos registros completos**, ambos aceptados por `check`. La variante con `perdón` produce el mismo resultado. | Conservar el ámbito de la rectificación durante la enumeración y comprobar cada indicador repetido; ante alcance dudoso, mantener pendientes las alternativas. Añadir ambos órdenes y `y`/`and`, contrastando con una medición adicional de un indicador que no se estuviera corrigiendo. |

## Riesgos residuales aceptables

- **Alias y lenguaje finitos — P2:** no encontré otra propuesta incorrecta causada por el filtro nuevo entre las variantes revisadas. Plan: ampliar el corpus ES/EN con resultados independientes, combinaciones de indicadores y contrastes entre alias genéricos y menciones calificadas.
- **Cancelación cooperativa:** una fábrica que no coopera puede superar el plazo. Plan: medir interrupción y latencias con el backend real.
- **Gramáticas:** el reconocedor manual no sustituye al parser GBNF real. Plan: validar ambas gramáticas y ejemplos durante la integración del motor.
- **Catálogo y almacenamiento:** la seguridad semántica depende de las frases curadas y la protección externa depende del almacenamiento privado Android. Plan: verificar ambos en su integración.

## Qué queda para la Fase 1

Los **dos P1 de la tabla deben cerrarse en Fase 0**, con regresiones Rust ejecutadas.

Para Fase 1 quedan:

- Motor real, apertura de GGUF y aplicación verificable del LoRA.
- Validación de gramáticas con el parser y backend reales.
- Integración Tauri/Android, almacenamiento privado y ciclo de modelos.
- Formularios con citas, ambigüedades y confirmación por campo; fallback visible sin modelo.
- Cancelación, latencia, RAM y comportamiento térmico en dispositivo.