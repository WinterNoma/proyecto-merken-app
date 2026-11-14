//! Capa Tauri: solo une los crates de `crates/` con la interfaz.
//! Ninguna decisión de salud se toma aquí ni en TypeScript.

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![core_status])
        .run(tauri::generate_context!())
        .expect("failed to start the Tauri application");
}
