//! Every #[tauri::command], thin. TODO.

use tauri::ipc::Invoke;

pub fn handler() -> impl Fn(Invoke<tauri::Wry>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![ping]
}

#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}
