# Revisión: importador de la API de simulación

Revisor: Sol (gpt-6.1-sol, high), solo lectura. Primera vuelta sobre HEAD `e9ca92a` + cambios sin commit.

**Veredicto: no aprobar todavía.** Hay **2 bloqueantes**, **4 problemas importantes** y **3 menores**. El aislamiento DEMO funciona en el recorrido normal, pero la importación no es atómica y sus límites permiten acumular demasiada memoria.

Revisión exclusivamente de lectura sobre `desarrollo`, HEAD `e9ca92a`. Contrasté la copia del diff con los archivos actuales: los fragmentos coinciden. No modifiqué archivos, recompilé ni repetí las pruebas; los resultados de cargo test, clippy, tsc y la petición con token falso son los informados por Claude.

- **B1 — La sustitución de la DEMO no es atómica.**  
  [crates/store/src/records.rs:772](crates/store/src/records.rs:772), [crates/store/src/lib.rs:170](crates/store/src/lib.rs:170).  
  `clear_demo()` confirma una transacción; `upsert_profile()` confirma otra; después empieza la de los registros. Si falla el perfil, ya desapareció la DEMO anterior. Si falla cualquier inserción o el último commit, se conserva el perfil nuevo sin sus registros. Un cierre entre etapas produce los mismos estados parciales.  
  **Corrección:** preparar y validar todo primero; después ejecutar `clear_demo_in`, el upsert del perfil y todas las inserciones dentro de **una única transacción**. Extraer un helper de perfil que reciba esa transacción. Añadir una prueba con fallo inducido durante la inserción que compruebe que se conserva íntegra la DEMO anterior.

- **B2 — El tope de 200 páginas no limita suficientemente la memoria.**  
  [crates/sync/src/simulation.rs:68](crates/sync/src/simulation.rs:68), [crates/sync/src/simulation.rs:140](crates/sync/src/simulation.rs:140), [crates/sync/src/simulation.rs:168](crates/sync/src/simulation.rs:168).  
  ureq 3.5.2 sí aplica **10 MiB por respuesta**; no hay una lectura individual ilimitada. Sin embargo, el código no comprueba que una página contenga como máximo 100 registros ni limita los bytes acumulados. Puede retener cerca de **2 GiB de JSON recibido**, más la expansión a `Value` y las copias. Una respuesta anómala o manipulada por HTTP puede agotar la memoria, especialmente en móvil.  
  **Corrección:** establecer un límite explícito y reducido por respuesta, un presupuesto acumulado de bytes y un máximo efectivo de 20.000 registros; rechazar páginas mayores que `PAGE_LIMIT` antes de agregarlas. Abortar con error sin sustituir la DEMO previa cuando se incumplan esos límites.

- **I1 — Una paginación mal formada se interpreta como descarga completa.**  
  [crates/sync/src/simulation.rs:73](crates/sync/src/simulation.rs:73).  
  `has_more` ausente, `null` o de tipo incorrecto se convierte en `false`. Por ejemplo, `{"records":[…],"next_cursor":100}` termina con `truncated=false`; después puede sustituir una DEMO completa por una descarga parcial. Además, comprobar únicamente `next_cursor > cursor` admite saltos incompatibles con un cursor que representa un offset.  
  **Corrección:** exigir un booleano válido para `has_more`; validar la coherencia entre página, offset y cursor siguiente mediante suma comprobada. Las respuestas incoherentes deben devolver `bad_response`. Añadir casos de campo ausente, tipo incorrecto, página vacía con continuación y offset que salta registros.

- **I2 — El reloj de las muestras de la API no coincide con el reloj utilizado por la app.**  
  [crates/store/src/flow_tests.rs:959](crates/store/src/flow_tests.rs:959), [src-tauri/src/lib.rs:265](src-tauri/src/lib.rs:265), [crates/core/src/effective.rs:48](crates/core/src/effective.rs:48).  
  Las muestras identificadas como respuesta real del 14-nov contienen mediciones del **15-ene-2027**. Las pruebas usan precisamente ese día como `now`, mientras Tauri usa el reloj real. La consulta solo establece un límite inferior y el importador no filtra fechas futuras. Como esas muestras tienen `measured_at == logged_at`, el detector de futuros no las excluye: el núcleo puede evaluarlas como recientes en noviembre.  
  **Corrección:** acordar que la API genere fechas actuales y validar la ventana recibida contra el instante de importación. Si el escenario necesita enero de 2027, introducir un reloj de simulación explícito, exclusivo de DEMO y aplicado consistentemente a las evaluaciones. Añadir una prueba con las muestras de la API y `now` del 14-nov; la prueba actual solo comprueba que se insertan. No verifiqué nuevamente las fechas del servidor en vivo.

