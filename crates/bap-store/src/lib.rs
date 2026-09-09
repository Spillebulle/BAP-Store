//! The Tauri application. `run` builds the window; `cli` is the text mode.

pub mod cli;
pub mod commands;
pub mod settings;
pub mod state;

pub fn run() {
    // WebKitGTK renders black on some NVIDIA drivers when it uses DMA-BUF
    // buffers; the environment variable is WebKit's own switch for it. Set
    // before the webview exists, only where an NVIDIA device is present, and
    // never overriding a value the user chose.
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERING").is_none()
        && bap_core::system::has_nvidia()
    {
        // SAFETY: called before any other thread exists.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERING", "1") };
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .manage(state::AppState::new())
        .invoke_handler(commands::handler())
        .run(tauri::generate_context!())
        .expect("error while running BAP Store");
}
