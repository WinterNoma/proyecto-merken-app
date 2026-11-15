# Revisión de Sol — F0 SLM, tercera ronda

HEAD `4a0df92` (15-nov-2026).

## Veredicto

**No se acepta todavía la Fase 0: los P0 del explicador quedan cerrados, pero persisten P1 en `check` y en la exclusión de carga, y aparece una regresión que rechaza propuestas legítimas de la capa 1.**

Revisé HEAD **`4a0df92`**, que integra `2770985` y `63b9bf3`. Contrasté 36 archivos de `core`, `slm` y configuración con los objetos Git, normalizando CRLF: sin diferencias. **No modifiqué archivos.**

`cargo test -p merken-core -p merken-slm` fue rechazado al crear el proceso (`Access is denied`). Tampoco pude ejecutar Clippy ni formato. Los resultados siguientes proceden de inspección y recorrido estático; no son resultados ejecutados de Rust.

En las referencias, `core/` significa `crates/core/src/` y `slm/`, `crates/slm/src/`. Agrupo hallazgos repetidos entre las tablas de la revisión anterior.

## Estado de hallazgos anteriores

| Hallazgo anterior | Estado | Evidencia y repetición de ejemplos |
|---|---|---|
| **P0 — Diagnósticos, tratamientos, dosis y pérdida de modalidad/prohibiciones** | **Cerrado** | `slm/explain/selection.rs:482`, `:512`, `:518`. `You have a clot.` y `Take aspirin [safety].` ya no son respuestas admisibles. Seleccionar `safety/no-aspirin` devuelve literalmente **`Do not take aspirin.`** |
| **P0 — Coherencia de estado; P1 — inversión de persistencia, fuente y acciones** | **Cerrado bajo el diseño autorizado** | `selection.rs:249`, `:374`; `explain.rs:127`. Una frase exclusiva de `Alarm` no se ofrece en `Stable`. El modelo no puede escribir `Your directional shift is persistent.` ni `Medical advice is recommended.`: solo seleccionar frases previamente autorizadas. La corrección semántica de esas frases pertenece a la curación. |
| **P0 — Cifras como bolsa y cantidades mediante sufijos** | **Cerrado** | `selection.rs:518`. `doubled`, `doubling`, `triples` y `quarterly` ya no pueden componerse desde palabras de otras fichas. |
| **P0 — Porcentajes y variante `percents`** | **Cerrado** | `selection.rs:512`, `:518`. No hay expansión morfológica ni prosa generada visible. |
| **P0 — Citas sin respaldo; P1 — cambio de sujeto mediante sufijos** | **Cerrado** | `selection.rs:386`, `:518`. `Take aspirin [hr-rest-1]` y la transformación de `Lower cabin lighting…` en `Your heart rate is lowered…` desaparecen como posibilidades del modelo. |
| **P0 — Aprobación inventada** | **Cerrado para generación** | `selection.rs:518`. El modelo no puede construir `Aspirin is approved [safety].` a partir de una contraindicación. Una frase curada incorrecta seguiría siendo responsabilidad del catálogo. |
| **P1 — Atribución del primer número; atribuciones ajenas posteriores** | **Cerrado en la capa 1** | `core/interpret/extract.rs:926`, `:1059`. `tras 6 ejercicios pulso 72` conserva 72; `pulso desconocido tras 6 flexiones` y `pulso desconocido tras 6` dejan `bpm=None`. `check` todavía permite recuperar algunas atribuciones rechazadas: N1. |
| **P1 — Unidades incompatibles o desconocidas** | **Parcial** | `core/interpret/lexicon.rs:613`, `:725`, `:1060`. `20 minutos`, `98.6 F`, `5,5 mmol/L`, `98.6 degF` y `98.6 ℉` quedan sin valor compatible. Una palabra desconocida separada, como `fahrenheits`, todavía puede tratarse como ausencia de unidad: N4. |
| **P1 — Hora completa o incompleta convertida en puntuación** | **Cerrado para los ejemplos** | `core/interpret/time.rs:162`. `a las 08:05` y `a las 08:` se consumen como tiempo; el segundo queda ambiguo, sin `score=8`. |
| **P1 — Fragmentos numéricos y signos** | **Cerrado** | `core/interpret/numbers.rs:465`, `:481`, `:502`. `trescientos setenta y dos` y `ánimo -6` siguen rechazándose íntegramente. |
| **P1 — Reutilización de denominadores** | **Cerrado** | `numbers.rs:531`, `:567`. `ánimo 6/7` no reutiliza 7; `6/10` sigue admitido. |
| **P1 — Negación global y excepción global de sueño** | **Cerrado para los ejemplos** | `extract.rs:1890`, `:2300`. `pulso 72 sin dolor` conserva el pulso; `dormí bien no tuve pesadillas` y `dormí bien sin interrupciones` conservan `good`; `no dormí bien` conserva `poor`. Hay una regresión distinta en `check`: N2. |
| **P1 — Negaciones desalineadas y contracciones inglesas** | **Cerrado para los ejemplos** | `extract.rs:1710`, `:1899`. `no tengo fatiga 6`, `no medí pulso 72` e `I can't measure pulse 72` van a negadas. |
| **P1 — Correcciones y repeticiones colapsadas** | **Cerrado en la capa 1** | `extract.rs:2173`, `:2222`. `pulso 72 perdón pulso 82` y `pulso 72, perdón pulso 82` dejan un registro pendiente y conservan el conflicto. `check` permite omitir la corrección de la cita: N1. |
| **P1 — Duración ajena incorporada al sueño** | **Cerrado en la capa 1** | `extract.rs:963`, `:1242`, `:2110`. Ni `ayer dormí mal. hoy trabajé 5 horas` ni `trabajé 5 horas antes de dormir mal` atribuyen esas horas al sueño. La continuación `dormí mal, 5 horas` sigue contemplada. |
| **P1 — Citas numéricas sin respaldo, recortes y ámbito ampliado** | **Parcial** | `core/proposal.rs:203`, `:217`, `:255`, `:455`. Se rechazan 72 recortado de 172 y préstamos entre cláusulas como `dormí 6 horas, pulso 72`. Falta demostrar relación local cuando la capa 1 no aporta un valor: N1. |
| **P1 — Unidad o negación excluida de la cita; zona y descripción ficticias** | **Cerrado para los ejemplos anteriores** | `proposal.rs:270`, `:322`, `:352`, `:426`. Citar solo `98.6` no oculta `F`; citar `pulso 72` no oculta `no medí`; `headache` no oculta `No headache`. Zona y descripción inventadas siguen rechazadas. Subsisten otros límites de contexto: N1 y N2. |
| **P1 — `None`/`null` desactiva la validación restante** | **Cerrado** | `proposal.rs:496`; `slm/interpreter.rs:376`. Cada valor presente se comprueba. `hours=999` no queda habilitado por otros campos pendientes. |
| **P1 — Ventanas temporales inventadas** | **Cerrado** | `proposal.rs:191`. Los extremos presentes siguen rechazándose, aunque citen `anoche`. |
| **P1 — Confinamiento que sigue enlaces** | **Cerrado para enlaces preexistentes** | `slm/manifest.rs:304`, `:314`, `:323`; `slm/store.rs:455`. Se mantienen inspección de componentes, rechazo de enlaces y confinamiento canónico. |
| **P1 — TOCTOU y pruebas verificadas falsificables** | **Parcial** | `store.rs:107`, `:238`; `slot.rs:79`. Los tipos siguen opacos y hay exclusión continua para la misma raíz. Raíces anidadas pueden modificar el mismo archivo mediante locks diferentes: N3. |
| **P1 — `requires_all` no asegura base + LoRA** | **Cerrado estructuralmente** | `manifest.rs:465`, `:483`, `:578`. Se exige el par y se comprueba compatibilidad declarada. Aplicar realmente el adaptador corresponde a Fase 1. |
| **P1 — Cancelación y plazo durante validación** | **Parcial** | `interpreter.rs:303`, `:309`, `:505`; `store.rs:238`, `:333`. El saneado consulta `Control`; las nuevas esperas bloqueantes del almacén no lo consultan: N5. |
| **P1 — Errores con datos sensibles, incluido backend** | **Cerrado en las rutas revisadas** | `slm/error.rs:40`, `:90`, `:130`; `interpreter.rs:452`. `Engine` contiene ahora un enum constante. Los errores propios no reproducen `[I have HIV]`, cifras ni mensajes de Serde. |
| **P1 — Bidi visible en la nota** | **Cerrado** | `selection.rs:75`. Se rechazan bidi, controles y todo carácter no ASCII imprimible al construir/deserializar `Statement`. La salida del modelo no aporta texto visible. |
| **P2 — Entrada SLM ilimitada y saneado sin cooperación** | **Cerrado para el procesamiento descrito** | `interpreter.rs:20`, `:234`, `:303`, `:435`, `:445`. Comparte 1.000 caracteres, limita salida a 16 KiB y acota registros/campos/citas. El saneado sigue repitiendo `check`, pero ahora tiene límites y consultas cooperativas. |
| **P2 — Sueño cero rechazado** | **Cerrado** | `proposal.rs:141`; `core/interpret/tests.rs:763`. `dormí 0 horas` vuelve a admitir `hours=0`. |
| **P2 — Texto clínico escondido en IDs** | **Cerrado** | `selection.rs:44`, `:386`, `:518`. `take two aspirin` no es un ID válido. Los IDs no habilitan vocabulario ni modifican las frases. |
| **P2 — `Control` no era `Send`** | **Cerrado** | `slm/control.rs:166`, `:174`, `:294`. Se conserva callback `Send` y prueba de traslado a un hilo. |

