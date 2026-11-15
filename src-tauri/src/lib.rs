//! Capa Tauri: solo une los crates de `crates/` con la interfaz.
//! Ninguna decisión de salud se toma aquí ni en TypeScript.
//!
//! Es el único sitio que lee el reloj del sistema: el instante se pasa a
//! `merken-store` y `merken-core` como argumento (RNF-03).
//! Contrato de los comandos: `docs/api/comandos-core.md` y `ui/src/core-api.ts`.

mod recommendations;

use merken_core::time::Timestamp;
use merken_store::{
    ExportCounts, ExportInput, ListTelemetryInput, RecordTelemetryInput, RecordTelemetryResult,
    Scenario, Store, StoreError, TelemetryItem, VerifyTelemetryResult,
    api::{
        CorrectRecordInput, EventItem, ImportResult, ListRecordsInput, LogRecordInput, LogResult,
        ProfileResponse, RaiseEventInput, RaiseEventResult, RecordItem, StatusResponse,
    },
};
use merken_sync::{
    SyncError,
    simulation::{DEFAULT_BASE_URL, SimAstronaut, SimClient},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

/// Versión de la app que viaja en cada registro (`app_version`).
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Base de datos local, compartida por todos los comandos.
pub struct AppState(Mutex<Store>);

fn now() -> Timestamp {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    Timestamp::from_millis(ms)
}

fn with_store<T>(
    state: &tauri::State<'_, AppState>,
    f: impl FnOnce(&mut Store) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    let mut guard = state.0.lock().map_err(|_| StoreError {
        code: "storage_error".into(),
        field: None,
        message: "The local database is unavailable. Restart the app".into(),
    })?;
    f(&mut guard)
}

#[derive(Serialize)]
pub struct RecordsResponse {
    pub records: Vec<RecordItem>,
}

#[derive(Serialize)]
pub struct EventsResponse {
    pub events: Vec<EventItem>,
}

#[derive(Deserialize, Default)]
pub struct ListEventsInput {
    pub limit: Option<usize>,
}

#[derive(Deserialize)]
pub struct LoadDemoInput {
    pub scenario: String,
}

#[derive(Deserialize, Default)]
pub struct SimListInput {
    /// Token de la API de simulación. Si falta, se usa `MERKEN_SIM_TOKEN`.
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct SimImportInput {
    pub astronaut_id: String,
    /// Días hacia atrás que se descargan (1–31, por defecto 14: la ventana de historial del núcleo).
    pub days: Option<u32>,
    pub token: Option<String>,
}

#[derive(Serialize)]
pub struct SimAstronautsResponse {
    pub astronauts: Vec<SimAstronaut>,
}

#[derive(Debug, Deserialize, Default)]
pub struct VerifyTelemetryInput {
    pub astronaut_key: Option<String>,
}

// ---------- Lógica sin Tauri (se puede probar) ----------

fn sync_err(e: SyncError) -> StoreError {
    StoreError {
        code: e.code,
        field: None,
        message: e.message,
    }
}

/// Token del comando o, si falta, de la variable de entorno `MERKEN_SIM_TOKEN`.
/// Nunca se guarda ni se escribe en registros.
fn sim_client(token: Option<String>) -> Result<SimClient, StoreError> {
    let token = token
        .filter(|t| !t.trim().is_empty())
        .or_else(|| std::env::var("MERKEN_SIM_TOKEN").ok())
        .unwrap_or_default();
    let base = std::env::var("MERKEN_SIM_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_owned());
    SimClient::new(&base, &token).map_err(sync_err)
}

/// Desde cuándo se importa: `now − days` (1–31 días, por defecto 14).
pub fn sim_from(now: Timestamp, days: Option<u32>) -> String {
    let days = i64::from(days.unwrap_or(14).clamp(1, 31));
    now.saturating_add_ms(-days * 86_400_000).to_rfc3339()
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, StoreError> + Send + 'static,
) -> Result<T, StoreError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| StoreError {
            code: "network".into(),
            field: None,
            message: "The simulation download was interrupted".into(),
        })?
}

pub fn handle_list_events(
    store: &Store,
    input: ListEventsInput,
) -> Result<EventsResponse, StoreError> {
    Ok(EventsResponse {
        events: store.list_events(input.limit.unwrap_or(100).clamp(1, 500))?,
    })
}

pub fn handle_load_demo(
    store: &mut Store,
    input: LoadDemoInput,
    now: Timestamp,
) -> Result<StatusResponse, StoreError> {
    let scenario = Scenario::parse(&input.scenario).ok_or_else(|| StoreError {
        code: "invalid_value".into(),
        field: Some("scenario".into()),
        message: "Unknown demo scenario. Use routine, persistent_change or alarm_event".into(),
    })?;
    store.load_demo(scenario, now, APP_VERSION)
}

pub fn handle_verify_telemetry(
    store: &Store,
    input: Option<VerifyTelemetryInput>,
) -> Result<VerifyTelemetryResult, StoreError> {
    let astronaut_key = input.and_then(|i| i.astronaut_key);
    store.verify_telemetry(astronaut_key)
}

// ---------- Exportación a JSON ----------

/// Respuesta del comando `export_json` (contrato en `ui/src/core-api.ts`).
#[derive(Debug, Serialize)]
pub struct ExportJsonResult {
    pub path: String,
    pub file_name: String,
    pub bytes: u64,
    pub exported_at: String,
    pub counts: ExportCounts,
}

const MAX_EXPORT_NAME_ATTEMPTS: u32 = 20;

fn export_io_error(message: &str) -> StoreError {
    StoreError {
        code: "storage_error".into(),
        field: None,
        message: message.into(),
    }
}

/// `merken-export-AAAAMMDDTHHMMSSZ` a partir del instante en UTC.
fn export_stem(now: Timestamp) -> String {
    let iso = now.to_rfc3339(); // AAAA-MM-DDTHH:MM:SS.mmmZ
    let compact: String = iso
        .chars()
        .take_while(|c| *c != '.')
        .filter(|c| c.is_ascii_digit() || *c == 'T')
        .collect();
    format!("merken-export-{compact}Z")
}

/// Escribe el documento en `<dir>/exports/` sin sobrescribir nunca un archivo.
///
/// El contenido se escribe completo en un temporal exclusivo (`create_new`, nombre
/// único) y solo entonces se publica con `hard_link`, que falla si el nombre final ya
/// existe: nunca queda un `.json` vacío ni se reemplaza uno ajeno. Si el nombre está
/// ocupado se prueba con `-2`, `-3`… (hasta 20 intentos).
pub fn write_export(
    dir: &Path,
    now: Timestamp,
    doc: &serde_json::Value,
) -> Result<(PathBuf, u64), StoreError> {
    use std::io::Write as _;
    let exports = dir.join("exports");
    std::fs::create_dir_all(&exports)
        .map_err(|_| export_io_error("Could not create the exports folder"))?;
    let mut text = serde_json::to_string_pretty(doc)?;
    text.push('\n');

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = exports.join(format!(
        ".merken-export-{}-{unique}.tmp",
        std::process::id()
    ));
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|()| f.sync_all()));
    if written.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err(export_io_error("Could not write the export file"));
    }

    let stem = export_stem(now);
    let mut result = Err(export_io_error(
        "Too many exports with the same timestamp. Try again in a moment",
    ));
    for attempt in 1..=MAX_EXPORT_NAME_ATTEMPTS {
        let name = if attempt == 1 {
            format!("{stem}.json")
        } else {
            format!("{stem}-{attempt}.json")
        };
        let target = exports.join(&name);
        match std::fs::hard_link(&tmp, &target) {
            Ok(()) => {
                let bytes = u64::try_from(text.len()).unwrap_or(u64::MAX);
                result = Ok((target, bytes));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => {
                result = Err(export_io_error("Could not create the export file"));
                break;
            }
        }
    }
    // Solo se borra el temporal propio, creado arriba con create_new.
    let _ = std::fs::remove_file(&tmp);
    result
}