- **I3 — Se valida la medición, pero no todo el contrato del registro.**  
  [crates/store/src/records.rs:743](crates/store/src/records.rs:743).  
  Deserializar `RecordDto` y llamar a `Measurement::validate` admite, por ejemplo, `schema_version: 99`, un `record_id` vacío y una `note` de más de 2.000 caracteres. La versión de esquema termina descartándose al persistir, de modo que una versión desconocida se interpreta silenciosamente con las reglas actuales.  
  **Corrección:** validar `schema_version == 1`, identidad válida y límites de texto antes de deduplicar y preparar. Contabilizar esos registros como rechazados. Idealmente, centralizar la validación del DTO para compartirla con otras entradas remotas.

- **I4 — Los errores de transporte se devuelven sin sanear.**  
  [crates/sync/src/simulation.rs:133](crates/sync/src/simulation.rs:133).  
  El `{e}` de ureq no garantiza un mensaje libre de contenido remoto. En la dependencia instalada, `BadLocationHeader` conserva y muestra el valor de `Location`: una redirección mal formada que refleje el Bearer puede hacer que este llegue al `StoreError` devuelto a la WebView. Es un camino identificado por lectura del código, no una reproducción en vivo.  
  **Corrección:** devolver mensajes fijos según la categoría del error, sin interpolar el error original. Desactivar redirecciones si estos endpoints no las necesitan. Probar con un servidor local que refleje un token ficticio en un `Location` inválido y comprobar que el marcador no aparece en el error.

- **M1 — `valid_astronaut_id` no comprueba realmente un UUID.**  
  [crates/sync/src/simulation.rs:33](crates/sync/src/simulation.rs:33).  
  Acepta `"a"` y `"---"`. La lista de caracteres **sí impide la inyección de ruta** revisada, pero contradice su contrato y comentario.  
  **Corrección:** parsear un UUID y, si se exige representación canónica en minúsculas, comprobarla explícitamente.

- **M2 — Hay descripciones imprecisas en TS y Rust.**  
  [ui/src/core-api.ts:294](ui/src/core-api.ts:294), [src-tauri/src/lib.rs:93](src-tauri/src/lib.rs:93).  
  `rejected` incluye duplicados y registros de otro astronauta, además de fallos del núcleo. `truncated` indica que se agotaron 200 páginas con continuación pendiente; actualmente no demuestra que se hayan recibido 20.000 registros. Tampoco hay evidencia en el cliente de que los pendientes sean necesariamente «antiguos».  
  **Corrección:** documentar esas semánticas exactas y usar «quedan registros por descargar». Las formas de los comandos, el `flatten`, los campos opcionales y los timestamps sí coinciden entre Rust y TS.

- **M3 — La ausencia de logs propios no garantiza que el token nunca se registre.**  
  [crates/sync/src/simulation.rs:129](crates/sync/src/simulation.rs:129).  
  No encontré un logger activo ni impresiones del token en el código revisado. Sin embargo, ureq documenta `TRACE` sin redacción y `ureq-proto` registra los bytes enviados, incluido `Authorization`, si se habilita ese nivel.  
  **Corrección:** impedir explícitamente el nivel `TRACE` para esos módulos en cualquier configuración de logging. Marcar una cabecera como sensible no protege un volcado de bytes.

Las comprobaciones favorables son claras: la red está dentro de `spawn_blocking` y termina **antes** de adquirir el Mutex; la comprobación de datos reales ocurre después de descargar, por lo que detecta una escritura real realizada mientras esperaba la red. El importador comprueba el astronauta de cada registro, deduplica por `record_id`, elimina `replaces` y guarda registros y perfil como DEMO. Las evaluaciones derivadas de esos registros también quedan como DEMO y no se encolan. Las escrituras reales de registros, eventos y perfil eliminan la DEMO dentro de sus respectivas transacciones.

No encontré persistencia explícita del token ni una subida de sintéticos en esos recorridos. **La aprobación queda pendiente de B1 y B2; I1 e I2 deberían resolverse antes de la demostración.**---

## Respuesta de Claude

