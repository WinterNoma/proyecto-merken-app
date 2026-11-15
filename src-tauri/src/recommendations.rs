//! Ground data synchronization is separate from offline model execution.
use crate::{AppState, now, with_store};
use merken_slm::contract::{self, Assessment, Input, Reading};
use merken_store::{StoreError, api::ListRecordsInput};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(target_os = "android")]
use serde_json::json;
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::Manager;

#[derive(Default)]
pub struct RecommendationState {
    busy: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    models: Arc<Mutex<Option<PathBuf>>>,
}
#[derive(Deserialize)]
pub struct GroundLogin {
    username: String,
    password: String,
}
#[derive(Serialize, Deserialize, Clone)]
pub struct BackendCache {
    pub received_at: String,
    pub crew: Vec<Value>,
}
#[derive(Deserialize, Default)]
pub struct RecommendationRequest {
    astronaut_id: Option<String>,
    preferred_activity: Option<String>,
}
#[derive(Serialize)]
pub struct RecommendationResult {
    provider: String,
    generated_at: String,
    source: String,
    core_action: String,
    snapshot_status: String,
    readings: Vec<Reading>,
    recommendation: contract::Output,
    elapsed_ms: u128,
    clinical_validation: bool,
    context_note: String,
}
fn err(message: impl Into<String>) -> StoreError {
    StoreError {
        code: "recommendations".into(),
        field: None,
        message: message.into(),
    }
}
fn cache_path(app: &tauri::AppHandle) -> Result<PathBuf, StoreError> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|_| err("Device storage is unavailable"))?
        .join("ground-snapshot.json"))
}
#[tauri::command]
pub fn recommendations_cached_crew(
    app: tauri::AppHandle,
) -> Result<Option<BackendCache>, StoreError> {
    let path = cache_path(&app)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|_| err("Cached backend snapshot cannot be read"))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| err("Cached backend snapshot is invalid"))
}
#[tauri::command]
pub async fn recommendations_sync_backend(
    app: tauri::AppHandle,
    input: GroundLogin,
) -> Result<BackendCache, StoreError> {
    if input.username.is_empty() || input.username.len() > 100 || input.password.len() > 512 {
        return Err(err("Enter your ground-station login"));
    }
    // Fixed HTTPS origin: no arbitrary host, embedded credentials or inference API.
    let origin = "https://deployconmerken.gabogrobier.dev";
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| err("Secure connection could not initialize"))?;
    let login = client
        .post(format!("{origin}/api/demo-session"))
        .form(&[("username", input.username), ("password", input.password)])
        .send()
        .await
        .map_err(|_| err("Ground station is unreachable; cached data remains available"))?;
    let cookie = login
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .find_map(|s| {
            s.split(';')
                .next()
                .filter(|s| s.starts_with("orbit_demo_session=") && s.len() > 30)
        })
        .ok_or_else(|| err("Ground-station sign-in failed"))?
        .to_owned();
    let response = client
        .get(format!("{origin}/api/astronauts"))
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .map_err(|_| err("Backend snapshot could not be downloaded"))?;
    if !response.status().is_success() {
        return Err(err("Backend snapshot was not authorized"));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| err("Backend snapshot download failed"))?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(err("Backend snapshot exceeds the device limit"));
    }
    let mut crew: Vec<Value> =
        serde_json::from_slice(&bytes).map_err(|_| err("Backend returned an invalid snapshot"))?;
    if crew.is_empty()
        || crew.len() > 100
        || crew
            .iter()
            .any(|p| !p.get("id").is_some_and(Value::is_number))
    {
        return Err(err("Backend returned an invalid roster"));
    }
    for row in &mut crew {
        if row.get("garminLinked").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let id = row
            .get("id")
            .ok_or_else(|| err("Missing backend ID"))?
            .to_string();
        let context_response = client
            .get(format!("{origin}/api/astronauts/{id}/recommendations"))
            .header(reqwest::header::COOKIE, &cookie)
            .send()
            .await
            .map_err(|_| {
                err("Core context could not be synchronized; previous snapshot remains available")
            })?;
        if !context_response.status().is_success() {
            return Err(err(
                "Core context is unavailable; previous snapshot remains available",
            ));
        }
        let context: Value = context_response
            .json()
            .await
            .map_err(|_| err("Invalid core context"))?;
        row.as_object_mut()
            .ok_or_else(|| err("Invalid backend row"))?
            .insert("analysisContext".into(), context);
    }
    let cache = BackendCache {
        received_at: now().to_rfc3339(),
        crew,
    };
    let path = cache_path(&app)?;
    let pending = path.with_extension("pending");
    std::fs::write(
        &pending,
        serde_json::to_vec(&cache).map_err(|_| err("Snapshot serialization failed"))?,
    )
    .map_err(|_| err("Backend snapshot could not be saved"))?;
    std::fs::rename(pending, path).map_err(|_| err("Backend snapshot could not be saved"))?;
    // Credentials and session cookie are dropped; they are never persisted.
    Ok(cache)
}
#[tauri::command]
pub fn recommendations_cancel(state: tauri::State<'_, RecommendationState>) {
    state.cancel.store(true, Ordering::Relaxed);
}

