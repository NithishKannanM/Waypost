pub mod commands;
pub mod db;
pub mod error;
pub mod project_path;
pub mod scanner;
pub mod tmux;

use std::sync::Mutex;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = db::open_default().expect("failed to open cc-console database");

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState { db: Mutex::new(conn) })
        .invoke_handler(tauri::generate_handler![
            commands::refresh,
            commands::list_sessions,
            commands::list_projects,
            commands::start_project_session,
            commands::attach_command,
            commands::resume_command,
            commands::tmux_available,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
