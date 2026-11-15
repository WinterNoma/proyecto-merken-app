# Revisión 7 (final) de la Fase 0 del SLM — Sol

Commit revisado: `190a2e8`. Revisión estática en solo lectura; el orquestador ejecutó clippy y los 86 tests de `merken-core`.

> Nota del orquestador: después de esta revisión, el commit `689453d` (Fase 1) validó ambas gramáticas con el parser GBNF real de llama.cpp y obtuvo 40/40 de paridad con el v5 (ver `docs/slm-motor-escritorio.md`), lo que cierra el riesgo «Gramáticas verificadas mediante reconocedor manual».

**Veredicto: sí, se acepta la Fase 0 en `190a2e8`: los dos P1 de la revisión 6 están cerrados; quedan dos P2 no bloqueantes con plan.**

## Estado de los P1

Revisión exclusivamente de lectura, sin modificar archivos. Contrasté HEAD mediante sus objetos Git: cambia `interpret/extract.rs` y añade pruebas en `proposal.rs`; los archivos revisados coinciden con HEAD, normalizando CRLF.

El entorno bloquea la creación de procesos (`Access denied`/`EPERM`). Mis conclusiones proceden del recorrido estático; los **86 tests de `merken-core` en verde y Clippy sin avisos** son evidencia ejecutada comunicada por el orquestador.

| P1 de la revisión 6 | Estado | Evidencia y variantes |
|---|---|---|
| **La corrección de un síntoma invalida otra zona corporal** | **Cerrado** | `dolor de cabeza 6 y dolor de espalda 4, perdón dolor de cabeza 7` deja cabeza pendiente y conserva espalda 4. Concuerda para el orden inverso, corrección de espalda, equivalentes ingleses y pie izquierdo/derecho. La identidad se obtiene del texto original, por zona o descripción; una referencia inespecífica como `perdón dolor 7` deja pendientes todos los candidatos afectados. Véase [extract.rs:2398](../crates/core/src/interpret/extract.rs:2398). |
| **La rectificación enumerada solo alcanza al primer indicador** | **Cerrado** | En `pulso 72 y glucosa 95 mg/dL, corrijo pulso 82 y glucosa 105 mg/dL`, ambas alternativas de ambos indicadores quedan pendientes; `check` rechaza recuperarlas mediante citas recortadas. Concuerda con ambos órdenes, `y`/`and`/comas y marcadores separados por coma. La matriz de 336 combinaciones contiene resultados esperados independientes. Una medición nueva como `body temperature 37` se conserva, incluso intercalada entre indicadores corregidos. Véanse [extract.rs:504](../crates/core/src/interpret/extract.rs:504) y [proposal.rs:2291](../crates/core/src/proposal.rs:2291). |

**No encontré reaparición de los contraejemplos cerrados en las revisiones 1–6:**

- Se mantienen atribución local, rechazo de signos y números fragmentarios, fracciones, horas completas/incompletas y unidades incompatibles. Permanecen las defensas frente a `pulso desconocido tras 6`, `37 °C/kg`, `fahrenheits` y citas que ocultan contexto.
- Se conservan negación local, sueño `good` en `dormí bien sin interrupciones`, sueño `poor` en `no dormí bien`, validación individual de campos y rechazo de ventanas inventadas. `dolor de cabeza grado 6` sigue siendo puntuación de dolor.
- `merken-slm` no cambió desde la revisión 5: permanecen selección literal de frases curadas, restricciones de estado, errores constantes, tipos verificados opacos, confinamiento, par base–adaptador, exclusión de raíces solapadas y eliminación cancelable sin escritura reentrante cruzada.

## Hallazgos nuevos

Los resultados de esta tabla son de recorrido estático; no están acreditados por una ejecución Rust propia.

| Prioridad | Hallazgo y contraejemplo | Plan y criterio de cierre |
|---|---|---|
| **P2 — El ámbito propagado puede vaciar un sueño independiente** | **`pulso 72, corrijo pulso 82 y dormí mal, 5 horas`**. `dormí mal` inicialmente conserva `quality=poor`, pero `5 horas` hereda el ámbito de rectificación: se bloquea la continuación normal de sueño y se ejecuta una corrección implícita sobre ese sueño. Termina sin calidad ni duración; el fragmento permanece en `uninterpreted`. Véanse [extract.rs:2581](../crates/core/src/interpret/extract.rs:2581) y `:2622`. | Distinguir el ámbito enumerativo de la pertenencia de cada registro. Permitir la continuación inequívoca del sueño independiente **sin desactivar** las correcciones posteriores de otros indicadores. Añadir regresiones ES/EN con `y`/`and`/comas: sueño `hours=5`, `quality=poor`, pulso pendiente. |
| **P2 — Cobertura térmica desigual ES/EN, preexistente** | **`temperatura 37`** se reconoce; **`temperature 37`** queda sin interpretar. También falta respaldo para que una propuesta de capa 2 recupere esa entrada mediante `check`. `body temperature 37` y `body temp 37` sí están reconocidos. Véase [lexicon.rs:2123](../crates/core/src/interpret/lexicon.rs:2123). | Documentar la cobertura y ampliar el reconocimiento compartido con casos corporales positivos y contrastes ambientales, como `cabin temperature`, además de escalas incompatibles. Añadir una prueba negativa explícita para el alias actualmente no soportado. |

**El cambio del dato del test a `body temperature 37` es correcto:** permite comprobar una temperatura reconocida sin mezclar ese objetivo con una carencia léxica. La asimetría merece el P2 documentado, pero no invalida el cierre.

## Riesgos residuales aceptables

Los dos P2 anteriores reducen el autocompletado y conservan el texto para revisión; no encontré que introduzcan valores incorrectos presentados como completos. Conforme al criterio de esta última ronda, son aceptables con sus planes.

Además, permanecen:

| Riesgo P2 | Plan |
|---|---|
| Identidad sin zona y paráfrasis limitadas | Ampliar corpus ES/EN con resultados independientes; mantener pendientes los casos sin identidad inequívoca. |
| Cancelación cooperativa | Medir interrupción y latencias con el backend real, incluyendo fábricas que no cooperan. |
| Gramáticas verificadas mediante reconocedor manual | Validar ambas con el parser GBNF del motor real. |
| Dependencia del catálogo curado y almacenamiento privado | Revisar frases autorizadas y verificar la frontera de almacenamiento en Android. |

## Qué queda para la Fase 1

- Integrar el motor real, abrir GGUF y verificar la aplicación efectiva del LoRA.
- Validar gramáticas y ejemplos con el backend real.
- Integrar Tauri/Android, almacenamiento privado y ciclo de modelos.
- Presentar citas, fragmentos pendientes y confirmación por campo, con fallback visible sin modelo.
- Ejecutar los planes P2 y medir cancelación, latencia, RAM y comportamiento térmico en dispositivo.