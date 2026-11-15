**Segunda revisión: NO se puede entregar todavía.** Encontré **2 bloqueos, 2 problemas importantes y 2 menores nuevos**, además de pendientes de la primera ronda.

Revisión en solo lectura: no edité archivos ni bases existentes. Las comprobaciones ejecutadas usaron SQLite y TypeScript en memoria, con dependencias simuladas. **No ejecuté la aplicación nativa ni acredité su presentación visual.**

**Estado de los ocho hallazgos originales**

| # | Estado | Evidencia y pendiente |
|---|---|---|
| **1. Panel sin integrar — B** | **RESUELTO** | Está importado, montado y conectado a tripulación, navegación y limpieza en [main.ts:49](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/main.ts:49). El contenedor está dentro de `#core-panel` en [index.html:70](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/index.html:70). Integración estructural comprobada; falta prueba nativa. |
| **2. Sustitución mediante `REPLACE` — I** | **PARCIAL** | El nuevo `BEFORE INSERT` protege `telemetry_readings` incluso con `recursive_triggers=0`: [store/lib.rs:156](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/lib.rs:156). Pero una **v4 anterior no recibe ese trigger**, y las nuevas cabezas dependen de `recursive_triggers=1`. Véanse N2 y N4. |
| **3. Truncamientos y series eliminadas — I** | **PARCIAL** | Las nuevas cabezas permiten detectar `missing_tail`, `missing_series` y diferencias de conteo: [telemetry.rs:420](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:420). Funciona sobre el esquema nuevo, pero no existe actualización desde la v4 anterior. La protección de las cabezas también tiene la limitación N4. |
| **4. IPC lento bloquea el feed — I** | **PARCIAL** | El HTTP ya publica y libera `inFlight` antes de persistir: [telemetry.ts:266](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry.ts:266). Se resuelve el bloqueo visual, pero el timeout introduce solapamientos IPC reales: N3. |
| **5. Identidades recortadas inconsistentemente — I** | **RESUELTO** | `parseCrew` normaliza y después comprueba duplicados; la conversión reutiliza la identidad: [telemetry.ts:72](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry.ts:72), [telemetry.ts:95](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry.ts:95). `latestCrew` conserva esos mismos ids. |
| **6. Auditorías completas y trabajo obsoleto — I** | **PARCIAL** | El refresco habitual ya no verifica todo; hay debounce, caché y exclusión de verificaciones: [telemetry-history.ts:404](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:404). **Los listados sí pueden acumularse**: cada `refresh()` inicia otro IPC y solo descarta respuestas antiguas. La auditoría manual sigue recorriendo todo bajo el mutex compartido. |
| **7. Pruebas insuficientes — M** | **PARCIAL** | Ahora existe rollback después de una inserción y pruebas UI en el repositorio. El test Tauri sigue llamando a una función auxiliar, sin pasar por el despachador IPC: [src-tauri/lib.rs:443](/C:/Users/Salva/Desktop/git/nasa.spaceapps/src-tauri/src/lib.rs:443). Faltan pruebas cruzadas UI→Rust y actualización desde la v4 anterior. |
| **8. `aria-busy` persistente — M** | **RESUELTO** | `clearLoadingBusy()` cubre tripulación vacía, fuera de Tauri, error, respuesta obsoleta y destrucción: [telemetry-history.ts:380](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:380). Hay una regresión reproducible en el repositorio. |

**Problemas nuevos**

**N1. B — La nueva validación Rust rechaza los payloads reales de radiación y distancia**

- **Ubicación:** [telemetry.ts:44](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry.ts:44), [telemetry.rs:148](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:148), [core-api.ts:318](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/core-api.ts:318).
- **Escenario:** la UI produce `{ usv_h: 14 }` y `{ km: 384400 }`; Rust exige exactamente `{ value: número }`. Rust valida **todo el lote antes de escribir**, por lo que cualquiera de esos indicadores rechaza también las lecturas válidas del mismo bloque.
- **Demostración:** ejecuté la conversión TypeScript en memoria y obtuve ambos formatos incompatibles. Además, las propias pruebas Rust exigen rechazarlos en [telemetry.rs:1195](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:1195), mientras las UI exigen producirlos en [telemetry.test.mjs:293](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/tests/telemetry.test.mjs:293).
- **Impacto adicional:** el error queda en `state.persist`, pero la integración actual no lo muestra al usuario.
- **Corrección:** acordar un único formato entre conversión, tipos, Rust, documentación y fixtures. Añadir una prueba que entregue a Rust el lote realmente generado por la UI. Un tipo discriminado por indicador ayudaría a prevenir otra divergencia.

