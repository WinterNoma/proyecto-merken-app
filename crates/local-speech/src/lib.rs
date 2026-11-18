use serde_json::Value;
use tauri::{AppHandle, Runtime, ipc::Channel, plugin::TauriPlugin};

#[cfg(target_os = "android")]
use tauri::{Manager, plugin::PluginHandle};

#[cfg(target_os = "android")]
struct Speech<R: Runtime>(PluginHandle<R>);

#[tauri::command]
async fn start<R: Runtime>(
    app: AppHandle<R>,
    session: String,
    language: String,
    events: Channel<Value>,
) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        let payload =
            serde_json::json!({ "session": session, "language": language, "events": &events });
        let result = app
            .state::<Speech<R>>()
            .0
            .run_mobile_plugin_async("start", payload)
            .await
            .map_err(|error| error.to_string());
        // Keep the webview callback alive through the final native response.
        drop(events);
        result
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, session, language, events);
        Err("On-device dictation requires a supported Android device. You can type instead.".into())
    }
}

#[tauri::command]
async fn finish<R: Runtime>(
    app: AppHandle<R>,
    session: String,
    discard: bool,
) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let payload = serde_json::json!({ "session": session, "discard": discard });
        app.state::<Speech<R>>()
            .0
            .run_mobile_plugin_async("finish", payload)
            .await
            .map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, session, discard);
        Ok(())
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let builder = tauri::plugin::Builder::new("local-speech")
        .invoke_handler(tauri::generate_handler![start, finish]);
    #[cfg(target_os = "android")]
    let builder = builder.setup(|app, api| {
        let native = api.register_android_plugin("com.pulsys.speech", "LocalSpeechPlugin")?;
        app.manage(Speech(native));
        Ok(())
    });
    builder.build()
}
