# Revisión de Sol — F0 SLM, quinta ronda

HEAD `5a31f70` (15-nov-2026).

## Veredicto

**No se acepta todavía la Fase 0: N1 sigue parcialmente abierto y el cambio de reconocimiento introduce una regresión P1 de atribución; no encontré P0 nuevos.**

Revisé **HEAD `5a31f70`**, incluidos `d7647e1` y `5f4d1f1`. Los archivos de ambos crates y su configuración coinciden con los objetos Git, normalizando CRLF. **No modifiqué archivos.**

Los contraejemplos siguientes se repitieron mediante **recorrido estático**, no ejecución Rust: este entorno rechaza la creación de procesos (`Access denied`/`EPERM`). Clippy limpio y los **73 tests de core y 52 de slm** son evidencia ejecutada comunicada por el orquestador.

En las referencias, `core/` significa `crates/core/src/` y `slm/`, `crates/slm/src/`.

## Estado de los P1

| P1 de la revisión 4 | Estado | Evidencia y contraejemplos repetidos |
|---|---|---|
| **N1 — Correcciones ocultas mediante citas recortadas** | **Parcial** | El caso exacto **`radiación 0.42 uSv/h, perdón radiación 0.84 uSv/h`** ya rechaza tanto 0.42 como 0.84 citando cada ocurrencia por separado. Se invalida el segmento original aunque carezca de registro L1 (`core/interpret/extract.rs:2333`, `:2548`); `check` ya no recurre al respaldo de la cita recortada (`core/proposal.rs:244`). Los tests contrastan ambas ocurrencias con sus versiones aisladas para los once indicadores (`core/proposal.rs:1320`). **Persiste una variante bloqueante:** en **`radiación 0.42 uSv/h y pulso 72, perdón radiación 0.84 uSv/h`**, la propuesta `usv_h=0.42`, cita de registro `radiación 0.42 uSv/h` y cita de campo `0.42 uSv/h` sigue pasando. La última mención recordada es pulso (`extract.rs:2712`, `:2730`); la corrección busca únicamente esa última mención y exige que coincida con radiación (`:2542`). Por ello no invalida la radiación anterior. **Para cerrar:** resolver la referencia explícita al indicador corregido entre los segmentos anteriores; si resulta ambigua, dejar pendientes las alternativas. Añadir ambos órdenes y comprobar que pulso conserva 72. |
| **Rechazo de propuestas legítimas de capa 2** | **Cerrado para el hallazgo** | La propuesta válida de radiación pasa aislada y contra **`pulso 72 y radiación 0.42 uSv/h`**, también en orden inverso. Las menciones delimitan segmentos aunque no exista extractor de valores (`core/interpret/extract.rs:488`, `:511`, `:2680`). Los tests cubren radiación, distancia y nutrición entre sí y con pulso, con y sin conjunción, comprobando valores concretos y propuestas combinadas (`core/proposal.rs:1533`). La regresión nueva de la tabla siguiente afecta a una expresión ambigua distinta. |
| **N5 — Interbloqueo cruzado y eliminación no cancelable** | **Cerrado** | Con cualquier lock retenido por el hilo, adquirir escritura devuelve el código constante `LockReentrant`, independientemente de la raíz (`slm/store.rs:131`, `:245`). En la secuencia **T1 carga A→elimina B / T2 carga B→elimina A**, ambas eliminaciones fallan antes de esperar. El test usa dos hilos y una barrera dentro de ambas fábricas (`:1527`). `remove_package` recibe `Control` y usa el bucle `try_write` con comprobaciones y sondeo de hasta 2 ms (`:240`, `:531`). Hay comprobaciones de timeout, cancelación, conservación del archivo y eliminación posterior exitosa (`:1597`). |

**Repaso de los cierres de las revisiones 1–4:**