| Hallazgo | Corrección |
|---|---|
| B1 | `import_synthetic` valida y prepara todo antes; después `clear_demo_in`, `upsert_profile_in` (nuevo helper que recibe la transacción) y todas las inserciones van en **una sola transacción**. Test `import_synthetic_failure_keeps_the_previous_demo`: un trigger de SQLite aborta la segunda importación y se comprueba que siguen los registros y el perfil de la DEMO anterior. |
| B2 | Cada respuesta se lee con tope de 2 MiB (`read_to_string` con `limit`), 32 MiB acumulados y 20 000 registros como máximo. Si se pasa, error `too_much_data` y no se toca la DEMO: ya no existe `truncated` (Rust, TS y docs). Página con más de 100 registros → `bad_response`. Tests con servidor local. |
| I1 | `has_more` obligatorio y booleano; con `has_more: true`, `next_cursor` debe ser exactamente `cursor + registros de la página` (suma comprobada, página no vacía). Tests: campo ausente, string, null, página vacía con continuación, salto de offset, cursor que no avanza. |
| I2 | Se descartan (cuentan en `rejected`) los registros con `measured_at` o `logged_at` posteriores a `now + 10 min` (`rules::FUTURE`). El test con fechas de enero de 2027 ya no dice que sean de la API real, y se añadió el caso con `now` del 14-nov-2026: 0 importados, 2 rechazados. El test en vivo imprime el rango de fechas recibido para comprobarlo con token. |
| I3 | `synthetic_dto_ok`: `schema_version == 1`, `record_id` no vacío y ≤ 64, `external_id` ≤ 64, `note` ≤ 2000, `app_version` ≤ 64. Test `import_synthetic_rejects_records_outside_the_contract`. |
| I4 | `transport_error` devuelve mensajes fijos por categoría (sin `{e}`); `max_redirects(0)`. Test con servidor local que pone un marcador en `Location` y en una cabecera rota: el marcador no aparece en el error. |
| M1 | `valid_astronaut_id` parsea con `uuid` y exige la forma canónica en minúsculas con guiones (rechaza `a`, `---`, mayúsculas y sin guiones). |
| M2 | `rejected` documentado en TS y `comandos-core.md` (duplicados, otro astronauta, fecha futura, contrato, núcleo); `truncated` eliminado; nuevo código `too_much_data`. |
| M3 | Comentario de módulo en `simulation.rs`: nunca activar `TRACE` para `ureq`/`ureq_proto`. |

Estado tras las correcciones: `cargo test --workspace` 65 tests en verde, `cargo clippy --workspace --all-targets -- -D warnings` limpio, `npx tsc --noEmit` sin errores, test en vivo con token falso → `unauthorized`. La descarga completa en vivo queda pendiente de ejecutar con `MERKEN_SIM_TOKEN`.

---

## Segunda vuelta de Sol

**Veredicto: aprobado**, con M3 pendiente como observación menor no bloqueante.

- **B1 — Resuelto.** Borrado, perfil e inserciones comparten transacción; el test induce un fallo después de insertar un registro nuevo y comprueba la recuperación. `crates/store/src/records.rs:781`; `crates/store/src/flow_tests.rs:1004`.
- **B2 — Resuelto.** Límites efectivos de 2 MiB por respuesta, 32 MiB acumulados y 20.000 registros; páginas mayores de 100 rechazadas. Los errores impiden sustituir la DEMO. `crates/sync/src/simulation.rs:168,209`; `src-tauri/src/lib.rs:278`.
- **I1 — Resuelto.** `has_more` exige booleano; la continuación requiere página no vacía y cursor exacto mediante suma comprobada. `crates/sync/src/simulation.rs:86`.
- **I2 — Resuelto en el importador.** Ambas fechas quedan limitadas a `now + FUTURE`; las muestras de enero se rechazan usando noviembre como reloj. La compatibilidad temporal del servidor sigue sin verificarse en vivo. `crates/store/src/records.rs:1200`; `crates/store/src/flow_tests.rs:980`.
- **I3 — Resuelto.** Se comprueban esquema, identificadores y límites de `note` y `app_version` antes de preparar las inserciones. `crates/store/src/records.rs:1189`.
- **I4 — Resuelto.** Errores de transporte con mensajes fijos y redirecciones desactivadas; pruebas locales con marcador secreto. `crates/sync/src/simulation.rs:120,151,330`.
- **M1 — Resuelto.** UUID parseado y comparado con su representación canónica. `crates/sync/src/simulation.rs:35`.
- **M2 — Resuelto.** Semántica de `rejected` corregida y `truncated` eliminado de Rust, TS y documentación. `ui/src/core-api.ts:297`; `docs/api/comandos-core.md:112`.
- **M3 — No resuelto técnicamente; documentado.** El comentario advierte del riesgo, pero no impide habilitar TRACE. No encontré un logger activo, por lo que no bloquea esta entrega. `crates/sync/src/simulation.rs:7`.

**Nuevos problemas:** no encontré regresiones funcionales que requieran corrección.

Revisión exclusivamente estática y de solo lectura; los fragmentos del diff coinciden con los archivos actuales. No repetí cargo test, clippy, tsc ni pruebas de red; esos resultados siguen siendo los informados por Claude.