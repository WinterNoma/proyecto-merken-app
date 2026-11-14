# Revisión de Sol: catálogo v2 (headache fuera, 4 indicadores nuevos)

Fecha: 14-nov-2026 · Revisor: Sol (gpt-6.1-sol, high, solo lectura) · Informe copiado por Claude desde el último mensaje de Sol.

**Veredicto: no aprobar todavía la integración con backend.** La evaluación del catálogo nuevo es coherente con las decisiones indicadas, pero quedan **dos bloqueantes de compatibilidad**, dos importantes y tres menores.

Revisión exclusivamente de lectura. Los **35 bloques del diff suministrado coinciden con los archivos actuales**; confirmé la referencia a la rama `desarrollo`. No ejecuté compilación, tests, Clippy ni TypeScript: los resultados de Claude son antecedentes aportados, no verificaciones de esta revisión.

**B1 — Un `headache` histórico recibido por `pull` impide avanzar la sincronización.**

Referencias: [records.rs:816](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/records.rs:816), [records.rs:765](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/records.rs:765), [records.rs:787](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/records.rs:787).

`pull_record` deserializa directamente a `RecordDto`, cuyo `Indicator` ya no admite `headache`. Devuelve `invalid_schema`; el `?` deshace la página completa y conserva el cursor anterior. Por tanto, una página con un `headache` antiguo y registros nuevos válidos fallará repetidamente.

Ignorarlo en `load_rows` protege la lectura local, pero no esta entrada.

**Corrección concreta:** incorporar una ruta explícita de compatibilidad para registros históricos `headache`: validar su estructura antigua, conservar íntegro el payload en `pulled_changes` y permitir confirmar la página y avanzar el cursor, sin materializarlo como indicador vigente. Mantener el rechazo para payloads realmente inválidos. Añadir una regresión con una página mixta y reaplicación idempotente.

**B2 — Se encola `reason: not_applicable`, que el contrato de evaluaciones no admite.**

Referencias: [records.rs:533](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/records.rs:533), [records.rs:595](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/records.rs:595), [modelo-datos-backend.md:168](C:/Users/Salva/Desktop/git/nasa.spaceapps/docs/api/modelo-datos-backend.md:168).

`persist_changed` omite únicamente `NoData`. La primera lectura real de cada indicador contextual genera una evaluación persistida y encolada con `not_applicable`. Sin embargo, §3 permite solamente `no_data`, `too_few_data` y `stale`. La ampliación de §5 no actualiza ese contrato.

La incompatibilidad documental es comprobable; no verifiqué que un backend desplegado rechace efectivamente el payload.

**Corrección concreta recomendada para esta entrega:** excluir también `NotApplicable` de `persist_changed`, manteniendo estas evaluaciones en la respuesta de interfaz y subiendo sus registros normalmente. Si se necesita auditar esas evaluaciones en Tierra, ampliar conjuntamente §3 y el validador del backend antes de enviarlas. La prueba debe inspeccionar la outbox de `evaluation`, no solo contar registros.

**I1 — Las filas antiguas ignoradas siguen pudiendo salir por la outbox.**

Referencias: [records.rs:159](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/records.rs:159), [lib.rs:211](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/lib.rs:211).

`load_rows` omite `headache`, pero `pending_outbox` devuelve los payloads originales sin filtrar el catálogo. Una instalación anterior puede dejar invisible el registro local y seguir ofreciendo para envío un `record` con `indicator: headache`, fuera del contrato actual. Las evaluaciones antiguas asociadas tampoco reciben tratamiento específico.

**Corrección concreta:** definir una migración de compatibilidad para las entradas pendientes del catálogo retirado, conservando su contenido y dejando explícito su estado de archivo/no envío. No convertir silenciosamente un payload ya identificado en otro indicador. Probar la reapertura de una base antigua con registros y outbox pendientes.

**I2 — La justificación FDA del corte de plausibilidad del 80 % está mal atribuida.**

Referencias: [umbrales-core.md:20](C:/Users/Salva/Desktop/git/nasa.spaceapps/docs/api/umbrales-core.md:20), [umbrales-hydrogen-v2.md:14](C:/Users/Salva/Desktop/git/nasa.spaceapps/reviews/umbrales-hydrogen-v2.md:14).

