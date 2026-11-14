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
        CorrectRecordInput, EventItem, ListRecordsInput, LogRecordInput, LogResult,
        ProfileResponse, RaiseEventInput, RaiseEventResult, RecordItem, StatusResponse,
    },
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

// ---------- Lógica sin Tauri (se puede probar) ----------

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
            clear_demo
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