/// Construye el snapshot y lo escribe. El candado de la base solo se mantiene
/// mientras se lee; la escritura del archivo ocurre después.
fn handle_export_json(
    state: &tauri::State<'_, AppState>,
    dir: &Path,
    input: Option<ExportInput>,
    now: Timestamp,
) -> Result<ExportJsonResult, StoreError> {
    let input = input.unwrap_or_default();
    let (doc, counts) = with_store(state, |s| s.export_snapshot(&input, now, APP_VERSION))?;
    let (path, bytes) = write_export(dir, now, &doc)?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(ExportJsonResult {
        path: path.to_string_lossy().into_owned(),
        file_name,
        bytes,
        exported_at: now.to_rfc3339(),
        counts,
    })
}

// ---------- Comandos ----------

/// Comprueba de punta a punta que la interfaz llega al núcleo en Rust.
#[tauri::command]
fn core_status() -> Result<String, String> {
    let sqlite = merken_store::version_sqlite().map_err(|e| e.to_string())?;
    Ok(format!(
        "merken-core {} · SQLite {}",
        merken_core::VERSION,
        sqlite
    ))
}

#[tauri::command]
fn get_profile(state: tauri::State<'_, AppState>) -> Result<ProfileResponse, StoreError> {
    with_store(&state, |s| s.get_profile())
}

