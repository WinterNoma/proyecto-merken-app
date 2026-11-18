# Etapa 1 de Pulsys · Recorrido de pulso

## 1. Qué hice

- Registré `pulsys-api` en un workspace de siete miembros y expuse apertura, pulso, historial y zonas. Cargo completó el lock durante la verificación de Claude.
- La migración v6 guarda registro, outbox y recibo en una transacción. La exportación conserva las tres claves públicas de sincronización y la fachada abre `pulsys.sqlite3`.
- En la ronda 2, un cambio de persona invalida la acción pendiente. El replay funciona con el reloj caído y la aprobación se vuelve a comprobar con los avisos y la DEMO vigentes.
- El recorrido de pulso resuelve la raíz desde `replaces` y el historial recorre el índice de fecha hasta reunir el número de ganadores pedido.
- En la ronda 3, una corrección sin raíz resuelta devuelve `unresolved_root/replaces`. La regresión intenta confirmar una acción restaurada antes de que llegue el antecesor y comprueba cero escrituras.

## 2. Archivos tocados

- `Cargo.toml` y `Cargo.lock` en la etapa inicial. Claude completó el lock antes de la ronda 2.
- `crates/api/{Cargo.toml,clippy.toml,src/lib.rs,src/records.rs,tests/pulse_flow.rs,tests/zones.rs}`.
- `crates/store/{Cargo.toml,src/lib.rs,src/records.rs,src/flow_tests.rs,src/telemetry.rs,src/export.rs}`.
- `crates/store/src/{schema.rs,profile.rs,outbox.rs,pulse.rs,pulse/receipt.rs,pulse/target.rs,pulse_history.rs,record_graph.rs}`.
- `crates/store/src/{pulse_tests.rs,pulse_tests/confirmation.rs,pulse_tests/history.rs}`.
- `reviews/etapa1-pulso-sol.md`.

## 3. Comandos y resultados

### Resultados de Claude antes de corregir la exportación

- `Cargo.lock`. La resolución previa estaba incompleta. Cargo añadió 13 paquetes, entre ellos `aho-corasick`, `borsh` y `cookie`. Claude dejó el lock corregido.
- `cargo check` del workspace pasó.
- `cargo test -p pulsys-api` pasó 8 de 8 pruebas y `cargo test -p pulsys-core` pasó 86 de 86.
- `cargo test -p pulsys-store` pasó 62 de 63 pruebas. Falló `export::tests::empty_database_exports_empty_lists` porque `sync_state` exportaba `pulse_store_id` y `demo_generation`.
- `cargo clippy` con `-D warnings` pasó en `pulsys-api` y `pulsys-store`.
- `cargo fmt --all -- --check` señaló cambios de formato en la fachada y los archivos nuevos de store. Claude aplicará el formato.

### Verificación tras la corrección de exportación

La consulta ejecutada directamente con `node:sqlite` devolvió `{}` cuando la tabla solo tenía claves internas. Con estado público, devolvió `pull_cursor`, `last_pull_at` y `last_push_at` con sus valores. El test original conserva su contenido y añadí `export_sync_state_omits_internal_keys`. El lock corregido por Claude coincide byte por byte con el que leí al iniciar esta corrección.

Claude informó que la corrección pasó `cargo test -p pulsys-store` con 64 de 64 pruebas y `cargo test -p pulsys-api` con 8 de 8. Esos resultados preceden a la ronda 2.

### Verificación auxiliar de la entrega inicial

En SQLite 3.53.4, la fixture de migración v5 a v6 conservó perfil, corrección, outbox, cursor y telemetría. `PRAGMA integrity_check` devolvió `ok`. Cuatro consultas de historial devolvieron los IDs esperados y el plan usó `records_indicator_history` y `records_replaces`. Un fallo forzado al insertar el recibo dejó los conteos de registro, outbox y recibo en cero tras `ROLLBACK`.

Con un límite fijo de 20 puntos, la mediana de 10 consultas fue 0,36 ms sobre 30 filas y 12,89 ms sobre 2.000. La medición incluye la función de validación implementada para la fixture en Node.

### Verificación de la ronda 2

- `node:sqlite` 3.53.4 conservó el JSON de un perfil sin ID al subir de v5 a v6. La prueba Rust intenta preparar un pulso hasta reparar el perfil y cubre IDs nulos, en blanco y numéricos.
- El SQL de la cadena fuera de orden devolvió `C/C` y `A/A` con B ausente, `C/A` cuando llegó B y `D/A` después de corregir C. Un intervalo que excluye D devolvió cero puntos. También pasaron las cuatro consultas de la fixture previa. En otras 36 fixtures sintéticas, 180 consultas coincidieron con una implementación auxiliar de la precedencia del núcleo.
- Con 2.004 filas y límite uno, el recorrido por `records_indicator_history` devolvió el último pulso con una sola llamada al validador de la fixture. El índice entregó el orden requerido por los candidatos.
- Las pruebas Rust añadidas cubren cambio de persona, reparación de v5, acción deserializada incoherente, raíz fuera de orden, corte del historial, replay y conflicto con reloj fallido, y el DTO completo de zonas.