fn backend_input(row: &Value, preference: Option<String>) -> Result<Input, StoreError> {
    if row.get("garminLinked").and_then(Value::as_bool) != Some(true) {
        return Err(err(
            "This backend record has unverified provenance outside the current V7 source contract. Use the local profile or a Garmin-linked backend record.",
        ));
    }
    let fields = [
        ("heart_rate", "heartRate", "bpm"),
        ("sleep", "sleepHours", "h"),
        ("hrv", "hrv", "ms"),
        ("body_battery", "bodyBattery", "score_0_100"),
    ];
    let readings = fields
        .into_iter()
        .map(|(metric, field, unit)| Reading {
            metric: metric.into(),
            value: row.get(field).and_then(Value::as_f64),
            unit: unit.into(),
            measured_at: None,
        })
        .collect();
    let action = row
        .get("analysisContext")
        .and_then(|c| c.get("coreAction"))
        .and_then(Value::as_str)
        .filter(|a| ["NOT_ASSESSED", "REVIEW_MEDICAL", "IMMEDIATE_PROTOCOL"].contains(a))
        .ok_or_else(|| err("Backend core action is missing; synchronize a complete snapshot"))?;
    Ok(Input {
        version: "wellbeing-en-v7".into(),
        source: "GARMIN_BACKEND_SAMPLE".into(),
        readings,
        assessment: Assessment {
            quality: "UNKNOWN".into(),
            freshness: "UNKNOWN".into(),
            core_action: action.into(),
            exercise_clearance: "UNKNOWN".into(),
        },
        assigned_plan: vec![],
        equipment: vec![],
        preferred_activity: preference,
    })
}
fn local_snapshot(
    state: &tauri::State<'_, AppState>,
    preference: Option<String>,
) -> Result<(Input, String), StoreError> {
    with_store(state, |store| {
        let status = store.status(now(), env!("CARGO_PKG_VERSION"))?;
        let records = store.list_records(ListRecordsInput {
            limit: Some(200),
            ..Default::default()
        })?;
        let mut readings = Vec::new();
        let mut origins = std::collections::BTreeSet::new();
        for e in &status.indicators {
            let Some(value) = &e.latest_value else {
                continue;
            };
            if let Some(record) = records
                .iter()
                .find(|r| e.input_record_ids.contains(&r.record_id))
            {
                origins.insert(format!("{:?}", record.source));
            }
            let fields: Vec<(&str, &str, &str)> = match e.indicator.as_str() {
                "heart_rate" => vec![("heart_rate", "bpm", "bpm")],
                "sleep" => vec![("sleep", "hours", "h")],
                "glucose" => vec![("glucose", "mg_dl", "mg/dL")],
                "body_temperature" => vec![("body_temperature", "celsius", "°C")],
                "oxygen_in_blood" => vec![("oxygen_in_blood", "percentage", "%")],
                "mood" => vec![("mood", "score", "score_0_10")],
                "blood_pressure" => vec![
                    ("blood_pressure_systolic", "systolic", "mmHg"),
                    ("blood_pressure_diastolic", "diastolic", "mmHg"),
                ],
                _ => vec![],
            };
            for (metric, field, unit) in fields {
                readings.push(Reading {
                    metric: metric.into(),
                    value: value.get(field).and_then(Value::as_f64),
                    unit: unit.into(),
                    measured_at: e.latest_measured_at.map(|t| t.to_rfc3339()),
                });
            }
        }
        if readings.is_empty() {
            return Err(err(
                "No local measurements yet. Existing core alerts remain authoritative. Record a measurement or synchronize a backend snapshot.",
            ));
        }
        if !status.is_demo && origins.is_empty() {
            return Err(err(
                "Measurement provenance could not be established from the local records",
            ));
        }
        let source = if status.is_demo {
            "SIMULATED"
        } else if origins.iter().all(|s| s == "Garmin") {
            "GARMIN_BACKEND_SAMPLE"
        } else if origins.contains("Garmin") {
            return Err(err(
                "Mixed Garmin/manual origins need separate review; no model source substitution was performed.",
            ));
        } else {
            "MANUAL"
        };
        use merken_core::evaluate::Escalation;
        let core_action = match status.overall.escalation {
            Escalation::Urgent => "IMMEDIATE_PROTOCOL",
            Escalation::FlagForGround => "REVIEW_MEDICAL",
            _ => "NOT_ASSESSED",
        };
        let input = Input {
            version: "wellbeing-en-v7".into(),
            source: source.into(),
            readings,
            assessment: Assessment {
                quality: "UNKNOWN".into(),
                freshness: "UNKNOWN".into(),
                core_action: core_action.into(),
                exercise_clearance: "UNKNOWN".into(),
            },
            assigned_plan: vec![],
            equipment: vec![],
            preferred_activity: preference,
        };
        Ok((input, format!("{:?}", status.overall.status)))
    })
}
#[derive(Deserialize)]
#[cfg(target_os = "android")]
struct Prepared {
    directory: String,
}
pub(crate) fn directory(
    app: &tauri::AppHandle,
    cache: &Mutex<Option<PathBuf>>,
) -> Result<PathBuf, String> {
    let mut cached = cache
        .lock()
        .map_err(|_| "Model preparation is unavailable")?;
    if let Some(path) = cached.as_ref() {
        return Ok(path.clone());
    }
    #[cfg(target_os = "android")]
    let path = {
        let result: Prepared = app
            .state::<tauri_plugin_slm_assets::SlmAssets<tauri::Wry>>()
            .0
            .run_mobile_plugin("prepare", json!({}))
            .map_err(|_| "Bundled model preparation failed; check free storage")?;
        PathBuf::from(result.directory)
    };
    #[cfg(not(target_os = "android"))]
    let path = {
        let _ = app;
        PathBuf::from(
            std::env::var("MERKEN_SLM_MODEL_DIR")
                .map_err(|_| "Set MERKEN_SLM_MODEL_DIR to the verified offline bundle")?,
        )
    };
    merken_slm::runtime::verify(&path)?;
    *cached = Some(path.clone());
    Ok(path)
}
/// Copia y verifica una sola vez los modelos incluidos en la APK (base + V7).
/// El servicio SLM la llama antes de cargar la base desde la misma carpeta.
#[cfg(target_os = "android")]
pub(crate) fn ensure_models(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let state = app.state::<RecommendationState>();
    let cache = Arc::clone(&state.models);
    directory(app, &cache)
}

