# Informe de revisión

**Resultado: 1 B, 5 I y 2 M.** La entrega necesita integración del panel y correcciones de integridad y funcionamiento antes de considerarse completa.

Revisión sin editar archivos. El entorno bloqueó `git diff`; comparé los archivos modificados contra los objetos de `HEAD` y leí completos los nuevos. No ejecuté Cargo, Tauri ni los tests declarados por los autores. Sí comprobé los triggers y planes de consulta con el DDL entregado en SQLite **en memoria**, sin tocar bases existentes.

## Hallazgos

### 1. B — El panel de historial no está integrado en la aplicación

**Ubicación:** [telemetry-history.ts:283](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:283), [main.ts:56](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/main.ts:56), [informe UI:19](C:/Users/Salva/Desktop/git/nasa.spaceapps/reviews/telemetria-local-ui-informe.md:19).

**Escenario:** `mountTelemetryHistory` está definido, pero no existe ninguna llamada ni importación desde la aplicación. El usuario puede generar lecturas sin disponer del nuevo historial ni de su verificación.

**Corrección:** montar el componente en una pantalla accesible, proporcionarle la tripulación y conectar su actualización y `destroy()` al ciclo de vida.

El informe reconoce que no conectarlo formaba parte del alcance del autor UI. Este bloqueo corresponde a **la entrega integrada solicitada**, no necesariamente al cumplimiento de esa subtarea aislada.

### 2. I — `INSERT OR REPLACE` permite sustituir filas conservando los triggers

**Ubicación:** [store/lib.rs:152](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/lib.rs:152); configuración de conexión en [store/lib.rs:71](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/lib.rs:71).

**Escenario:** insertar con `INSERT OR REPLACE` un `reading_id` existente sustituye la fila. Con `recursive_triggers=0`, el borrado implícito no ejecuta el trigger de `DELETE`; tampoco es un `UPDATE`.

**Comprobación:** con el DDL entregado, SQLite en memoria rechazó `UPDATE` y `DELETE`, pero aceptó la sustitución sin eliminar ningún trigger. El SQLite incluido por la dependencia usa ese valor predeterminado y la apertura de `Store` no lo cambia.

**Corrección:** impedir mediante un trigger de inserción la reutilización de identificadores existentes; activar también `recursive_triggers` en las conexiones controladas y añadir una regresión para `REPLACE`.

Los comandos actuales usan `INSERT` ordinario: el bypass demostrado requiere acceso SQL.

### 3. I — `verify_telemetry` no detecta truncamientos finales ni series eliminadas completas

**Ubicación:** [telemetry.rs:477](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:477), [telemetry.rs:517](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:517).

**Escenario:** una serie válida contiene A → B → C. Si se permite borrar C, A → B continúa verificando como válida. Si desaparece toda la serie, devuelve `ok: true, checked: 0`. No existe una referencia al último hash o al número esperado de lecturas.

Sí detecta un borrado intermedio o inicial **cuando queda un sucesor** cuyo `prev_hash` delata el hueco.

**Corrección:** contrastar contra un checkpoint confiable del extremo y tamaño de cada serie. Si no se incorpora esa referencia, documentar que se comprueba coherencia interna, no exhaustividad, y evitar presentar «chain verified» como garantía contra cualquier borrado. El test actual solo elimina la primera fila dejando otra después.

### 4. I — Un guardado IPC lento detiene la actualización del feed

**Ubicación:** [telemetry.ts:197](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry.ts:197), especialmente las líneas 209 y 214–215.

**Escenario:** el HTTP termina correctamente, pero `recordTelemetry` tarda o queda pendiente. `refresh()` espera a `persistCrew`, mantiene `inFlight=true` y no llama a `onUpdate`. Los siguientes intervalos de dos segundos retornan sin consultar la API. El temporizador de ocho segundos ya fue cancelado y no limita esa espera IPC.

El comentario «Guardar localmente no debe impedir que el feed refresque el panel» no coincide con este flujo.

**Corrección:** publicar inmediatamente el resultado HTTP y separar la persistencia mediante una cola serial con estado y política explícita de saturación/error. Mantener la exclusión de escrituras sin bloquear la visualización del feed.

### 5. I — El recorte de claves mezcla astronautas y rompe búsquedas

**Ubicación:** [telemetry.ts:93](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry.ts:93); consulta en [telemetry-history.ts:417](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:417).

**Escenario:** `parseCrew` admite los identificadores distintos `"a1"` y `" a1 "`, pues comprueba duplicados antes de normalizarlos. `crewToReadings` guarda ambos como `"a1"`, mezclando sus lecturas en una serie. El historial utiliza el identificador original del selector: consultar `" a1 "` devuelve vacío.

**Corrección:** conservar exactamente la identidad de la API, o normalizarla una sola vez en `parseCrew` y detectar duplicados después de esa normalización. Todos los consumidores deben usar la misma clave.

### 6. I — Cada refresco del historial verifica todo el histórico y permite acumular trabajo obsoleto

**Ubicación:** [telemetry-history.ts:385](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:385), [telemetry-history.ts:415](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:415), [telemetry.rs:438](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:438), [src-tauri/lib.rs:43](C:/Users/Salva/Desktop/git/nasa.spaceapps/src-tauri/src/lib.rs:43).

**Escenario:** cada cambio de indicador o pulsación envía un listado y una verificación completa del astronauta. `requestSeq` descarta resultados antiguos, pero no impide ni cancela ese trabajo. La verificación recorre y calcula hashes para todas las lecturas mientras mantiene el mutex compartido con los demás comandos.