El entorno devolvió `CreateProcessAsUserW failed: 5 (Access is denied)`. Revisé las diferencias leyendo los objetos y el índice de Git y los archivos nuevos del árbol de trabajo.

**NO EJECUTADO** `git diff --stat`

**NO EJECUTADO** `cargo test --locked -p pulsys-core -p pulsys-store -p pulsys-api`

**NO EJECUTADO** `cargo check --workspace --locked`

**NO EJECUTADO** `cargo clippy --locked -p pulsys-api -p pulsys-store --all-targets -- -D warnings`

**NO EJECUTADO** `cargo fmt --all -- --check`

### Cierre de verificación de la ronda 2

- Claude informó `cargo test` con core 86/86, store 69/69 y api 9/9 en el flujo de pulso más 1/1 en zonas. Clippy pasó en api y store.
- Astra cerró los hallazgos 1, 2, 4 y 5 y el caso de zonas. Astra mantuvo abierto el hallazgo 3 por el caso de confirmar una corrección antes de recibir el antecesor. Su `cargo fmt --all -- --check` devolvió salida 1. [Revisión de Astra](etapa1-revision-astra-r2.md)

### Verificación de la ronda 3

- En `node:sqlite` 3.53.4, la consulta de raíz devolvió cero filas con B ausente o con un ciclo. Al enlazar B con A, devolvió A para C. Esta comprobación ejecutó el SQL de resolución.
- La nueva prueba `orphan_correction_is_rejected_before_its_ancestor_arrives` comprueba `unresolved_root/replaces` en preparación y confirmación. Tras intentar confirmar la acción restaurada, la fixture conserva C, cero entradas de outbox, cero recibos y el mismo contador de escrituras. La prueba previa conserva el caso de guardado válido después de recibir B.

**NO EJECUTADO** `cargo test --locked -p pulsys-store pulse_tests::history`. El entorno devolvió `CreateProcessAsUserW failed: 5 (Access is denied)`.

**NO EJECUTADO** `cargo clippy --locked -p pulsys-api -p pulsys-store --all-targets -- -D warnings`. El entorno devolvió el mismo error.

**NO EJECUTADO** `cargo fmt --all -- --check` sobre esta corrección. Claude aplica el formato.

## 4. Pendiente

Quedan por ejecutar las pruebas Rust y Clippy de la ronda 3 y verificar el formato que aplica Claude.

El hallazgo 6 de Astra queda para la Etapa 2. Falta definir la conversión durable y versionada del DTO del puente y probar su recuperación al reiniciar.

La exclusión de Tauri pedida para esta tarea tiene precedencia sobre la conexión con Tauri citada en la Etapa 1 del plan. El primer recorrido se expone desde la fachada Rust. El archivo del almacén de la fachada se llama `pulsys.sqlite3` por decisión del usuario.

## 5. Migración y decisiones del recorrido

La migración v6 añade `pulse_operations`, dos índices para el historial y dos valores internos en `sync_state` que identifican el almacén y la generación DEMO. La versión del esquema sube en la misma transacción. Los datos clínicos, la cola, el cursor y la telemetría existentes se conservan. Abrir una versión futura devuelve `invalid_schema` antes de cambiar el modo de journal.

Un perfil existente necesita un `astronaut.astronaut_id` de texto y con contenido. Un perfil v5 sin identidad se conserva al migrar. Preparar devuelve `invalid_schema/astronaut.astronaut_id` hasta que una actualización válida asigne el ID. Si el destino de una acción pendiente cambia, confirmar devuelve `action_expired/key` sin guardar.

El UUID se normaliza antes de consultar la clave. La huella versionada usa el destino, la raíz de corrección cuando aplica, el reemplazado, el valor de bpm, la nota, el origen y el instante medido fijado. `BEGIN IMMEDIATE` ordena dos confirmaciones concurrentes, y la consulta de la clave precede a la reevaluación de avisos y del destino. Una huella igual recupera el recibo original. Una distinta devuelve `idempotency_conflict/key`. Ambos resultados se resuelven dentro de la transacción antes de invocar el reloj.