**N2. B — Una base de la primera ronda permanece en v4 con un esquema incompatible**

- **Ubicación:** [store/lib.rs:85](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/lib.rs:85), [store/lib.rs:130](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/lib.rs:130).
- **Escenario:** abrir una base creada por la primera ronda, con `user_version=4`. La inicialización la acepta, pero omite completamente el bloque que ahora incorpora `telemetry_heads`, el índice nuevo y el trigger antirreemplazo.
- **Consecuencia:** listar las lecturas antiguas puede funcionar; registrar o verificar falla con `storage_error` porque falta `telemetry_heads`. No se corrompe necesariamente el archivo: **queda incompatible con el código actual**.
- **Demostración:** reproduje en SQLite en memoria el esquema anterior y la condición de migración. La consulta de cabezas falla con `no such table: telemetry_heads`.
- **Corrección:** crear una migración **v5**, compatible tanto con la v4 anterior como con la v4 ampliada. Construir las cabezas desde las lecturas existentes sin reescribir sus hashes ni su JSON, y comprobar previamente la coherencia de sus cadenas. Ese checkpoint inicial no puede acreditar retrospectivamente datos que ya faltasen.

**N3. I — El timeout libera la cola sin cancelar el guardado anterior**

- **Ubicación:** [telemetry.ts:169](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry.ts:169), [telemetry.ts:219](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry.ts:219).
- **Escenario:** un `recordTelemetry` tarda más de diez segundos y existe un lote pendiente. `Promise.race` rechaza la espera; la cola continúa y lanza otro `invoke`, aunque el anterior sigue vivo.
- **Demostración:** con el código TypeScript real y promesas controladas, las llamadas pendientes pasaron de **1 a 2** después del timeout. El feed siguió publicando correctamente.
- **Impacto:** la cola ya no garantiza una sola llamada IPC pendiente. Ante bloqueo prolongado puede acumular comandos nativos y guardar posteriormente snapshots que la UI dio por fallidos.
- **Corrección:** separar el aviso de demora de la finalización del IPC. Mantener la exclusión hasta que termine la promesa original, o implementar cancelación efectiva y confirmada en el backend. `stop()` tampoco cancela un comando ya enviado; solo detiene la actividad futura del feed.

**N4. I — `telemetry_heads` admite sustituciones con `recursive_triggers=0`**

- **Ubicación:** [store/lib.rs:181](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/lib.rs:181).
- **Escenario:** una conexión SQL con triggers recursivos desactivados ejecuta `REPLACE INTO telemetry_heads` o `INSERT OR REPLACE`. El borrado implícito no ejecuta `telemetry_heads_no_delete`; tampoco interviene el trigger de UPDATE.
- **Demostración con el DDL actual:**

| Tabla | `recursive_triggers=0` | `recursive_triggers=1` |
|---|---|---|
| `telemetry_readings` | Ambos reemplazos bloqueados | Ambos bloqueados |
| `telemetry_heads` | **Ambos aceptados** | Ambos bloqueados |

- **Alcance:** las conexiones abiertas por `Store` activan el valor 1. El bypass demostrado requiere otra configuración/conexión SQL; no es una operación disponible mediante los comandos actuales.
- **Corrección:** proteger también la reutilización de la clave de una cabeza independientemente del PRAGMA. **No basta con añadir un trigger que rechace cualquier INSERT duplicado**, porque bloquearía también el UPSERT legítimo actual. Una alternativa es actualizar explícitamente las cabezas existentes e insertar solo las nuevas, dentro de la misma transacción, y añadir después la protección antirreemplazo.

**N5. M — La UI revierte el nuevo orden por `reading_id`**

