//! Capa Tauri: solo une los crates de `crates/` con la interfaz.
//! Ninguna decisión de salud se toma aquí ni en TypeScript.
//!
//! Es el único sitio que lee el reloj del sistema: el instante se pasa a
//! `merken-store` y `merken-core` como argumento (RNF-03).
//! Contrato de los comandos: `docs/api/comandos-core.md` y `ui/src/core-api.ts`.

use merken_core::time::Timestamp;
use merken_slm::{
    Ficha,
    service::{
        ExplainResponse, InterpretResponse, ServiceError, SlmService, SlmStatus, default_deadline,
        wait_limit,
    },
};
use merken_store::{
    ListTelemetryInput, RecordTelemetryInput, RecordTelemetryResult, Scenario, Store, StoreError,
    TelemetryItem, VerifyTelemetryResult,
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
use tauri::{Emitter, Manager};

mod strings;
mod voice;
use voice::VoiceState;

/// Versión de la app que viaja en cada registro (`app_version`).
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Base de datos local, compartida por todos los comandos.
pub struct AppState(Mutex<Store>);
pub struct SlmState {
    service: SlmService,
    fichas: Vec<Ficha>,
}

#[derive(Deserialize)]
pub struct InterpretInput {
    pub request_id: String,
    pub text: String,
    pub source: Option<String>,
}
#[derive(Deserialize)]
pub struct ExplainInput {
    pub request_id: String,
    pub indicator: String,
    pub body_zone: Option<merken_core::catalog::BodyZone>,
}

fn slm_error(code: &str, field: Option<&str>, message: &str) -> StoreError {
    StoreError {
        code: code.into(),
        field: field.map(str::to_owned),
        message: message.into(),
    }
}
fn service_error(e: ServiceError) -> StoreError {
    match e {
        ServiceError::Busy => slm_error("busy", None, strings::SLM_BUSY),
        ServiceError::InvalidValue => {
            slm_error("invalid_value", Some("text"), strings::TEXT_TOO_LONG)
        }
        ServiceError::InvalidRequestId => {
            slm_error("invalid_value", Some("request_id"), strings::REQUEST_ID)
        }
        ServiceError::Unavailable => slm_error("slm_error", None, strings::SLM_UNAVAILABLE),
    }
}
fn validate_request_id(id: &str) -> Result<(), StoreError> {
    if merken_slm::service::valid_request_id(id) {
        Ok(())
    } else {
        Err(service_error(ServiceError::InvalidRequestId))
    }
}
pub fn handle_interpret(
    service: &SlmService,
    input: InterpretInput,
) -> Result<InterpretResponse, StoreError> {
    validate_request_id(&input.request_id)?;
    if input
        .source
        .as_deref()
        .is_some_and(|s| s != "text" && s != "voice")
    {
        return Err(slm_error("invalid_value", Some("source"), strings::SOURCE));
    }
    let deadline = default_deadline();
    let pending = service
        .interpret(input.request_id, input.text, deadline.clone())
        .map_err(service_error)?;
    Ok(pending.recv_timeout(wait_limit(&deadline)))
}
pub fn handle_find_evaluation(
    store: &mut Store,
    input: ExplainInput,
    at: Timestamp,
) -> Result<merken_core::evaluate::Evaluation, StoreError> {
    validate_request_id(&input.request_id)?;
    let status = store.status(at, APP_VERSION)?;
    let entries = if input.indicator == "symptom" {
        status.symptoms
    } else {
        status.indicators
    };
    entries
        .into_iter()
        .find(|e| e.indicator == input.indicator && e.body_zone == input.body_zone)
        .ok_or_else(|| slm_error("not_found", Some("indicator"), strings::INDICATOR_NOT_FOUND))
}
pub fn handle_explain(
    service: &SlmService,
    request_id: String,
    eval: merken_core::evaluate::Evaluation,
    fichas: Vec<Ficha>,
) -> Result<ExplainResponse, StoreError> {
    let deadline = default_deadline();
    let pending = service
        .explain(request_id, eval, fichas, deadline.clone())
        .map_err(service_error)?;
    Ok(pending.recv_timeout(wait_limit(&deadline)))
}
async fn slm_blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, StoreError> + Send + 'static,
) -> Result<T, StoreError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| slm_error("slm_error", None, strings::SLM_INTERRUPTED))?
}
#[tauri::command]
async fn interpret_text(
    state: tauri::State<'_, SlmState>,
    input: InterpretInput,
) -> Result<InterpretResponse, StoreError> {
    let service = state.service.clone();
    slm_blocking(move || handle_interpret(&service, input)).await
}
#[tauri::command]
async fn explain_indicator(
    app: tauri::AppHandle,
    state: tauri::State<'_, SlmState>,
    input: ExplainInput,
) -> Result<ExplainResponse, StoreError> {
    let service = state.service.clone();
    let fichas = state.fichas.clone();
    slm_blocking(move || {
        let request_id = input.request_id.clone();
        // Both mutex acquisition and SQLite/evaluation run on the blocking pool.
        let eval = with_store(&app.state::<AppState>(), |s| {
            handle_find_evaluation(s, input, now())
        })?;
        // with_store has released its guard before any wait for the SLM.
        handle_explain(&service, request_id, eval, fichas)
    })
    .await
}
#[tauri::command]
async fn slm_status(state: tauri::State<'_, SlmState>) -> Result<SlmStatus, StoreError> {
    let service = state.service.clone();
    slm_blocking(move || {
        let deadline = default_deadline();
        service
            .status(deadline.clone())
            .map_err(service_error)?
            .recv_timeout(wait_limit(&deadline))
            .map_err(receive_error)
    })
    .await
}
fn receive_error(error: std::sync::mpsc::RecvTimeoutError) -> StoreError {
    match error {
        std::sync::mpsc::RecvTimeoutError::Timeout => {
            slm_error("timeout", None, strings::SLM_TIMEOUT)
        }
        std::sync::mpsc::RecvTimeoutError::Disconnected => {
            slm_error("slm_error", None, strings::SLM_UNAVAILABLE)
        }
    }
}
#[tauri::command(rename_all = "snake_case")]
async fn slm_cancel(
    state: tauri::State<'_, SlmState>,
    request_id: String,
) -> Result<bool, StoreError> {
    validate_request_id(&request_id)?;
    Ok(state.service.cancel(&request_id))
}
#[tauri::command]
async fn slm_release(state: tauri::State<'_, SlmState>) -> Result<bool, StoreError> {
    let service = state.service.clone();
    slm_blocking(move || {
        let deadline = default_deadline();
        service
            .release(deadline.clone())
            .map_err(service_error)?
            .recv_timeout(wait_limit(&deadline))
            .map_err(receive_error)
    })
    .await
}

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
        message: strings::STORAGE_UNAVAILABLE.into(),
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
            message: strings::SIM_INTERRUPTED.into(),
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
        message: strings::UNKNOWN_SCENARIO.into(),
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
                message: strings::ASTRONAUT_NOT_FOUND.into(),
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
            let root = if cfg!(debug_assertions) {
                std::env::var_os("MERKEN_SLM_MODELS_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or(app.path().app_data_dir()?.join("models"))
            } else {
                app.path().app_data_dir()?.join("models")
            };
            #[cfg(feature = "slm-llama")]
            let factory = merken_slm::service::LlamaFactory;
            #[cfg(not(feature = "slm-llama"))]
            let factory = merken_slm::service::NoEngineFactory;
            let handle = app.handle().clone();
            let progress =
                std::sync::Arc::new(move |event: merken_slm::service::ServiceProgress| {
                    let _ = handle.emit("slm-progress", event);
                });
            app.manage(VoiceState::new(root.clone()));
            let (service, fichas) = SlmService::start_with_interpreter(
                root,
                factory,
                Some(progress),
                Some(merken_slm::service::granite_interpreter_manifest()),
            )?;
            app.manage(SlmState { service, fichas });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            voice::speak_text,
            voice::speak_cancel,
            voice::voice_release,
            interpret_text,
            explain_indicator,
            slm_status,
            slm_cancel,
            slm_release,
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
            verify_telemetry
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
    fn slm_handlers_use_local_status_and_validate_input() -> Result<(), StoreError> {
        let (service, fichas) = SlmService::start(
            std::env::temp_dir().join("merken-tauri-slm-test"),
            merken_slm::service::NoEngineFactory,
            None,
        )
        .map_err(|_| slm_error("slm_error", None, "Test setup failed"))?;
        let invalid = handle_interpret(
            &service,
            InterpretInput {
                request_id: "tauri-test".into(),
                text: "a".repeat(merken_core::interpret::MAX_INPUT_CHARS + 1),
                source: None,
            },
        );
        assert!(
            invalid.is_err_and(|e| e.code == "invalid_value" && e.field.as_deref() == Some("text"))
        );
        let nullable: InterpretInput = serde_json::from_value(serde_json::json!({
            "request_id": "nullable", "text": "pulso 72", "source": null,
        }))?;
        assert!(nullable.source.is_none());
        let nullable_explain: ExplainInput = serde_json::from_value(serde_json::json!({
            "request_id": "nullable-explain", "indicator": "heart_rate", "body_zone": null,
        }))?;
        assert!(nullable_explain.body_zone.is_none());
        let invalid_id = handle_interpret(
            &service,
            InterpretInput {
                request_id: "not valid".into(),
                text: "pulso 72".into(),
                source: None,
            },
        );
        assert!(
            invalid_id.is_err_and(
                |e| e.code == "invalid_value" && e.field.as_deref() == Some("request_id")
            )
        );
        let mut store = Store::open_in_memory()?;
        let evaluation = handle_find_evaluation(
            &mut store,
            ExplainInput {
                request_id: "tauri-test".into(),
                indicator: "heart_rate".into(),
                body_zone: None,
            },
            Timestamp::from_millis(0),
        )?;
        let explanation = handle_explain(&service, "tauri-explain".into(), evaluation, fichas)?;
        assert_eq!(explanation.note_status, "no_fichas");
        assert!(!explanation.facts.is_empty());
        let missing = handle_find_evaluation(
            &mut store,
            ExplainInput {
                request_id: "tauri-test".into(),
                indicator: "unknown".into(),
                body_zone: None,
            },
            Timestamp::from_millis(0),
        );
        assert!(missing.is_err_and(|e| e.code == "not_found"));
        Ok(())
    }

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
}