#[tauri::command]
fn log_record(
    state: tauri::State<'_, AppState>,
    input: LogRecordInput,
) -> Result<LogResult, StoreError> {
    with_store(&state, |s| s.log_record(input, now(), APP_VERSION))
}

#[tauri::command]
fn correct_record(
    state: tauri::State<'_, AppState>,
    input: CorrectRecordInput,
) -> Result<LogResult, StoreError> {
    with_store(&state, |s| s.correct_record(input, now(), APP_VERSION))
}

#[tauri::command]
fn list_records(
    state: tauri::State<'_, AppState>,
    input: Option<ListRecordsInput>,
) -> Result<RecordsResponse, StoreError> {
    with_store(&state, |s| {
        Ok(RecordsResponse {
            records: s.list_records(input.unwrap_or_default())?,
        })
    })
}

#[tauri::command]
fn get_status(state: tauri::State<'_, AppState>) -> Result<StatusResponse, StoreError> {
    with_store(&state, |s| s.status(now(), APP_VERSION))
}

#[tauri::command]
fn raise_event(
    state: tauri::State<'_, AppState>,
    input: RaiseEventInput,
) -> Result<RaiseEventResult, StoreError> {
    with_store(&state, |s| s.raise_event(input, now(), APP_VERSION))
}

#[tauri::command]
fn list_events(
    state: tauri::State<'_, AppState>,
    input: Option<ListEventsInput>,
) -> Result<EventsResponse, StoreError> {
    with_store(&state, |s| handle_list_events(s, input.unwrap_or_default()))
}

#[tauri::command]
fn load_demo(
    state: tauri::State<'_, AppState>,
    input: LoadDemoInput,
) -> Result<StatusResponse, StoreError> {
    with_store(&state, |s| handle_load_demo(s, input, now()))
}

#[tauri::command]
fn clear_demo(state: tauri::State<'_, AppState>) -> Result<(), StoreError> {
    with_store(&state, |s| s.clear_demo())
}

#[tauri::command]
fn record_telemetry(
    state: tauri::State<'_, AppState>,
    input: RecordTelemetryInput,
) -> Result<RecordTelemetryResult, StoreError> {
    with_store(&state, |s| s.record_telemetry(input, now(), APP_VERSION))
}