- **N2, negación local:** `pulso 72 sin medir glucosa` conserva pulso 72 y rechaza la propuesta positiva de glucosa. Siguen las comprobaciones dirigidas en `core/proposal.rs:1226`. Se añaden contrastes equivalentes con los tres indicadores de contexto (`core/interpret/tests.rs:849`).
- **N3, raíces solapadas:** se conserva el rechazo padre/hijo en ambos órdenes y la reutilización del lock para una raíz equivalente (`slm/store.rs:328`; test `:1120`).
- **N4, unidades:** siguen rechazados `fahrenheits`, `farenheit`, `grados f` y sus citas recortadas; se conservan `pulso 72 ahora` y `temperatura 37 grados ahora` (`core/proposal.rs:1255`; `core/interpret/tests.rs:785`).
- **N6, timeout dentro del saneado:** permanecen las aserciones de reparación efectiva antes del timeout y de errores todavía pendientes (`slm/interpreter.rs:978`, `:983`, `:989`).
- **Atribución y números:** los contraejemplos anteriores —`trabajé 5 horas antes de dormir mal`, `pulso desconocido tras 6`, corrección simple de pulso y `37 °C/kg`— mantienen sus rechazos (`core/proposal.rs:1121`). Se conservan las defensas sobre signos, fragmentos, denominadores y horas completas/incompletas. **La atribución presenta, sin embargo, la regresión nueva indicada abajo.**
- **P0 del explicador y restantes cierres:** selección, hechos, manifiesto, ranura, errores, límites y `Control` no cambiaron. `You have a clot.` y `Take aspirin [safety].` siguen fuera del esquema aceptado; una selección autorizada copia literalmente la frase curada (`slm/explain/selection.rs:482`, `:512`, `:518`). Se mantienen validación por campo, rechazo de ventanas inventadas, tipos verificados opacos, par base–adaptador, confinamiento y errores constantes. No encontré nuevas excepciones de producción a los lints heredados.

## Hallazgos nuevos

| Prioridad | Archivo:línea | Problema y contraejemplo | Corrección y criterio de cierre |
|---|---|---|---|
| **P1 — Una puntuación de dolor se convierte en temperatura** | `core/interpret/lexicon.rs:2036`; `core/interpret/extract.rs:511`, `:2251`, `:2670`; `core/proposal.rs:227` | **`dolor de cabeza grado 6`** antes conservaba `score=6`: `grado` era un enlace permitido de atribución. Ahora el reconocimiento compartido clasifica `grado` como mención de temperatura y crea un segmento propio. El síntoma queda con `score=None` y aparece **`body_temperature { celsius: 6 }`**, cita `grado 6`, campo citado por `6`. La validación admite ese Celsius positivo (`core/units.rs:32`) y `check` lo respalda con el mismo resultado L1. Por tanto, que `interpret(t).check(t)` pase no detecta el error semántico. | Desambiguar `grado` en contexto de puntuación; una mención ambigua no debe activar automáticamente extracción térmica. Añadir resultados esperados independientes: esta frase debe conservar la puntuación de dolor y no crear temperatura, manteniendo los positivos térmicos existentes. |

Este hallazgo es de **recorrido estático antes/después** y requiere una regresión Rust dirigida. Los tests verdes comunicados no contienen este caso.

## Riesgos residuales aceptables

- **Cancelación cooperativa:** no interrumpe una fábrica que deja de cooperar ni garantiza tiempo real estricto. Plan: probar interrupción y latencias con el backend real.
- **Gramáticas:** el reconocedor manual no sustituye al parser GBNF real. Plan: validar gramáticas y ejemplos al integrar el motor.
- **Catálogo y lenguaje:** vocabulario finito y autorización semántica dependiente de frases curadas. Plan: ampliar corpus ES/EN con resultados esperados independientes, además de propiedades de autoconsistencia.
- **Almacenamiento:** la defensa frente a modificaciones externas depende del almacenamiento privado Android. Plan: verificar esa frontera en dispositivo.
- **Registro global de locks:** conserva referencias débiles y capacidad hasta una limpieza posterior; no encontré una regresión en su liberación.

**Los dos problemas P1 descritos arriba son bloqueantes de Fase 0; no quedan cubiertos por estos riesgos aceptables.**

## Qué queda para la Fase 1

- Motor real, apertura efectiva de GGUF y aplicación verificable del LoRA.
- Aceptación de gramáticas por el parser y backend reales.
- Integración Tauri/Android, almacenamiento privado y ciclo de modelos.
- Formularios, confirmación por campo y fallback visible sin modelo.
- Cancelación del backend y mediciones de latencia, RAM y temperatura en dispositivo.