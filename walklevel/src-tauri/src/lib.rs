//! Aplikacja WalkLevel: okno Tauri + rdzeń (crate `core`). Tu tylko klej: komendy dla UI,
//! zdarzenia ze stanem, okna (główne, popup nad grą, pływający przycisk).

#[tauri::command]
fn app_version() -> String {
    walklevel_core::version().to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("nie udało się uruchomić okna WalkLevel");
}
