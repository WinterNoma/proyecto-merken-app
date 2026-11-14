# Revisión core v1 — Sol

Revisor: Sol (gpt-6.1-sol, high) · 14-nov-2026 · Guardado por Claude (el sandbox de Sol era de solo lectura). Las correcciones están al final, en «Respuesta de Claude».

## Veredicto

**No aprobar todavía para trabajar con los datos de la persona real: 5 bloqueantes.** La separación del núcleo y la validación numérica están bien encaminadas, pero hay mezcla DEMO/real, silenciamiento de eventos y defectos de descarga que pueden esconder correcciones.

Revisión estática contra el contrato backend v0.3 y el contraste de Hydrogen. El entorno bloqueó `git` y `cargo` con `Access denied/EPERM`; delimité los cambios leyendo HEAD, índice y objetos Git directamente. Los 39 tests y clippy en verde son evidencia comunicada por el equipo, no repetida aquí. Sin ejecución Android ni mediciones de rendimiento.

## **Bloqueantes**

1. **DEMO y datos reales se evalúan juntos.**

   **Archivo:línea:** `crates/store/src/records.rs:298`, `:445`, `:549`; `crates/store/src/lib.rs:428`. El test `crates/store/src/flow_tests.rs:339` acepta expresamente registrar datos reales con DEMO cargado.

   **Problema:** `load_demo` bloquea real → DEMO, pero `log_record` crea registros reales aunque exista DEMO. Evento, perfil real y pull tampoco aíslan ambos conjuntos. `status` carga todo junto.

   Ejemplo: cargar `routine` y registrar un único pulso real de 72 permite usar lecturas sintéticas como historial y devolver `stable`. Si la evaluación incluye algún ID DEMO, se marca DEMO y no se encola, aunque incluya el registro real.

   **Por qué importa:** fabrica una referencia personal y puede ocultar cambios reales o impedir que su evaluación llegue a Tierra. El distintivo DEMO no evita esa contaminación.

   **Corrección exacta propuesta:** para esta entrega, al comenzar una escritura real, eliminar únicamente DEMO en la misma transacción de esa escritura y comunicar el cambio de modo. Cubrir registro, evento, perfil real y pull. Alternativamente, separar físicamente las bases y seleccionar un único modo explícito. Nunca evaluar ambos conjuntos juntos.

   Sustituir el test que autoriza la mezcla por DEMO → primer registro/evento/pull real: ninguna referencia sintética, un pulso real → `too_few_data`, outbox exclusivamente real. `Need help` debe poder guardarse durante la transición.

2. **Las alarmas de eventos desaparecen a las 24 horas sin resolución.**

   **Archivo:línea:** `crates/core/src/evaluate.rs:294`; `crates/core/src/rules.rs:42`; `crates/core/src/tests.rs:233`.

   **Problema:** al llegar exactamente a 24 h, `need_help`, `syncope`, `visual_field_loss` y `suspected_clot` salen de `alerts` sin cierre registrado. Un `need_help` introducido con `occurred_at` antiguo puede nacer sin alarma. La evaluación histórica guardada no sostiene el estado actual.

   **Por qué importa:** el tiempo transcurrido puede silenciar la única señal de emergencia. Hydrogen lo identifica como crítico en `reviews/umbrales-hydrogen.md:88`.

   **Corrección exacta propuesta:** eliminar el filtro de antigüedad para eventos de alarma en esta entrega. Mantenerlos activos hasta disponer de una resolución explícita acordada en el contrato. No usar `feeling_report` ni una lectura normal como cierre implícito.

   Sustituir el test de desaparición por casos a 24/48 h, evento antiguo y reloj adelantado; todos conservan `alarm/urgent`. Actualizar documentación y comentarios TS que prometen caducidad.

3. **El estado global puede mostrar `stable` con seis indicadores sin evaluar.**

   **Archivo:línea:** `crates/core/src/evaluate.rs:319`, orden de estados en `:13`; uso de cabecera en `docs/api/comandos-core.md:74`.

   **Problema:** `max()` considera `not_evaluated` menor que `stable`. Cuatro pulsos normales, sin ningún otro indicador, producen un indicador estable, seis `no_data` y `overall.status = stable`. También sucede con otras series `too_few_data` o `stale`.

   La base completamente vacía sí devuelve `not_evaluated`; el defecto es la cobertura parcial.

   **Por qué importa:** la cabecera puede afirmar estabilidad global aunque falten datos para sostenerla.

   **Corrección exacta propuesta:** conservar prioridad `alarm`, después `change`; si ninguna existe, devolver `not_evaluated` cuando alguna de las siete series generales o alguna serie de síntomas presente esté sin evaluar. `stable` solo cuando todas esas evaluaciones estén evaluadas y estables. No exigir datos de zonas sin síntomas.

   Hacer que `worst` corresponda a las series que determinan el resultado. Probar stable/no_data, stable/too_few_data, stable/stale y alarm/not_evaluated. Si se adopta otra cobertura, declararla y mostrarla expresamente junto al estado parcial.