## Hallazgos nuevos

Todos los ejemplos de esta tabla son **resultados de recorrido estático**, pendientes de ejecución Rust.

| Prioridad | Archivo:línea | Problema y ejemplo concreto | Propuesta |
|---|---|---|---|
| **P1 · N1** | `core/proposal.rs:217`, `:230`, `:268`, `:429`; `core/interpret/lexicon.rs:1857` | **La ausencia de un valor determinista deja pasar atribuciones rechazadas, y la cita recortada cambia el contexto validado.** En `trabajé 5 horas antes de dormir mal`, una propuesta con cita de registro completa, `hours=5`, cita `5 horas` y resto pendiente pasa: la capa 1 devuelve `hours=None`, por lo que se omite la coincidencia. También pasa `bpm=6`, cita `6`, para `pulso desconocido tras 6`. En `pulso 72, perdón pulso 82`, citar únicamente `pulso 72` permite recuperar 72. Además, `temperatura 37 °C/kg`, citando solo `37`, permite Celsius: se comprueba `°C` sin comprobar su cola `/kg`. | Validar relación, correcciones y unidad completa sobre la **ocurrencia original**, compartiendo la segmentación de la capa 1. Distinguir valor desconocido de valor explícitamente rechazado o conflictivo. |
| **P1 · N2** | `core/interpret/extract.rs:1946`, `:2256`, `:2498`; `core/proposal.rs:426` | **`check` rechaza una propuesta legítima de `interpret`.** Para **`pulso 72 sin medir glucosa`**, la capa 1 produce pulso 72 y `negated=["sin medir glucosa"]`. Sin embargo, al comprobar la cita `pulso 72`, el análisis posterior encuentra `sin medir` y declara negado también el pulso. Por tanto, **no se cumple** `interpret(text).check(text).is_empty()`. | Hacer que la polaridad posterior respete el siguiente indicador y sus introductores, usando los mismos límites que la extracción. Añadir este caso como regresión explícita. |
| **P1 · N3** | `slm/store.rs:107`, `:113`, `:238`, `:333` | **Dos `ModelStore` autorizados pueden escribir el mismo archivo sin compartir lock.** A: raíz `/models`, manifiesto `v5/base.gguf`. B: raíz `/models/v5`, manifiesto `base.gguf`. A mantiene lectura durante su fábrica; B obtiene escritura en otro lock y puede reemplazar ese mismo archivo antes de que la fábrica lo abra. No requiere escritura externa ni saltarse la API. | Una raíz propietaria única, rechazo de raíces solapadas o sincronización por los archivos afectados. Probar el solapamiento padre/hijo durante la carga. |
| **P1 · N4** | `core/interpret/lexicon.rs:1060`, `:1092`; `core/interpret/extract.rs:1045` | **Unidad desconocida separada todavía puede equivaler a unidad ausente.** `temperatura 98.6 fahrenheits` permite `celsius=98.6`: la palabra no está clasificada, no está pegada y no tiene mezcla de minúsculas/mayúsculas. `check` comparte esa aceptación. | Reconocer variantes relevantes y tratar los sufijos con apariencia de unidad desconocida como pendientes. Conservar la distinción entre una unidad dudosa y prosa posterior legítima. |
| **P1 · N5** | `slm/store.rs:126`, `:238`, `:305`, `:333`; `slm/slot.rs:80` | **La espera del lock puede superar el plazo y admite interbloqueo por reentrada.** Si una fábrica mantiene lectura, una instalación bloqueada en `.write()` no observa cancelación ni deadline hasta adquirir el lock. Si la propia fábrica —o el callback `Loading`— llama a `remove_package` sobre la misma raíz, espera una escritura que su lectura impide liberar. | Adquisición cancelable/acotada y prevención explícita de reentrada incompatible. Las rutas ordinarias ya evitan la doble adquisición interna; falta cubrir callbacks y contención. |
| **P2 · N6** | `slm/interpreter.rs:914`, `:947`, `:250`; `slm/engine.rs:134`, `:143` | **El test de timeout “dentro del saneado” no alcanza el saneado.** Con los tres pasos del mock, los primeros 11 checks preceden a los límites; el siguiente está en `check_raw_proposal_limits` y el check con `n=12` expira al recorrer su primer registro. El test devuelve `Timeout` antes de `sanitize_proposal`. | Activar el reloj mediante un punto observable dentro del saneado y comprobar que hubo una reparación/iteración antes del timeout. |