- **Ubicación:** [telemetry.rs:628](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:628), [telemetry-history.ts:138](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:138).
- **Escenario:** el backend devuelve ids `[2, 1]`, pero el reloj de la lectura 2 está atrasado respecto de la 1. La UI reordena por `observed_at` y presenta `[1, 2]`.
- **Demostración:** reproducido con la función de ordenación en memoria.
- **Corrección:** conservar `reading_id DESC` para la lista de últimas inserciones. Si la gráfica necesita otro orden temporal, tratarlo explícitamente sin cambiar silenciosamente el orden de la lista.

**N6. M — La etiqueta de integridad puede permanecer obsoleta indefinidamente**

- **Ubicación:** [telemetry-history.ts:419](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:419), [telemetry-history.ts:531](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:531).
- **Escenario:** se verifica una base vacía y después llegan lecturas del mismo astronauta. Los refrescos actualizan la lista, pero nunca revisan la caducidad del resultado mientras no cambie el astronauta.
- **Demostración:** después de avanzar el reloj simulado dos minutos, el panel mostró simultáneamente `1 readings loaded` y `0 readings · chain consistent ✓`, con una sola verificación ejecutada.
- **Corrección:** mostrar fecha y alcance de la última comprobación, y marcarla como anterior a las nuevas lecturas. Puede conservarse la auditoría manual sin volver a verificar todo en cada refresco.

**Integración `main.ts` / `index.html`**

La conexión principal está bien realizada:

- **Navegador:** `isTauri()` hace que `available=false`; no se monta el historial y su contenedor permanece oculto.
- **Preview de desarrollo:** `?preview=1` hace que `available=false` y utiliza `createPreviewFeed`, evitando el feed persistente.
- **Tripulación:** `renderFeed` actualiza `latestCrew` en cada publicación; `getCrew` devuelve los ids ya normalizados por `parseCrew`.
- **Temporizador:** cada cambio de pantalla elimina el intervalo anterior antes de crear otro. Solo existe refresco periódico del historial en la pantalla de índice 1.
- **Visibilidad:** `historyHost.hidden=!available` es coherente con el `hidden` del padre controlado por `setupPages`; `[hidden]` tiene `display:none!important`.
- **Limpieza:** `pagehide` no persistido detiene el intervalo y destruye el componente.

Referencias: [main.ts:25](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/main.ts:25), [main.ts:55](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/main.ts:55), [main.ts:67](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/main.ts:67), [main.ts:78](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/main.ts:78), [dashboard.ts:22](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/dashboard.ts:22).

**Duda de alcance del preview:** en un build de producción, `import.meta.env.DEV` es falso y `?preview=1` se ignora. Por tanto, dentro de Tauri **`available` seguirá siendo verdadero**. El comentario define el preview como exclusivo de desarrollo. Si se exige que la query desactive el núcleo también en producción, esa condición no está cumplida; falta fijar esa expectativa.

El temporizador no se acumula al navegar, pero **las peticiones pueden acumularse**: reproduje tres `listTelemetry` pendientes a la vez mediante montaje y dos refrescos. Para cerrar el hallazgo 6 falta una exclusión con un único refresco pendiente, equivalente a «ejecutar una vez más con la selección más reciente».

**Contrato, cabezas y concurrencia**

`VerifyTelemetryResult` coincide entre Rust y TS: `ok`, `checked`, `series_checked`, `first_bad_reading_id` y `problem`, incluidos sus valores nulos. Los cinco códigos producidos por Rust tienen etiqueta en la UI. `series_checked` está tipado pero no se muestra; no es por sí mismo un incumplimiento.

Referencias: [telemetry.rs:101](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:101), [core-api.ts:379](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/core-api.ts:379), [telemetry-history.ts:18](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:18).

`list_telemetry` sí ordena por `reading_id DESC`. Un indicador desconocido devuelve `invalid_value`, con `field="indicator"`. **La UI maneja el rechazo** mediante su `catch` y muestra el mensaje de `CoreError`; el selector normal contiene únicamente indicadores admitidos. No hay una prueba UI específica para ese error estructurado.

El trigger de cabezas **permite el avance legítimo**: el UPSERT asigna un id mayor e incrementa `count`. Lo comprobé ejecutando inserción y actualización de cabeza en una misma transacción.

Dos comandos actuales `record_telemetry` no acceden simultáneamente al mismo `Store`: [with_store:39](/C:/Users/Salva/Desktop/git/nasa.spaceapps/src-tauri/src/lib.rs:39) mantiene el mutex hasta terminar el commit. El solapamiento N3 acumula invocaciones, pero **no demuestra una bifurcación de hashes**.

