# Revisión de Sol — F0 SLM, cuarta ronda

HEAD `5ddddb6` (15-nov-2026).

## Veredicto

**No se acepta todavía la Fase 0: no encontré regresiones P0, pero N1 y N5 siguen parcialmente abiertos y hay una regresión P1 que rechaza propuestas legítimas de capa 2.**

Revisé **HEAD `5ddddb6`**, incluida la revisión anterior. Contrasté los archivos de ambos crates y `Cargo.toml` con los objetos Git: coinciden, normalizando CRLF. **No modifiqué archivos.**

El entorno rechazó la creación de procesos antes de ejecutar Cargo (`Access is denied`/`EPERM`). Los resultados siguientes son de **inspección y recorrido estático**, no de ejecución Rust. En las referencias, `core/` significa `crates/core/src/` y `slm/`, `crates/slm/src/`.

## Estado de N1–N6

| Hallazgo | Estado | Evidencia y repetición de ejemplos |
|---|---|---|
| **N1 — Validar la ocurrencia original, atribución, conflictos y unidad completa** | **Parcial** | `core/proposal.rs:218`, `:224`, `:235`; `core/interpret/extract.rs:2860`; `core/interpret/lexicon.rs:2253`. Los cuatro ejemplos originales quedan rechazados: `hours=5` en **`trabajé 5 horas antes de dormir mal`**; `bpm=6` en **`pulso desconocido tras 6`**; recuperar 72 citando solo **`pulso 72`** de **`pulso 72, perdón pulso 82`**; y `celsius=37` citando `37` de **`temperatura 37 °C/kg`**. Los tests dirigidos están en `core/proposal.rs:1136`. Subsiste el recorte de correcciones para indicadores sin extractor determinista y aparece un rechazo excesivo de capa 2: tabla siguiente. |
| **N2 — Negación posterior e invariancia de `interpret(t).check(t)`** | **Cerrado para el hallazgo** | `core/interpret/extract.rs:2689`, `:2756`. **`pulso 72 sin medir glucosa`** conserva pulso 72; la negación comienza en el segmento de glucosa y no alcanza el pulso. Una propuesta positiva de glucosa citando `glucosa` sigue rechazada. Hay comprobaciones explícitas en `core/proposal.rs:1241` y `core/interpret/tests.rs:771`. No encontré otro contraejemplo de capa 1 entre los casos revisados; el barrido y las propiedades no demuestran universalidad. |
| **N3 — Raíces solapadas con locks distintos** | **Cerrado** | `slm/store.rs:340`, `:350`, `:354`. Una raíz canónica comparte lock; padre e hijo activos se rechazan en ambos órdenes. El ejemplo `/models` con `v5/base.gguf` frente a `/models/v5` con `base.gguf` ya no puede obtener dos almacenes activos. `slm/store.rs:1123` prueba además reapertura tras liberar la raíz. **Se permiten raíces disjuntas**: no hay una única raíz total por proceso. |
| **N4 — Unidades desconocidas separadas** | **Cerrado para el hallazgo** | `core/interpret/lexicon.rs:599`, `:807`, `:841`, `:1455`. **`temperatura 98.6 fahrenheits`**, `37 farenheit` y `37 grados f` quedan pendientes; recortar la cita a `37` o `37 grados` no los habilita. **`pulso 72 ahora`** y **`temperatura 37 grados ahora`** siguen admitidos. Tests en `core/proposal.rs:1270` y `core/interpret/tests.rs:785`. |
| **N5 — Espera cancelable y reentrada** | **Parcial** | `slm/store.rs:209`, `:240`. Instalación y verificación usan `try_*`, consultan `Control` y esperan en pasos de hasta 2 ms. La escritura reentrante sobre **la misma raíz**, desde fábrica o `Loading`, devuelve `LockReentrant`: `:131`, test `:1383`. Sin embargo, `remove_package` conserva una espera no cancelable y permite un interbloqueo entre raíces distintas. |
| **N6 — Timeout realmente dentro del saneado** | **Cerrado** | `slm/interpreter.rs:914`, `:978`, `:983`, `:989`. La prueba directa comprueba que el registro cambió mediante una reparación y que quedan errores cuando vence el plazo en la iteración siguiente. La ruta completa arma el reloj desde `Validating` (`:1017`). El recorrido de checks concuerda con los contadores esperados; ya no vence antes de entrar al saneado. |

