## 1. Dictamen

**Dejaría la etapa 1 abierta.** El guardado normal es atómico. `BEGIN IMMEDIATE` precede a la consulta de la clave, la huella versionada incluye el destino y los datos de la intención, y el registro, la salida y el recibo comparten transacción. Las pruebas contemplan dos escritores, respuesta perdida, reapertura y conflicto de huella. [pulse.rs:119](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse.rs:119) · [pulse.rs:221](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse.rs:221)

La migración v6 conserva los datos de la fixture v5 y comprueba la versión futura antes de activar WAL. Además, forcé un fallo en el último `INSERT` de la migración en memoria. El rollback conservó la versión 5 y el valor anterior, y eliminó la tabla recién creada. [lib.rs:87](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/lib.rs:87) · [pulse_tests.rs:205](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse_tests.rs:205)

### Hallazgos priorizados

1. **Crítico · Una acción pendiente puede guardarse para otra persona.** `pulse_context` representa con el mismo destino local la ausencia de perfil y cualquier perfil real sin `astronaut_id`. `upsert_profile_in` admite esos perfiles. Dos personas distintas producen el mismo `(store_id, None)` y la confirmación acepta el cambio. Verifiqué los tres casos con la consulta real. **Arreglo:** distinguir «sin perfil» de «perfil existente» y exigir una identidad estable para este último, incluida una transición definida para perfiles v5 sin ID. Probar que cambiar de persona devuelve `action_expired` con cero escrituras. [pulse.rs:318](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse.rs:318) · [lib.rs:411](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/lib.rs:411)

2. **Importante · Recuperar un recibo depende del reloj.** La fachada ejecuta `app.now()?` antes de consultar la clave. Tras un commit con respuesta perdida, un reloj que devuelve error o una fecha fuera de rango impide recuperar el recibo y también oculta un conflicto de huella. **Arreglo:** leer el reloj una sola vez, después de descartar el replay dentro de la transacción. Probar la reapertura con un reloj que falla y exigir el recibo original. [records.rs:85](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/api/src/records.rs:85)

3. **Importante · El recibo y el historial pueden discrepar sobre la raíz.** `pulse_replacement` confía en `records.root_id`, que puede quedar desactualizado al recibir correcciones fuera de orden. Reproduje la llegada de A, luego C que reemplaza a B, y finalmente B que reemplaza a A. Al corregir C con D, la ruta del recibo toma **B** y el historial devuelve **A** para el mismo D. **Arreglo:** resolver la raíz con la misma regla en preparación, confirmación e historial. Añadir esa secuencia y comparar los `root_id`, además de los IDs ganadores. [pulse.rs:349](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse.rs:349) · [records.rs:991](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/records.rs:991)

4. **Importante · La confirmación confía en un estado deserializado incoherente.** Si la acción recuperada contiene los avisos y el destino vigentes, pero su estado es `NotRequired`, la comprobación permite guardar incluso cuando corresponde confirmar la sustitución de DEMO. **Arreglo:** derivar la necesidad de aprobación en Rust a partir de los avisos y la consecuencia actuales. Para esos casos exigir `Approved` explícito. Probar que una acción persistida incoherente devuelve `NeedsConfirmation` y conserva la DEMO. [pulse.rs:241](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse.rs:241)

5. **Importante · El límite del historial llega después de materializar los candidatos.** Con 2.004 filas válidas y `limit=1`, el SQL devolvió un punto y llamó 2.013 veces al validador instrumentado. **Arreglo:** recorrer candidatos por el índice en orden descendente, resolver cada raíz contra toda su cadena y detenerse al completar el límite de ganadores. Conservar las pruebas de correcciones fuera del intervalo. [pulse_history.rs:11](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse_history.rs:11)

6. **Menor · La acción durable necesita una conversión explícita en E2.** Las firmas, los errores con código/campo y los milisegundos UTC concuerdan con §3. `Box<dyn Fn + Send>` y `&mut App` encajan en el worker propietario. La dificultad concreta es que los getters prestados de `PreparedPulse` omiten la raíz preparada y el estado exacto de aprobación. **Arreglo:** definir una conversión Rust versionada de ida y vuelta para el DTO propio del puente y probar la recuperación tras reiniciar. [lib.rs:24](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/api/src/lib.rs:24) · [pulse.rs:62](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/pulse.rs:62)

## 2. Archivos tocados

Ninguno. Entrego el informe en el chat por la instrucción de solo lectura.

## 3. Comandos y resultados

- `git diff` sobre los archivos indicados de `store` y lectura directa de los nuevos sin seguimiento. Revisé también el código de producción nuevo de `api` y pulso. Encontré cero `unwrap()`, `expect()`, `panic!` o bloques `unsafe`. Las unidades son explícitas y los comentarios nuevos son escasos.
- `target\debug\deps\pulsys_store-c38963de354cf160.exe pulse_tests --skip future_schema --skip schema_v5` → **6 aprobadas, 0 fallidas**, usando el binario existente y bases en memoria.
- `node:sqlite` sobre `:memory:` → comprobaciones independientes de rollback, identidad de perfil, divergencia de raíz y trabajo del historial.

**NO EJECUTADO** Cargo, Clippy ni la suite sobre archivos temporales en esta revisión de solo lectura. Los resultados de compilación y de la suite completa son los comunicados por Claude.

## 4. Pendiente

Los casos nominales de §6 tienen pruebas. Antes de cerrar E1 priorizaría las regresiones de identidad, recuperación con reloj fallido, raíz fuera de orden y aprobación restaurada. Para zonas falta también un caso positivo con síntoma, zona y evidencia que compruebe la proyección completa del DTO. [pulse_flow.rs:191](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/api/tests/pulse_flow.rs:191)