/// Ruta de la carpeta de modelos en Android, sin copiar nada (raíz del servicio SLM).
#[cfg(target_os = "android")]
pub(crate) fn android_model_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let result: Prepared = app
        .state::<tauri_plugin_slm_assets::SlmAssets<tauri::Wry>>()
        .0
        .run_mobile_plugin("directory", json!({}))
        .map_err(|_| "Model directory is unavailable")?;
    Ok(PathBuf::from(result.directory))
}

/// Un solo modelo en memoria: descarga el del servicio SLM (v5) antes de cargar V7.
/// Si el servicio está ocupado se rechaza la recomendación en vez de cargar 2 copias.
fn release_service_model(app: &tauri::AppHandle) -> Result<(), String> {
    use merken_slm::service::{default_deadline, wait_limit};
    let service = app.state::<crate::SlmState>().service.clone();
    let deadline = default_deadline();
    service
        .release(deadline.clone())
        .map_err(|_| "The assistant is busy; try again when it finishes")?
        .recv_timeout(wait_limit(&deadline))
        .map_err(|_| "The assistant model could not be released")?;
    Ok(())
}

pub(crate) fn recommendation_busy(app: &tauri::AppHandle) -> bool {
    app.state::<RecommendationState>()
        .busy
        .load(Ordering::SeqCst)
}