## Regresiones o hallazgos nuevos

Todos los contraejemplos siguientes son conclusiones de recorrido estático, pendientes de reproducción ejecutada.

| Prioridad | Archivo:línea | Problema y ejemplo concreto | Propuesta |
|---|---|---|---|
| **P1 · N1 persistente** | `core/proposal.rs:249`; `core/interpret/extract.rs:2970` | **El respaldo para indicadores sin extractor todavía permite esconder correcciones.** Reutilizando exactamente la propuesta válida de radiación de `core/proposal.rs:1220`, `check` acepta `usv_h=0.42`, cita de campo `0.42 uSv/h` y cita de registro `radiación 0.42 uSv/h` contra **`radiación 0.42 uSv/h, perdón radiación 0.84 uSv/h`**. No hay segmentos deterministas de radiación y `check_fallback_field_attribution` busca correcciones únicamente dentro de la cita recortada. | Reconocer menciones y correcciones en la entrada original para todos los indicadores del contrato, aunque no tengan extractor de valores. Añadir el contraste entre radiación válida aislada y ambas ocurrencias del texto corregido. |
| **P1 · Regresión de aceptación de capa 2** | `core/proposal.rs:218`, `:224`; `core/interpret/extract.rs:198`, `:343` | **Un segmento de capa 1 ocupa texto que contiene otro indicador legítimo.** La propuesta anterior de radiación pasa contra **`radiación 0.42 uSv/h`**, pero se rechaza contra **`pulso 72 y radiación 0.42 uSv/h`**, conservando las mismas citas. Radiación no figura entre los introductores utilizados para separar la conjunción ni entre las anclas del extractor. Su campo queda dentro del segmento de pulso y se rechaza por indicador distinto. El saneado termina borrando el valor legítimo. | Separar el reconocimiento de menciones del soporte de extracción. Delimitar todos los indicadores del esquema y comprobar atribución local; evitar que un segmento amplio monopolice cualquier número contenido. Probar ambos órdenes de los indicadores y combinaciones con radiación, distancia y nutrición. |
| **P1 · N5 persistente ampliado** | `slm/store.rs:131`, `:247`, `:481`, `:540` | **Interbloqueo entre dos raíces disjuntas.** T1 entra en la fábrica de A manteniendo lectura y llama a `B.remove_package(...)`; simultáneamente T2 hace lo simétrico desde la fábrica de B. La comprobación de reentrada solo compara el lock solicitado con los retenidos por ese hilo, por lo que ambas llamadas pasan. Las dos quedan en `.write()` esperando que termine la otra fábrica. El plazo exterior no se consulta durante esa espera. La escritura en otro almacén desde una fábrica está expresamente admitida por el test de `slm/store.rs:1412`. | Rechazar adquisiciones cruzadas incompatibles desde callbacks o imponer un orden global verificable. Dar también a la eliminación una adquisición acotada/cancelable. Añadir un test con dos hilos y barrera que reproduzca esta secuencia sin depender de pausas temporales. |

**Comprobación de lo cerrado anteriormente:**