#[tauri::command]
fn list_telemetry(
    state: tauri::State<'_, AppState>,
    input: ListTelemetryInput,
) -> Result<Vec<TelemetryItem>, StoreError> {
    with_store(&state, |s| s.list_telemetry(input))
}

#[tauri::command]
fn verify_telemetry(
    state: tauri::State<'_, AppState>,
    input: Option<VerifyTelemetryInput>,
) -> Result<VerifyTelemetryResult, StoreError> {
    with_store(&state, |s| handle_verify_telemetry(s, input))
}

/// Exporta todos los datos locales a un archivo JSON en `<datos de la app>/exports/`.
#[tauri::command]
fn export_json(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    input: Option<ExportInput>,
) -> Result<ExportJsonResult, StoreError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|_| export_io_error("The app data folder is unavailable"))?;
    handle_export_json(&state, &dir, input, now())
}

/// Lista los astronautas de la API de simulación (datos inventados).
#[tauri::command]
async fn sim_list_astronauts(
    input: Option<SimListInput>,
) -> Result<SimAstronautsResponse, StoreError> {
    let token = input.unwrap_or_default().token;
    blocking(move || {
        let client = sim_client(token)?;
        Ok(SimAstronautsResponse {
            astronauts: client.list_astronauts().map_err(sync_err)?,
        })
    })
    .await
}

/// Descarga los registros de un astronauta simulado y los guarda como DEMO
/// (sustituye la DEMO anterior; nunca se suben a Tierra). Después, `get_status`.
#[tauri::command]
async fn sim_import(
    state: tauri::State<'_, AppState>,
    input: SimImportInput,
) -> Result<ImportResult, StoreError> {
    let SimImportInput {
        astronaut_id,
        days,
        token,
    } = input;
    let from = sim_from(now(), days);
    let id = astronaut_id.clone();
    let (name, downloaded) = blocking(move || {
        let client = sim_client(token)?;
        let name = client
            .list_astronauts()
            .map_err(sync_err)?
            .into_iter()
            .find(|a| a.astronaut_id == id)
            .map(|a| a.display_name)
            .ok_or_else(|| StoreError {
                code: "not_found".into(),
                field: Some("astronaut_id".into()),
                message: "Astronaut not found in the simulation".into(),
            })?;
        let records = client.records_since(&id, &from).map_err(sync_err)?;
        Ok((name, records))
    })
    .await?;
    // Si la descarga falla (red, límites, paginación incoherente) no se llega
    // aquí y la DEMO anterior queda como estaba.
    with_store(&state, |s| {
        s.import_synthetic(&astronaut_id, &name, &downloaded, now())
    })
}

