fn main() {
    tauri_plugin::Builder::new(&["start", "finish"])
        .android_path("android")
        .build();
}