El listado está limitado a 200 y sus índices son adecuados. El coste sin límite está en la verificación. En la comprobación en memoria, esta consulta también necesitó ordenar mediante una tabla temporal.

Con doce indicadores cambiantes cada dos segundos pueden añadirse **518.400 filas diarias por astronauta**; aun constantes, el máximo sostenido de una lectura por minuto produce 17.280.

**Corrección:** agrupar solicitudes del panel, impedir verificaciones simultáneas y separar la auditoría completa del refresco habitual. Revisar ejecución en segundo plano, conexión de lectura e índice `(astronaut_key, reading_id)`. Definir capacidad y archivado conservando la trazabilidad.

El crecimiento y el trabajo por fila están demostrados por el código; **no medí la latencia nativa**.

### 7. M — Las pruebas no acreditan rollback tras una escritura ni el contrato IPC completo

**Ubicación:** [telemetry.rs:781](C:/Users/Salva/Desktop/git/nasa.spaceapps/crates/store/src/telemetry.rs:781), [src-tauri/lib.rs:400](C:/Users/Salva/Desktop/git/nasa.spaceapps/src-tauri/src/lib.rs:400), [informe UI:38](C:/Users/Salva/Desktop/git/nasa.spaceapps/reviews/telemetria-local-ui-informe.md:38).

**Escenario:** el test de atomicidad rechaza un indicador durante la validación previa a la transacción. Prueba correctamente ese rechazo completo, pero no un rollback después de insertar una primera fila. El test de Tauri llama directamente a `Store` y deserializa `"{}"` como estructura interna; no pasa por `invoke` ni prueba la ausencia de la clave externa `input`.

Las pruebas UI declaradas viven en un bundle temporal, por lo que no son reproducibles desde la entrega.

**Corrección:** provocar un fallo SQL en una inserción posterior del lote y comprobar que ninguna queda guardada; probar el despachador IPC con y sin `input`, incluyendo fechas; conservar pruebas UI ejecutables en el repositorio.

Esto es una carencia de cobertura: el código transaccional revisado no demuestra un fallo de rollback.

### 8. M — `aria-busy` puede quedar activado sin ninguna consulta pendiente

**Ubicación:** [telemetry-history.ts:399](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:399), [telemetry-history.ts:412](C:/Users/Salva/Desktop/git/nasa.spaceapps/ui/src/ui/telemetry-history.ts:412).

**Escenario:** una consulta establece `aria-busy=true`; antes de terminar, otro refresco recibe una tripulación vacía. Esa rama sustituye el contenido y retorna sin retirar el atributo. La respuesta anterior se descarta por `requestSeq`, por lo que tampoco lo limpia.

**Corrección:** retirar `aria-busy` en todas las salidas que abandonan el estado de carga, incluidas tripulación vacía y fuera de Tauri.

## Comprobaciones favorables

- **Contrato:** nombres de los tres comandos, envoltorio `{ input }` y campos `snake_case` coinciden.
- **Fechas:** `Timestamp` serializa y acepta RFC 3339; `Iso` es correcto para IPC. Los enteros de SQLite son almacenamiento interno.
- **`verify_telemetry` opcional:** la firma y el deserializador local de Tauri admiten omitir `input`; `{ input: {} }` también verifica todo. Sin ejecución IPC en vivo.
- **Valores y unidades UI:** las doce conversiones coinciden con lo almacenado y leído; Rust conserva el JSON sin reinterpretar sus claves.
- **Hashes:** prefijos de longitud, dominio y orden de campos eliminan la ambigüedad de separadores; las series Rust se separan por astronauta e indicador.
- **Regla temporal:** implementa `<60.000` para omitir y permite guardar exactamente a los 60.000 ms; un duplicado consecutivo dentro del lote ve la inserción anterior.
- **Concurrencia y atomicidad:** los comandos comparten mutex; consulta e inserción pertenecen a una transacción y el éxito se devuelve después del commit.
- **Migración:** para `user_version=3`, v4 añade estructuras en una transacción sin modificar tablas anteriores; el límite soportado está actualizado a 4.
- **Aislamiento:** el registro no escribe en las cuatro tablas prohibidas y `clear_demo_in` no incluye telemetría.
- **XSS y ciclo de vida:** el panel usa `textContent` y propiedades DOM; no encontré `innerHTML` con nombres o valores. `stop()` limpia el intervalo y `destroy()` retira listeners.
- **Pánicos:** no encontré nuevos `unwrap()`, `expect()` o `panic!` fuera de tests; `unwrap_or` no introduce un pánico.

## Dudas de contrato, sin calificarlas como incumplimientos demostrados

- Rust acepta JSON no nulo genérico: por ejemplo, `{"bpm":null}` o claves ajenas al indicador. La UI normal filtra números no finitos; un `NaN` enviado directamente por un consumidor JavaScript puede convertirse en un `null` anidado. Si los prompts exigían estructura por indicador, falta esa validación; la documentación entregada solo exige JSON no nulo y tamaño máximo.
- La igualdad es estructural de `serde_json::Value`, sin tolerancia numérica: `72` y `72.0` pueden considerarse distintos. La UI habitual normaliza esa representación al serializar. Hace falta fijar si «valor distinto» significa igualdad JSON o igualdad numérica.
- Un filtro `indicator` desconocido devuelve una lista vacía; el rechazo de indicadores desconocidos sí está aplicado al registro. No consta aquí que también deba rechazarse en consultas.