4. **El cursor avanza sobre entidades que no se han conservado.**

   **Archivo:línea:** `crates/store/src/records.rs:706`, `:709`, `:749`, `:773`, `:776`; test `crates/store/src/flow_tests.rs:437`.

   **Problema:** un record/event inválido se omite con `continue`; `analysis`, `suggestion_review` y los demás tipos se omiten con `_ => {}`. Después se guarda igualmente `next_cursor`. El test descarta un `analysis` y confirma el cursor.

   **Por qué importa:** esos cambios no reaparecerán en siguientes descargas. La transacción SQL es atómica, pero la página no queda conservada íntegramente. El helper ofrece una semántica incorrecta aunque HTTP siga pendiente.

   **Corrección exacta propuesta:** conservar todas las entidades del diario, al menos como JSON íntegro con tipo/ID/seq, en la misma transacción del cursor; materializar record/event además.

   Ante payload inválido o tipo que todavía no pueda conservarse, devolver error y revertir toda la página sin avanzar el cursor. Validar coherencia entre `c.id`, ID del payload, `c.seq`, versión y metadatos requeridos.

   Probar página válida + evento inválido y página con analysis/review: nunca cursor avanzado sin conservación de todos los cambios entregados.

5. **Ignorar duplicados de pull puede esconder una corrección de Tierra.**

   **Archivo:línea:** `crates/store/src/records.rs:725`, `:755`; prioridad de orden en `crates/core/src/effective.rs:39`.

   **Problema:** `INSERT OR IGNORE` no incorpora `seq/received_at` a un ID local existente.

   Reproducción: A local normal se publica, pero se pierde la respuesta; Tierra publica B con `replaces=A` y valor de alarma; pull devuelve A y B. A conserva `seq=None`, B tiene seq del servidor y el selector considera A mayor: el valor normal gana a la corrección alarmante.

   **Por qué importa:** puede silenciar una alarma y seguir usando `logged_at` cuando ya existe `received_at` autoritativo.

   **Corrección exacta propuesta:** para un ID existente, comprobar equivalencia del contenido clínico inmutable y actualizar únicamente `seq/received_at` autoritativos, conciliando su outbox en esa transacción. Conflicto de contenido → error sin mover cursor. No sobrescribir campos clínicos.

   Probar pérdida de respuesta de A + corrección B + pull: B gana por seq y da alarma; repetir la página es idempotente.

## Importantes

1. **Sueño sin ventana: divergencia del contrato.**

   `crates/core/src/measurement.rs:148`, `ui/src/core-api.ts:34`, `docs/api/comandos-core.md:53` y decisión 003:32 permiten omitir ambos extremos y encolar ese DTO. §5 exige relacionar `hours` con una ventana positiva de hasta 24 h; sin extremos no se puede comprobar esa relación.

   Pedir ambos extremos para sueño real y exigirlos al validar/serializar; actualizar también seed con ventanas sintéticas válidas. No inventarlas silenciosamente. Si se quieren opcionales, acordar y versionar primero esa modificación en la fuente de verdad.

2. **Un síntoma futuro confirmado se guarda, pero el comando devuelve error.**

   `crates/store/src/records.rs:322` hace commit antes de buscar evaluación en `:326–331`. Si es el primer síntoma de esa zona y supera `logged_at+10 min`, `evaluate_all` excluye el registro y no genera esa serie (`crates/core/src/evaluate.rs:364`).

   Resultado: `storage_error` con entidad y outbox ya guardadas; reintentar puede duplicar el síntoma.

   Crear para la clave solicitada una evaluación explícita `not_evaluated/no_data`, sin incorporar el futuro, y devolver `saved` con su aviso. Probar primer síntoma futuro y corrección futura. Separar en general el resultado durable del guardado de un fallo posterior al evaluarlo.

3. **Trabajo sin límite de historial bajo el bloqueo compartido.**

   `crates/store/src/records.rs:118`, `:415`, `:445` cargan, deserializan y revalidan todo. `list_records` limita después; `save_record` hace dos cargas completas.

   `crates/core/src/effective.rs:47–69` recorre cada cadena desde cero: una cadena de N correcciones cuesta O(N²). Todos los comandos retienen el mismo Mutex (`src-tauri/src/lib.rs:38–43`), retrasando también ayuda.

   Cachear raíces/efectivos y evaluaciones por serie, invalidando por inserción y cambio de metadatos; memoizar raíces y limitar en SQL el historial de la vista. Preservar ancestros y últimas alarmas: truncar indiscriminadamente por fecha rompería las reglas.

   Medir en el móvil con historial representativo y cadenas largas. No hay evidencia aquí de una latencia concreta ni de un incumplimiento ya medido.