## Qué está bien

**La selección de frases cierra la vía de generación clínica anterior.** En la ruta `SlmExplainer::explain`, el modelo solo determina IDs y orden; Rust copia el texto curado exacto. La garantía depende de que las fichas sean contenido confiable:

- **Aplicabilidad:** Rust filtra por `Status` antes de construir prompt y gramática (`selection.rs:374`). `applies_to=[]` nunca ofrece la frase. No se filtra adicionalmente por indicador, fuente o persistencia: el catálogo debe formular y autorizar las frases de acuerdo con ese alcance.
- **IDs y colisiones:** cada componente admite `[a-z0-9-]{1,40}`; `/` queda reservado para unirlos. No hay colisión entre pares distintos. Se rechazan fichas repetidas y frases repetidas dentro de una ficha (`:283`, `:337`).
- **Gramática:** hay escape sucesivo de literal JSON y terminal GBNF (`:395`, `:418`). La variante vacía permite exclusivamente `{"select":[]}`; la no vacía admite entre cero y tres IDs. La gramática permite repetir un ID, pero el validador lo rechaza.
- **JSON:** límite de 4 KiB, estructura y tipos comprobados, rechazo de campos extra, IDs desconocidos, más de tres elementos y selecciones repetidas (`:478`). Una salvedad menor: al analizar como `serde_json::Value`, una clave `select` repetida queda sobrescrita; eso no permite introducir texto ajeno al catálogo.
- **Texto y orden:** máximo 500 caracteres, ASCII imprimible y alguna letra; sin bidi ni controles. Esto valida formato, no idioma ni corrección médica. Se preserva el orden elegido. Dos IDs distintos con texto idéntico pueden mostrar la misma frase dos veces: no hay deduplicación por contenido.