fn open_store(app: &tauri::App) -> Result<Store, Box<dyn std::error::Error>> {
    let dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&dir)?;
    Ok(Store::open(&dir.join("merken.sqlite3"))?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_slm_assets::init())
        .setup(|app| {
            let store = open_store(app)?;
            app.manage(AppState(Mutex::new(store)));
            app.manage(recommendations::RecommendationState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_status,
            get_profile,
            log_record,
            correct_record,
            list_records,
            get_status,
            raise_event,
            list_events,
            load_demo,
            clear_demo,
            sim_list_astronauts,
            sim_import,
            record_telemetry,
            list_telemetry,
            verify_telemetry,
            recommendations::recommendations_cached_crew,
            recommendations::recommendations_sync_backend,
            recommendations::recommendations_generate,
            recommendations::recommendations_cancel,
            export_json
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("failed to start the Tauri application: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_demo_scenario_is_rejected() -> Result<(), StoreError> {
        let mut store = Store::open_in_memory()?;
        let r = handle_load_demo(
            &mut store,
            LoadDemoInput {
                scenario: "nope".into(),
            },
            Timestamp::from_millis(1_800_000_000_000),
        );
        assert!(r.is_err_and(|e| e.code == "invalid_value"));
        Ok(())
    }

    #[test]
    fn sim_window_defaults_to_14_days_and_is_clamped() {
        let now = Timestamp::from_millis(1_800_000_000_000);
        let day: i64 = 86_400_000;
        let at = |d: i64| Timestamp::from_millis(1_800_000_000_000 - d * day).to_rfc3339();
        assert_eq!(sim_from(now, None), at(14));
        assert_eq!(sim_from(now, Some(0)), at(1));
        assert_eq!(sim_from(now, Some(400)), at(31));
    }

    #[test]
    fn demo_then_status_end_to_end() -> Result<(), StoreError> {
        let mut store = Store::open_in_memory()?;
        let now = Timestamp::from_millis(1_800_000_000_000);
        let st = handle_load_demo(
            &mut store,
            LoadDemoInput {
                scenario: "alarm_event".into(),
            },
            now,
        )?;
        assert!(st.is_demo);
        assert_eq!(st.alerts.len(), 1);
        let events = handle_list_events(&store, ListEventsInput::default())?;
        assert_eq!(events.events.len(), 1);
        Ok(())
    }

    #[test]
    fn telemetry_round_trip_and_verify_command_paths() -> Result<(), StoreError> {
        let mut store = Store::open_in_memory()?;
        let now = Timestamp::from_millis(1_800_000_000_000);
        let input: RecordTelemetryInput = serde_json::from_value(serde_json::json!({
            "readings": [
                {
                    "astronaut_key": "astro-1",
                    "astronaut_name": "Alex Morgan",
                    "indicator": "heart_rate",
                    "value": { "bpm": 72 },
                    "unit": "bpm"
                },
                {
                    "astronaut_key": "astro-2",
                    "astronaut_name": "Sam Vega",
                    "indicator": "hrv",
                    "value": { "ms": 64 },
                    "unit": "ms"
                },
                {
                    "astronaut_key": "astro-1",
                    "astronaut_name": "Alex Morgan",
                    "indicator": "space_radiation",
                    "value": { "usv_h": 14.1 },
                    "unit": "µSv/h"
                },
                {
                    "astronaut_key": "astro-1",
                    "astronaut_name": "Alex Morgan",
                    "indicator": "distance_from_earth",
                    "value": { "km": 418.2 },
                    "unit": "km"
                }
            ]
        }))
        .map_err(|_| StoreError {
            code: "invalid_schema".into(),
            field: None,
            message: "bad test json".into(),
        })?;
        let recorded = store.record_telemetry(input, now, APP_VERSION)?;
        assert_eq!(recorded.stored, 4);

        let later = now.saturating_add_ms(10_000);
        let follow_up: RecordTelemetryInput = serde_json::from_value(serde_json::json!({
            "readings": [
                {
                    "astronaut_key": "astro-1",
                    "astronaut_name": "Alex Morgan",
                    "indicator": "heart_rate",
                    "value": { "bpm": 76 },
                    "unit": "bpm"
                }
            ]
        }))
        .map_err(|_| StoreError {
            code: "invalid_schema".into(),
            field: None,
            message: "bad follow-up json".into(),
        })?;
        let recorded_later = store.record_telemetry(follow_up, later, APP_VERSION)?;
        assert_eq!(recorded_later.stored, 1);

        let list_input: ListTelemetryInput = serde_json::from_value(serde_json::json!({
            "astronaut_key": "astro-1",
            "indicator": "heart_rate",
            "from": "2027-01-15T08:00:05Z",
            "to": "2027-01-15T09:00:15+01:00"
        }))
        .map_err(|_| StoreError {
            code: "invalid_schema".into(),
            field: None,
            message: "bad list json".into(),
        })?;
        let filtered_rfc3339 = store.list_telemetry(list_input)?;
        assert_eq!(filtered_rfc3339.len(), 1);
        assert_eq!(filtered_rfc3339[0].value, serde_json::json!({ "bpm": 76 }));

        let items = store.list_telemetry(ListTelemetryInput {
            astronaut_key: "astro-1".into(),
            ..Default::default()
        })?;
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].app_version, APP_VERSION);

        // Sin `input` (Option::None).
        let verified_none = handle_verify_telemetry(&store, None)?;
        assert_eq!(
            verified_none,
            VerifyTelemetryResult {
                ok: true,
                checked: 5,
                series_checked: 4,
                checked_through_reading_id: Some(5),
                first_bad_reading_id: None,
                problem: None,
            }
        );

        // Con `input: {}`.
        let verify_empty: Option<VerifyTelemetryInput> =
            serde_json::from_str("{}").map_err(|_| StoreError {
                code: "invalid_schema".into(),
                field: None,
                message: "bad verify json".into(),
            })?;
        let verified_empty = handle_verify_telemetry(&store, verify_empty)?;
        assert_eq!(verified_empty, verified_none);

        // Con `input: { astronaut_key }`.
        let verify_scoped: Option<VerifyTelemetryInput> =
            serde_json::from_str(r#"{"astronaut_key":"astro-1"}"#).map_err(|_| StoreError {
                code: "invalid_schema".into(),
                field: None,
                message: "bad scoped verify json".into(),
            })?;
        let verified_scoped = handle_verify_telemetry(&store, verify_scoped)?;
        assert_eq!(
            verified_scoped,
            VerifyTelemetryResult {
                ok: true,
                checked: 4,
                series_checked: 3,
                checked_through_reading_id: Some(5),
                first_bad_reading_id: None,
                problem: None,
            }
        );

        let json_payload = serde_json::to_value(&verified_scoped)?;
        assert_eq!(
            json_payload,
            serde_json::json!({
                "ok": true,
                "checked": 4,
                "series_checked": 3,
                "checked_through_reading_id": 5,
                "first_bad_reading_id": null,
                "problem": null,
            })
        );
        Ok(())
    }

    fn scratch_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "merken-export-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn write_export_never_overwrites_and_leaves_no_tmp() -> Result<(), Box<dyn std::error::Error>> {
        let dir = scratch_dir("write");
        let store = Store::open_in_memory()?;
        let now = Timestamp::from_millis(1_800_000_000_123);
        let (doc, counts) = store.export_snapshot(&ExportInput::default(), now, APP_VERSION)?;
        assert_eq!(counts, ExportCounts::default());

        let (first, bytes) = write_export(&dir, now, &doc)?;
        let (second, _) = write_export(&dir, now, &doc)?;
        let (third, _) = write_export(&dir, now, &doc)?;
        let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
        assert_eq!(
            name(&first).as_deref(),
            Some("merken-export-20270115T080000Z.json")
        );
        assert_eq!(
            name(&second).as_deref(),
            Some("merken-export-20270115T080000Z-2.json")
        );
        assert_eq!(
            name(&third).as_deref(),
            Some("merken-export-20270115T080000Z-3.json")
        );

        let text = std::fs::read_to_string(&first)?;
        assert_eq!(u64::try_from(text.len())?, bytes);
        assert!(text.ends_with("}\n"));
        let parsed: serde_json::Value = serde_json::from_str(&text)?;
        assert_eq!(parsed["format"], "merken-export");
        assert_eq!(parsed, doc);

        let leftovers = std::fs::read_dir(dir.join("exports"))?
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn write_export_gives_up_after_twenty_name_collisions() -> Result<(), Box<dyn std::error::Error>>
    {
        let dir = scratch_dir("collide");
        let now = Timestamp::from_millis(1_800_000_000_000);
        let doc = serde_json::json!({"format": "merken-export"});
        for _ in 0..20 {
            write_export(&dir, now, &doc)?;
        }
        let err = write_export(&dir, now, &doc).err().map(|e| e.code);
        assert_eq!(err.as_deref(), Some("storage_error"));
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