4. **La cita NEWS2 no respalda el operador de temperatura en 38.0.**

   `crates/core/src/rules.rs:182–183` marca `>=38.0` como `published`, pero la [tabla oficial NEWS2](https://www.rcp.ac.uk/media/alxev00t/news2-chart-1_the-news-scoring-system_0_0.pdf) incluye 38.0 en puntuación cero y empieza el tramo superior en 38.1.

   Añadir una fuente primaria que respalde exactamente ese corte o marcarlo `demo_heuristic` hasta documentarla. Si se pretende implementar NEWS2, definir también el tratamiento de decimales.

   En [ADA 2026 §6](https://pmc.ncbi.nlm.nih.gov/articles/PMC12690178/), los cortes glucémicos citados sí aparecen; la clasificación específica de hipoglucemia está en tabla 6.4. Los rangos CGM de diabetes y la [guía AHA/ACC 2025](https://www.ahajournals.org/doi/10.1161/CIR.0000000000001356) no validan por sí solos todo el escalado automático en astronautas: distinguir corte publicado y decisión operativa.

   No encontré reglas `published` con `source_ref=None`.

5. **Extensiones del DTO de evaluación pendientes de acuerdo.**

   `crates/store/src/records.rs:98` añade `body_zone`, ausente del DTO de evaluación §3; `crates/core/src/evaluate.rs:305` usa nombres de eventos como `indicator`, fuera del catálogo §5. Decisión 003:76 reconoce pendiente la aceptación por backend.

   No está demostrado ni su rechazo ni su aceptación. Cerrar y versionar el acuerdo, y probar el payload completo contra el validador del backend antes de sincronizarlo. Mantener localmente esas evaluaciones hasta resolverlo, sin declararlas sincronizadas.

   Los DTO ordinarios de record/event sí excluyen campos de interfaz.

## Menores

1. **Diseño documental desactualizado.** `docs/decisiones/003-nucleo-rust.md:60` describe caducidad antes de alarmas, aunque el código conserva alarmas absolutas antiguas. También usa nombres distintos de la API real: `Score0To10`, `effective::select` y `evaluate_indicator`. Actualizar a `Score`, `EffectiveSet::build` y `evaluate_series`, documentando la excepción de alarma antigua.

2. **Pruebas JSON parciales.** `crates/store/src/flow_tests.rs:454` verifica claves parciales; no lee TS ni cubre todos los comandos. Por inspección, envolturas, nombres, enums y salidas normales coinciden con `core-api.ts`. El perfil real es `Value` (`crates/store/src/api.rs:14`) y `upsert_profile` no garantiza sus campos internos: usar DTO validado al integrar la descarga y fixtures completos de profile/event/log/errors.

3. **Mensaje de historial ambiguo.** `docs/api/comandos-core.md:66` propone “Need 3 readings”, pero `history_count` cuenta anteriores: son tres previas más la actual. Cambiar a “Need 3 previous readings” o mostrar ambas cantidades.

## Lo que está bien

- Core no lee reloj, archivos o red ni genera UUID; el instante entra por parámetro y el orden observable es determinista.
- No encontré `unwrap()`, `expect()`, `panic!()` ni indexación insegura en las rutas productivas revisadas. `get` y `unwrap_or` no equivalen a un unwrap que aborte.
- Duraciones usan i128/saturación; la mediana promedia mitades y no divide por cero. El `as f64` del seed solo convierte 0, 1 o 2.
- Números finitos, unidades y puntuaciones se validan sin rechazar extremos reales: pulso 300, glucosa 2375 y temperatura 14 se aceptan con confirmación. Cero de ánimo/sueño es válido.
- La relación sistólica/diastólica genera aviso, no rechazo duro.
- Sin efectivos → `no_data`; lectura normal aislada → `too_few_data`; alarmas absolutas funcionan sin historial y las últimas alarmas absolutas sobreviven a `stale`.
- Los eventos futuros no pierden alarma por límite superior. Los textos revisados son orientativos y no afirman diagnósticos.
- La selección efectiva excluye futuros respecto de `received_at/logged_at` antes de elegir ganador, sigue cadenas y ordena por seq entre sincronizados.
- Ciclos y referencias ausentes terminan sin bucle infinito. Las correcciones locales exigen registro previo y preservan indicador y zona. B5 debe corregirse para proporcionar metadatos correctos.
- Registro+outbox, evento+outbox y evaluaciones+outbox son transaccionales. Las correcciones insertan otra fila y `clear_demo` solo borra DEMO.
- Actualizar perfil, metadatos y cola no constituye sobrescritura del contenido clínico.
- SQL parametrizado; migración v0→v1 transaccional con rechazo de versiones futuras; outbox FIFO que conserva fallos definitivos.
- La firma documentada deduplica evaluaciones ante consultas repetidas.
- Seed es ficticio y determinista en valores/tiempos relativos; `load_demo` por sí solo no encola datos sintéticos y bloquea si ya hay reales. Falta la protección inversa de B1.
- La verificación médica fue puntual sobre NEWS2, ADA y AHA/ACC; no equivale a validación clínica de todas las reglas ni de su aplicación en vuelo.

**Resumen de 10 líneas**

1. Veredicto: pendiente de correcciones; **5 bloqueantes**.
2. B1: mezcla de historial DEMO con datos reales.
3. B2: eventos urgentes desaparecen sin resolución.
4. B3: estabilidad global con cobertura insuficiente.
5. B4: cursor avanza sobre información descartada.
6. B5: duplicados descargados pueden esconder correcciones.
7. Revisar además sueño, síntomas futuros y rendimiento.
8. JSON normal Rust/TypeScript coincide por inspección.
9. Cargo y Android no pudieron verificarse aquí.
10. Archivo no creado: informe completo entregado para guardarlo.
---

## Respuesta de Claude (14-nov-2026)

Verificado en escritorio tras cada ronda: `cargo test --workspace` (46 tests) y `cargo clippy --workspace --all-targets -- -D warnings` en verde; los 9 comandos se llamaron desde la WebView de `npm run tauri dev` (vía CDP).

| Punto | Estado | Qué se hizo |
|---|---|---|
| B1 DEMO + real | Corregido (verificado por Sol) | `clear_demo_in` en la misma transacción de la primera escritura real: registro, evento, perfil real y `pull`. `status` excluye además lo DEMO si hay datos reales. Tests `demo_never_reaches_outbox_and_blocks_on_real_data`, `need_help_during_demo_is_saved_as_real` |
| B2 alarmas de evento | Corregido (verificado por Sol) | Sin caducidad ni filtro horario. **Pendiente de contrato:** cómo se resuelve una alarma |
| B3 estado global | Corregido (verificado por Sol) | `alarm` > `change` > `not_evaluated` si falta alguna serie > `stable` |
| B4 cursor | Corregido (2.ª ronda) | Tabla `pulled_changes` (migración v2) con todo lo recibido; `seq`, `received_at` y `schema_version` obligatorios; mismo `seq` con otro contenido → `conflict`; cualquier error deshace la página y no mueve el cursor |
| B5 duplicados | Corregido (2.ª ronda) | ID existente: se compara el DTO de entrada completo; si coincide solo se añaden `seq`/`received_at` y se concilia la outbox; si difiere → `conflict`. Un `seq` ya conocido distinto → `conflict` (no se reescribe) |
| Imp. 1 sueño | Corregido | Ventana obligatoria; seed con ventanas de 9 h |
| Imp. 2 síntoma futuro | Corregido | Devuelve `saved` con `not_evaluated/no_data` |
| Imp. 3 rendimiento | Pendiente | Medir en el móvil; cachear series si hace falta |
| Imp. 4 cita 38 °C | Corregido | `celsius ≥ 38.1` (NEWS2) |
| Imp. 5 DTO evaluación | Pendiente con backend | `body_zone` en evaluaciones y `indicator = need_help` en alertas |
| Menores 1 y 3 | Corregidos | Decisión 003 actualizada; "Need 3 previous readings" |
| Menor 2 | Parcial | Test de claves de `StatusResponse` y `RecordItem`; el perfil sigue como `Value` hasta que exista la descarga |

### Verificación final de Sol (2.ª ronda)
> No identifico bloqueantes para usar mañana el núcleo con datos reales exclusivamente locales, sin ejecutar sincronización. Sí quedan pendientes antes de habilitarla.

**Pendiente antes de activar `crates/sync`** (no afecta al uso local):
1. `apply_pull_page`: exigir `received_at` y `schema_version` también en entidades que no son `record`/`event` (`records.rs`, bucle de `apply_pull_page`).
2. `pull_record`/`pull_event`: comparar también `external_id` (hoy no se guarda en local) y `app_version`.
3. No reescribir un `received_at` ya conocido si llega distinto, ni en `pull_*` ni en `mark_synced` (`lib.rs`, `mark_synced`): rechazar con `conflict`.
4. Tests para esos tres casos.