`PreparedPulse::approve` documenta la decisión explícita del llamador. Rust deriva la aprobación requerida de los avisos y de la DEMO vigentes. Cuando cambian, el resultado entrega una preparación que requiere otra aprobación. Con un aviso o una DEMO por sustituir, un estado restaurado `NotRequired` devuelve `NeedsConfirmation`. Una corrección DEMO queda en el dispositivo. La limpieza de DEMO elimina el contenido de sus recibos y conserva la clave retirada, que devuelve `action_expired/key`. Si cambia el perfil local de una acción pendiente, también se devuelve `action_expired/key`.

Preparar, confirmar por primera vez y consultar zonas leen el reloj una vez al llegar a la operación temporal. Preparación, confirmación e historial siguen los enlaces `replaces` para resolver la raíz. El historial atribuye raíz propia a un registro recibido cuya cadena siga incompleta o sea circular. Preparar o confirmar una corrección de esa cadena devuelve `unresolved_root/replaces` antes de escribir. En el guardado, solo un pulso sin `replaces` recibe su propio ID como raíz. El historial recorre candidatos por fecha y orden local descendentes, comprueba el ganador contra toda su raíz y se detiene al reunir el límite pedido. El límite predeterminado es 100 y los límites explícitos se acotan entre 1 y 500. La evaluación clínica y sus fuentes provienen de las reglas ya presentes en el núcleo. Los ejemplos de esta etapa usan datos sintéticos.

## 6. Contrato público de `pulsys-api`

```rust
pub type Clock = Box<dyn Fn() -> Result<Timestamp, ApiError> + Send>;

pub fn system_clock() -> Result<Timestamp, ApiError>;
pub fn open_store(private_dir: &Path, app_version: String, clock: Clock) -> Result<App, ApiError>;
pub fn close_store(app: App) -> Result<(), ApiError>;
pub fn prepare_pulse(app: &App, input: PulseInput) -> Result<PreparedPulse, ApiError>;
pub fn confirm_pulse(app: &mut App, action: PreparedPulse) -> Result<SaveOutcome, ApiError>;
pub fn effective_pulse_history(app: &App, query: HistoryQuery) -> Result<Vec<PulsePoint>, ApiError>;
pub fn zone_status(app: &mut App) -> Result<ZoneSnapshot, ApiError>;

pub enum SaveOutcome {
    NeedsConfirmation(PreparedPulse),
    Saved(PulseReceipt),
}
```

`PreparedPulse` ofrece `key() -> &str`, `bpm() -> f64`, `measured_at_ms() -> i64`, `note() -> Option<&str>`, `replaces() -> Option<&str>`, `target() -> &PulseTarget`, `warnings() -> &[PulseWarning]`, `demo_to_replace() -> Option<&DemoTarget>`, `requires_confirmation() -> bool` y `approve(self) -> Self`. Serde permite conservar la acción aprobada en el almacenamiento privado del futuro adaptador y reenviar esa misma acción, incluida la hora medida resuelta.

Los siguientes ejemplos usan una sesión mutable abierta sobre una base de prueba vacía, un reloj fijo y un UUID distinto por intención.

### Éxito

```rust
let input = PulseInput {
    key: "00000000-0000-4000-8000-000000000001".into(),
    bpm: 73.5,
    measured_at_ms: None,
    note: None,
    replaces: None,
};
let prepared: PreparedPulse = prepare_pulse(&app, input)?;
let saved: SaveOutcome = confirm_pulse(&mut app, prepared)?;
if let SaveOutcome::Saved(receipt) = saved {
    let receipt: PulseReceipt = receipt;
    assert_eq!(receipt.outbox_state, PulseOutboxState::Pending);
    assert_eq!(receipt.unit, "bpm");
}
```

### Aviso y posterior aprobación

```rust
let input = PulseInput {
    key: "00000000-0000-4000-8000-000000000002".into(),
    bpm: 201.0,
    measured_at_ms: None,
    note: None,
    replaces: None,
};
let prepared: PreparedPulse = prepare_pulse(&app, input)?;
let pending: SaveOutcome = confirm_pulse(&mut app, prepared)?;
if let SaveOutcome::NeedsConfirmation(action) = pending {
    let warnings: &[PulseWarning] = action.warnings();
    assert!(warnings.iter().any(|warning| warning.code == "unusual_value"));

    let approved: PreparedPulse = action.approve();
    let outcome: SaveOutcome = confirm_pulse(&mut app, approved)?;
    assert!(matches!(outcome, SaveOutcome::Saved(_)));
}
```

### Fallo de validación

```rust
let input = PulseInput {
    key: "00000000-0000-4000-8000-000000000003".into(),
    bpm: f64::NAN,
    measured_at_ms: None,
    note: None,
    replaces: None,
};
let result: Result<PreparedPulse, ApiError> = prepare_pulse(&app, input);
if let Err(error) = result {
    assert_eq!(error.code, "invalid_value");
    assert_eq!(error.field.as_deref(), Some("value.bpm"));
}
```