- **Atribución, números y unidades:** los ejemplos `tras 6 ejercicios pulso 72`, `20 minutos`, `98.6 F`, `5,5 mmol/L`, signos, números fragmentarios y denominadores conservan las defensas anteriores. Evidencia: `core/interpret/tests.rs:418`, `:439`, `:510`, `:529`; `core/proposal.rs:904`. Las limitaciones nuevas de capa 2 están delimitadas arriba.
- **Negación, tiempos, correcciones y ventanas:** `no dormí bien` conserva `poor`; `dormí bien sin interrupciones` conserva `good`; las contracciones inglesas siguen negadas. `a las 08:` permanece temporal y no se convierte en puntuación. Las correcciones originales de pulso quedan pendientes y los extremos de ventanas presentes siguen rechazados incondicionalmente (`core/interpret/tests.rs:681`; `core/proposal.rs:275`).
- **Frases autorizadas y P0 del explicador:** selección, explicador y hechos no cambiaron respecto de la revisión anterior. `You have a clot.` y `Take aspirin [safety].` siguen descartados; seleccionar `safety/no-aspirin` copia literalmente **`Do not take aspirin.`**. Se conserva el filtro de estado y no reaparece generación libre de cifras, tratamientos, persistencia, fuentes o acciones (`slm/explain/selection.rs:374`, `:482`, `:518`; tests `slm/explain/tests.rs:220`).
- **Manifiesto y privacidad:** se mantienen confinamiento, rechazo de enlaces, tipos verificados opacos, par base–adaptador y compatibilidad declarada. El cambio de manifiesto/slot solo adapta el paso de `Control` al lock. `LockReentrant` es un código constante; no reaparecen mensajes libres del backend ni fragmentos de Serde (`slm/manifest.rs:307`, `:465`, `:578`; `slm/error.rs:40`, `:133`; `slm/interpreter.rs:452`).

**Lints y tests:** ambos crates siguen heredando los lints del workspace: `unsafe_code=forbid` y prohibiciones Clippy de `unwrap`, `expect`, `panic` e indexación. No encontré nuevas excepciones a esas prohibiciones en producción. Esto es inspección de configuración y código, no un resultado de Clippy.

Intenté ejecutar:

```text
cargo test -p merken-core -p merken-slm
cargo clippy -p merken-core -p merken-slm --all-targets -- -D warnings
cargo fmt --all -- --check
```

Los tres quedaron bloqueados **antes de arrancar**. No certifico compilación, tests, Clippy ni formato.

Las pruebas nuevas sí contienen aserciones relevantes sobre resultados concretos, contención observada, liberación de raíces y reparación antes del timeout. No cubren los tres contraejemplos de la tabla. El reconocedor manual `grammar_accepts` sigue sin equivaler a validar con un parser GBNF real.

## Riesgos residuales aceptables

- **Registro global:** no encontré una fuga acumulativa de propietarios ni un pánico propio nuevo. Usa `Weak`, elimina entradas vencidas al siguiente acceso y libera guardas mediante `Drop` (`slm/store.rs:145`, `:187`, `:340`). Quedan entradas débiles y capacidad de memoria hasta una limpieza posterior; es una retención diferida aceptable. El interbloqueo señalado arriba sigue siendo bloqueante.
- **Cancelación cooperativa:** el sondeo de 2 ms no proporciona tiempo real estricto ni interrumpe una fábrica que deja de cooperar. Plan: verificar cancelación y latencias con el backend real.
- **Cobertura lingüística y catálogo:** el léxico es finito y las frases curadas requieren autorización semántica. Plan: ampliar un corpus ES/EN con positivos, negativos y combinaciones, conservando confirmación por campo.
- **Frontera del sistema operativo:** la protección frente a escrituras externas depende del almacenamiento privado previsto. Plan: verificar esa integración en Android.

## Qué queda para la Fase 1

- Motor real, apertura efectiva de GGUF y aplicación comprobable del LoRA.
- Aceptación de las gramáticas por el backend real.
- Integración Tauri/Android, almacenamiento privado y ciclo de modelos.
- Formularios, confirmación por campo y fallback visible sin modelo.
- Cancelación del backend y mediciones de latencia, RAM y temperatura en dispositivo.

Los **tres P1 de esta revisión deben resolverse en Fase 0**; no dependen del motor real ni de Android.