Con dos instancias independientes de `Store` sobre el mismo archivo ya no existiría ese mutex común. No hay prueba de ese escenario ni reintento explícito ante conflictos SQLite. Tampoco hay una transacción de lectura que abarque conjuntamente la carga de cabezas y lecturas de `verify_telemetry`; una conexión escritora independiente podría producir una comprobación con snapshots distintos. Lo señalo como límite del escenario multiconexión, no como fallo demostrado de la aplicación actual.

**Regla de 10 s / 60 s e igualdad**

La implementación coincide con la regla documentada:

- Primera lectura: se guarda.
- Cambio de valor o unidad: se guarda desde **10.000 ms**, inclusive.
- Sin cambios: se guarda desde **60.000 ms**, inclusive.
- Cada comparación usa la última lectura **guardada**, no la última recibida.
- Una segunda lectura de la misma serie dentro del mismo lote ve la primera inserción y se omite por intervalo cero.
- Un reloj atrasado produce un intervalo negativo y se omite hasta alcanzar de nuevo el umbral.

La resta usa `i128`, evitando desbordamiento. `73` y `73.0` se consideran iguales y los objetos se comparan estructuralmente. Véase [telemetry.rs:524](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:524).

La igualdad mediante `f64` **no es igualdad exacta para números arbitrariamente grandes**: enteros distintos superiores a su precisión pueden convertirse al mismo número. Está documentada como igualdad por `f64`; no la clasifico como otro defecto sin una exigencia adicional de precisión.

**Qué acreditan las pruebas**

- **Rollback:** el test nuevo sí provoca una segunda inserción fallida por SQL, después de haber insertado una fila y su cabeza. Comprueba su reversión y la conservación de una serie previa: [telemetry.rs:1783](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:1783). Cierra esa carencia concreta.
- **Frecuencia:** cubre 9.999/10.000 ms, 59.999/60.000 ms, igualdad entero/flotante e inserción intralote: [telemetry.rs:1022](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:1022).
- **Integridad:** cubre alteración, enlace roto, truncamiento, desaparición completa y conteo: [telemetry.rs:1575](/C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:1575).
- **REPLACE:** solo se prueba con `recursive_triggers=1`. Falta la matriz con valor 0 para ambas tablas.
- **Migración:** prueba v3→v4 nueva y reapertura de esa misma base; **no prueba actualizar una v4 anterior**.
- **IPC:** el test auxiliar y la serialización son útiles, pero `handle_verify_telemetry(None)` no demuestra que el despachador acepte una invocación sin la clave externa `input`. Tampoco prueba fechas pasando por `invoke`.
- **UI:** las ocho pruebas son reproducibles desde el repositorio, pero sus mocks aceptan los payloads incompatibles de N1. El test del timeout exige volver a invocar mientras la primera promesa nunca termina; acredita recuperación de la espera, no exclusión IPC.
- **Contador vacío:** el test «fuera de Tauri» declara `invokeCount=0` y lo compara con cero sin conectarlo a ninguna invocación: [telemetry.test.mjs:316](/C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/tests/telemetry.test.mjs:316). Esa aserción no acredita ausencia de IPC.
- **Integración:** la suite empaqueta `telemetry.ts` e historial, no `main.ts`; no prueba navegación, intervalos, preview ni visibilidad del contenedor.

Intenté ejecutar los tres comandos autorizados:

| Comando | Resultado de esta revisión |
|---|---|
| `cargo test --workspace --offline` | No pudo iniciarse: `CreateProcessAsUserW`, acceso denegado |
| `cargo clippy --workspace --all-targets --offline -- -D warnings` | Mismo bloqueo |
| `npm run test:ui` | Mismo bloqueo |

Los resultados verdes de los informes anteriores **no fueron revalidados en esta ronda**.

**Veredicto: no entregar.** Antes deben corregirse los payloads incompatibles y la migración desde v4; mantener la exclusión IPC después del timeout; cerrar la protección de cabezas y la acumulación de listados; y añadir pruebas cruzadas que detecten estos escenarios. También deben corregirse el orden y la vigencia de la etiqueta de integridad. Después faltan ejecutar las suites y comprobar el flujo real en Tauri, incluida la navegación y presentación del panel.