#[tauri::command]
pub async fn recommendations_generate(
    app: tauri::AppHandle,
    store: tauri::State<'_, AppState>,
    state: tauri::State<'_, RecommendationState>,
    input: RecommendationRequest,
) -> Result<RecommendationResult, StoreError> {
    if input
        .preferred_activity
        .as_deref()
        .is_some_and(|p| !["READING", "MUSIC", "PUZZLE", "JOURNAL"].contains(&p))
    {
        return Err(err("Unsupported activity preference"));
    }
    let (packet, status, note) = if let Some(id) = input.astronaut_id {
        let cache = recommendations_cached_crew(app.clone())?.ok_or_else(|| {
            err("Synchronize the backend once; its snapshot can then be used offline")
        })?;
        let row = cache
            .crew
            .iter()
            .find(|r| r.get("id").map(Value::to_string).as_deref() == Some(id.as_str()))
            .ok_or_else(|| err("Selected astronaut is not in the saved backend snapshot"))?;
        (
            backend_input(row, input.preferred_activity)?,
            "Not evaluated".into(),
            format!(
                "Backend replay saved {}. The SLM uses four Garmin-linked fields with unverified per-field provenance. Sync time is not measurement time. No onboard evaluation is claimed for a different crew profile.",
                cache.received_at
            ),
        )
    } else {
        let (packet, status) = local_snapshot(&store, input.preferred_activity)?;
        (packet,status,"On-device profile and independent core evaluation. Unknown signal quality does not establish exercise readiness.".into())
    };
    contract::normalize(&packet).map_err(err)?;
    if state.busy.swap(true, Ordering::SeqCst) {
        return Err(err("A recommendation is already running"));
    }
    state.cancel.store(false, Ordering::Relaxed);
    let busy = state.busy.clone();
    let cancel = state.cancel.clone();
    let model_cache = state.models.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| {
            let urgent = packet.assessment.core_action == "IMMEDIATE_PROTOCOL";
            let (recommendation, elapsed) = if urgent {
                (contract::expected(&packet).map_err(err)?, 0)
            } else {
                let path = directory(&app, &model_cache).map_err(err)?;
                release_service_model(&app).map_err(err)?;
                let (response, _, elapsed) =
                    merken_slm::runtime::generate(&path, &packet, &cancel).map_err(err)?;
                (response, elapsed)
            };
            Ok(RecommendationResult {
                provider: if urgent { "CORE_POLICY" } else { "SLM_OFFLINE" }.into(),
                generated_at: now().to_rfc3339(),
                source: packet.source,
                core_action: packet.assessment.core_action,
                snapshot_status: status,
                readings: packet.readings,
                recommendation,
                elapsed_ms: elapsed,
                clinical_validation: false,
                context_note: note,
            })
        })();
        busy.store(false, Ordering::SeqCst);
        result
    })
    .await
    .map_err(|_| err("Offline generation was interrupted; core alerts remain available"))?
}
