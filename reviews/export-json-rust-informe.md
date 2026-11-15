# Informe: exportación JSON en Rust

Autor: Claude (Terra no pudo compilar: el sandbox de Windows le denegó la creación de procesos).

## Hecho
- `crates/store/src/export.rs`: `Store::export_snapshot(&ExportInput, now, app_version)`; todo en una transacción de lectura, incluida la verificación (`verify_telemetry_in`, extraída de `verify_telemetry`; no cambia su comportamiento).
- `telemetry.rs`: solo extracción de `verify_telemetry_in` y `validate_astronaut_key` a `pub(crate)`.
- `src-tauri/src/lib.rs`: comando `export_json`, `write_export` (create_new + sufijos -2…-20, `.tmp` + rename, nunca sobrescribe), `ExportJsonResult`.
- `docs/api/export-json.md`: formato, comprobación manual con Python y límites.

## Decisiones
- Cada lectura exporta `value` (parseado) y `value_json` (texto exacto) más `observed_at_ms`: el hash usa los bytes de `value_json` y los milisegundos enteros.
- Una fila con JSON corrupto se exporta como texto crudo en lugar de abortar todo.
- `astronaut_key` filtra solo la telemetría; >1 000 000 lecturas obliga a filtrar.

## Verificado (ejecutado)
- `cargo test --workspace` verde (store 55, incl. 7 de export; 2 de `write_export`), `cargo clippy --workspace --all-targets -- -D warnings` limpio, `rustfmt --edition 2024 --check` limpio.
- Export de una copia de la base real: 3100 lecturas, 62 series, `verification.ok = true`, y un script Python independiente recalcula las 3100 con 0 fallos.

## No verificado
- El comando Tauri con `AppHandle` real (solo se prueban `write_export` y `export_snapshot`); el botón en la app en ejecución; ruta de exports en Android.
- Revisión de Sol pendiente.

## Revisión de Sol (reviews/export-json.md) y correcciones
- I temporal inseguro / M `.json` vacío ante interrupción: `write_export` escribe un temporal exclusivo (`create_new`, nombre único) completo y lo publica con `hard_link` (falla si el nombre existe); solo borra su propio temporal.
- I verificador Python incompleto: ahora comprueba orden por `reading_id`, conjunto de series, y cabeza (último id, hash, conteo) por serie. Probado sobre 3100 lecturas reales: 0 problemas.
- M columnas omitidas: documentado en «Qué no se copia».
- M filtro de astronauta en la UI: pendiente (el backend ya lo admite).
