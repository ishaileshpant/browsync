#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::detect_browsers,
            commands::import_browser,
            commands::import_all,
            commands::search_all,
            commands::get_bookmarks,
            commands::get_history,
            commands::get_stats,
            commands::get_sync_log,
            commands::export_bookmarks,
            commands::open_url,
            commands::get_auth_entries,
            commands::delete_browser_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running browsync");
}