**Cero frases y ausencia de modelo tienen comportamientos distintos.** Una selección `[]` devuelve `note=None`, sin descarte. Si no hay frases ofrecidas, todavía se llama al motor con la gramática vacía (`explain.rs:128`). Un fallo del motor se propaga mediante `Err`; `explain` no devuelve entonces el objeto con hechos. `render_facts` sí puede utilizarse sin modelo. El fallback de interfaz y formularios deberá integrarse en Fase 1.

**El lock sí tiene efecto para una misma raíz canónica.** Las instalaciones, renombres, creación de directorios, borrado y limpieza temporal propios quedan bajo escritura; verificación y carga mantienen lectura hasta que termina la fábrica. Las rutas internas `*_unlocked` evitan adquirir de nuevo el mismo lock. El envenenamiento se convierte en `EngineErrorCode::LockPoisoned`, sin `unwrap` ni pánico propio. Esto no resuelve N3 ni N5.

**Los límites y la privacidad mejoraron.** No encontré nuevas llamadas prohibidas ni logs clínicos en producción. Ambos crates heredan los lints del workspace; los permisos para `unwrap`, índices y pánico permanecen en tests. `EngineErrorCode` elimina el mensaje libre del backend en esa variante.

**Los tests añaden comprobaciones útiles**, especialmente resultados concretos y `assert_valid`, que ejecuta `check`. Sus límites son relevantes:

- Las propiedades aleatorias existentes no demuestran el contrato universal; N2 proporciona un contraejemplo dirigido.
- El test concurrente cubre dos almacenes con la misma raíz. Su espera de 80 ms no asegura que el escritor haya intentado adquirir el lock antes de liberar la fábrica.
- `grammar_accepts` (`slm/explain/tests.rs:184`) es un reconocedor manual que impone por su cuenta parte del esquema y el máximo de tres. Comprueba ejemplos y escapes, pero **no valida la gramática completa con un parser GBNF real**.
- No tengo evidencia ejecutada de tests, Clippy o formato en este entorno.

## Cambios mínimos para aceptar

1. **Cerrar N1:** comprobar atribución, corrección y unidad completa en la ocurrencia original, sin convertir automáticamente un `None` determinista en permiso.
2. **Cerrar N2:** compartir límites de negación y demostrar con casos dirigidos que las propuestas legítimas de la capa 1 pasan `check`.
3. **Cerrar N3 y N5:** impedir escrituras solapadas con locks distintos y acotar la espera, incluyendo reentrada desde callbacks.
4. **Cerrar N4** conservando pendientes las unidades dudosas.
5. Corregir N6, añadir las regresiones anteriores y ejecutar tests de ambos crates, Clippy y formato en un entorno que permita procesos.

## Qué queda explícitamente para la Fase 1

- Motor real, carga efectiva de GGUF y aplicación verificable del LoRA.
- Compilación y aceptación de las gramáticas por el parser/backend real.
- Integración Tauri/Android, almacenamiento privado, instalación y actualización de modelos.
- Fallback visible sin modelo, formularios y presentación segura del texto literal.
- Cancelación del backend, latencias, RAM, comportamiento térmico y pruebas en dispositivo.

**No considero la ausencia de motor real ni de evidencia Android un defecto de esta Fase 0.** Los bloqueos señalados están en sus contratos y adaptadores actuales.