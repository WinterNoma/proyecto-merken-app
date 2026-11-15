use tauri::{
    Runtime,
    plugin::{Builder, TauriPlugin},
};
#[cfg(target_os = "android")]
pub struct SlmAssets<R: Runtime>(pub tauri::plugin::PluginHandle<R>);
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("slm-assets")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle =
                    api.register_android_plugin("com.merken.slmassets", "SlmAssetsPlugin")?;
                app.manage(SlmAssets(handle));
            }
            #[cfg(not(target_os = "android"))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}