La documentación afirma que FDA 2013 establece precisión degradada por debajo del 80 %. La sección 4.1.1 consultada describe especificaciones de precisión para **70–100 %**, sin establecer ese punto de degradación en el 80 %. Tampoco valida el 80 % como umbral de plausibilidad. [Guía FDA de 2013](https://www.fda.gov/regulatory-information/search-fda-guidance-documents/pulse-oximeters-premarket-notification-submissions-510ks-guidance-industry-and-food-and-drug).

**Corrección concreta:** conservar `< 80` como `demo_heuristic`, pero escribir: «Corte de confirmación elegido por Merken, sin validación publicada específica». Presentar FDA únicamente como referencia técnica de precisión y corregir también la atribución en Hydrogen. No hace falta cambiar el número por esta observación.

**M1 — Las evaluaciones contextuales conservan metadatos de ausencia de datos.**

Referencias: [evaluate.rs:135](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/core/src/evaluate.rs:135), [evaluate.rs:143](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/core/src/evaluate.rs:143).

La salida contextual conserva `summary: "No data available"` y `history_needed: 3`, incluso cuando incluye `latest_value`. Además, hereda `source_kind: demo_heuristic` y una referencia nula para una exclusión definida por protocolo. El contrato de interfaz permite mostrar `summary` directamente.

**Corrección concreta:** establecer un resumen contextual explícito, por ejemplo `"Context only; health evaluation does not apply"`, `history_needed: 0` y procedencia `approved_protocol` con referencia a la decisión del catálogo.

**M2 — Quedaron descripciones contradictorias en TS y la decisión 003.**

Referencias: [core-api.ts:220](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/core-api.ts:220), [003-nucleo-rust.md:19](C:/Users/Salva/Desktop/git/nasa.spaceapps/docs/decisiones/003-nucleo-rust.md:19), [003-nucleo-rust.md:32](C:/Users/Salva/Desktop/git/nasa.spaceapps/docs/decisiones/003-nucleo-rust.md:32), [003-nucleo-rust.md:56](C:/Users/Salva/Desktop/git/nasa.spaceapps/docs/decisiones/003-nucleo-rust.md:56).

El comentario TS promete siempre siete indicadores, aunque ahora pueden salir diez. La decisión 003 omite los cuatro nuevos tipos de unidad, sus restricciones duras y `not_applicable` en la enumeración de motivos. Su algoritmo comienza con `no_data`, aunque una llamada directa para contexto vacío devuelve `not_applicable`.

**Corrección concreta:** documentar «siete indicadores médicos siempre, más hasta tres contextuales con datos efectivos»; añadir las unidades y restricciones nuevas y describir la salida contextual antes del algoritmo médico.

**M3 — La cobertura nueva no comprueba varios contratos centrales del cambio.**

Referencias: [flow_tests.rs:722](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/flow_tests.rs:722), [tests.rs:323](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/core/src/tests.rs:323).

El test contextual verifica presencia de valor y exclusión de `worst`, pero no comprueba ausencia sin datos, el resultado global completo ni las evaluaciones encoladas. Los tests de SpO₂ cubren 90/91, 93 y 101, pero faltan fronteras y comportamiento longitudinal específicos.

**Corrección concreta:** añadir regresiones focalizadas para B1/B2 y para: contexto vacío; `overall` formado exclusivamente por contexto; series médicas estables con contexto; SpO₂ 94 y 100; rechazo de 0 y negativos; caída exacta de tres puntos; alarma de SpO₂ con más de 48 horas; confirmaciones a ambos lados de 80, 6000 y 10000.

**Comportamiento correcto confirmado por inspección**

| Caso | Resultado del código actual |
|---|---|
| Contexto sin datos efectivos en `evaluate_all` | Se omite. |
| `evaluate_series` contextual vacío | `not_evaluated / not_applicable`, sin valor. |
| `overall` recibe únicamente evaluaciones contextuales | `not_evaluated`, `continue`, `worst: []`. |
| Solo existen registros contextuales en `get_status` | Las siete series médicas quedan `no_data`; contexto excluido de `worst`. |
| Series médicas estables más contexto | El contexto no impide `stable`. |
| SpO₂ ≤ 91 | Alarma urgente, también sin historial; sobrevive a la caducidad. |
| SpO₂ > 91 y < 94 | `change` descendente; puede escalar por persistencia. |
| SpO₂ 94 aislada | `too_few_data`, sin afirmar estabilidad. |
| Plausibilidad | `< 80`, `> 6000` y `> 10000` son estrictos; los valores exactos no generan esos avisos. |
| Validación dura | Porcentaje finito en `(0,100]`; las otras tres unidades, finitas y ≥ 0. |

Los cortes absolutos de SpO₂ tienen respaldo identificable: NEWS2 escala 1 asigna tres puntos a ≤91 %, y BTS establece el objetivo 94–98 %. Su aplicación y escalado en Merken siguen siendo una adaptación de la app. [NEWS2](https://www.rcplondon.ac.uk/media/ctulqqbn/news2-executive-summary_0.pdf), [BTS 2017](https://doi.org/10.1136/thoraxjnl-2016-209729).

La caída personal de tres puntos, las 48 horas y las plausibilidades nuevas están correctamente presentadas como **heurísticas**, sin convertir sus referencias orientativas en validación del algoritmo. También queda explícitamente pendiente la integración de tasa de radiación en dosis acumulada.

No encontré nuevas llamadas prohibidas a `unwrap`, `expect`, `panic!` ni indexación insegura en producción dentro del cambio. Los usos añadidos están en tests con las excepciones correspondientes; `f64::INFINITY` en `history` contextual no se alcanza desde su evaluación, que retorna antes.

Finalmente, `telemetry.rs` permanece como un modelo diferente del contrato por registros: ánimo textual, campos planos y ausencia de ventana de sueño. No hallé una conexión suya con el evaluador. La pantalla actual consume `ui/src/types.ts` y `/api/astronauts`, sin usar `core-api.ts`; por ello, esta revisión **no acredita integración visual de los estados nuevos**, aunque los tipos del wrapper sí coinciden con las salidas Rust revisadas.
---

## Respuesta de Claude

| Hallazgo | Corrección |
|---|---|
| B1 | `pull_record` acepta un `headache` histórico bien formado (`record_id`, `unit: score_0_10`, `score` 0–10): queda íntegro en `pulled_changes`, no se materializa y el cursor avanza. Uno mal formado sigue rechazando la página. Test `pull_page_with_retired_headache_advances_and_keeps_it` (página mixta + reaplicación). |
| B2 | `persist_changed` no guarda ni encola evaluaciones `not_applicable`; los registros de contexto sí se suben. Test `context_evaluations_are_never_queued`. |
| I1 | Migración v3: lo pendiente en la outbox con `indicator: headache` (registros y evaluaciones) pasa a `failed` con `retired_indicator…`; se conserva y no se envía. Test `migration_v3_parks_pending_headache_outbox`. |
| I2 | `umbrales-core.md`: `< 80 %` queda como corte de Merken sin validación publicada; FDA solo como referencia técnica (70–100 %). Nota añadida al final de `umbrales-hydrogen-v2.md`. |
| M1 | La evaluación de contexto lleva `summary: "Context only; health evaluation does not apply"`, `history_needed: 0`, `source_kind: approved_protocol` y `source_ref` de la decisión del catálogo. |
| M2 | `core-api.ts` ("7 médicos siempre + hasta 3 de contexto con datos") y decisión 003 (unidades nuevas, paso 0 de contexto, `not_applicable`, migración v3). |
| M3 | Tests nuevos: límites duros (0, negativos, 100.1), plausibilidad a ambos lados de 80, 6000 y 10000, `overall` solo con contexto. Ya estaban cubiertos 91/93/101 y la alarma que sobrevive a la caducidad. |

Estado tras las correcciones: `cargo test --workspace` 53 tests en verde, `cargo clippy --workspace --all-targets -- -D warnings` limpio, `npx tsc --noEmit` sin errores.
