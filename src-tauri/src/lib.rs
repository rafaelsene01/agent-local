mod commands;
mod config;
mod config_commands;
mod connection_commands;
mod connections;
mod db;
mod model_commands;
mod models;
mod providers;
mod system_info;

use db::DbState;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle();
            let existing_conn = match config::load_config(handle) {
                Ok(Some(cfg)) if cfg.onboarding_completed => {
                    let db_file = config::db_path(&cfg.base_path_buf());
                    match db::open(&db_file) {
                        Ok(conn) => Some(conn),
                        Err(e) => {
                            eprintln!("failed to open database at {}: {e}", db_file.display());
                            None
                        }
                    }
                }
                _ => None,
            };
            app.manage(DbState(Mutex::new(existing_conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_chat,
            commands::list_chats,
            commands::rename_chat,
            commands::delete_chat,
            commands::list_messages,
            config_commands::get_app_config,
            config_commands::get_default_base_path,
            config_commands::pick_folder,
            config_commands::complete_onboarding,
            config_commands::update_theme,
            config_commands::update_language,
            config_commands::update_base_path,
            connection_commands::list_connections,
            connection_commands::add_connection,
            connection_commands::toggle_connection,
            connection_commands::refresh_connection_status,
            model_commands::list_downloadable_models,
            model_commands::list_installed_models,
            model_commands::pull_model,
            model_commands::set_active_model,
            model_commands::get_active_model,
            model_commands::configure_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
