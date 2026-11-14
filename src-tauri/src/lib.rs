//! Capa Tauri: solo une los crates de `crates/` con la interfaz.
//! Ninguna decisión de salud se toma aquí ni en TypeScript.
//!
//! Es el único sitio que lee el reloj del sistema: el instante se pasa a
//! `merken-store` y `merken-core` como argumento (RNF-03).
//! Contrato de los comandos: `docs/api/comandos-core.md` y `ui/src/core-api.ts`.

use merken_core::time::Timestamp;
use merken_store::{
    Scenario, Store, StoreError,
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
        .setup(|app| {
            let store = open_store(app)?;
            app.manage(AppState(Mutex::new(store)));
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
            sim_import
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
}
