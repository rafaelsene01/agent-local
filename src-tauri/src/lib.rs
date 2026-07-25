mod chat;
mod chat_commands;
mod commands;
mod config;
mod config_commands;
mod connection_commands;
mod connections;
mod db;
mod document_commands;
mod embedded_commands;
mod model_commands;
mod models;
mod providers;
mod rag;
mod runtime;
mod system_info;

use db::DbState;
use runtime::process::SidecarState;
use std::sync::Mutex;
use tauri::{Manager, RunEvent};

/// Starts the sidecar at boot when it was already set up and its connection
/// is the active one, so the user doesn't have to re-click anything after a
/// restart (EMBED-06). Any failure here is logged and ignored: the app must
/// still open, with the connection simply reporting unavailable.
fn autostart_sidecar(app: &tauri::AppHandle) {
    let (row, is_active) = {
        let db = app.state::<DbState>();
        let Ok(guard) = db.0.lock() else { return };
        let Some(sql) = guard.as_ref() else { return };
        let Ok(row) = runtime::store::load(sql) else {
            return;
        };
        let is_active = connections::active_connection(sql)
            .ok()
            .flatten()
            .is_some_and(|c| c.provider == "embedded");
        (row, is_active)
    };

    if !is_active || !row.is_ready() {
        return;
    }

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        match embedded_commands::start_sidecar_from_row(&handle, &row).await {
            Ok(port) => println!("embedded runtime listening on 127.0.0.1:{port}"),
            Err(e) => eprintln!("failed to start embedded runtime: {e}"),
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
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
            app.manage(SidecarState::empty());
            app.manage(chat::cancellation::CancellationRegistry::new());

            if let Ok(Some(cfg)) = config::load_config(app.handle()) {
                // Keeps the embedding model inside the user's chosen folder
                // (AD-008) instead of a hidden per-user cache.
                rag::embedding::set_cache_dir(cfg.base_path_buf().join("models"));
            }

            autostart_sidecar(app.handle());
            document_commands::requeue_unfinished_documents(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_chat,
            commands::list_chats,
            commands::rename_chat,
            commands::delete_chat,
            commands::list_messages,
            chat_commands::create_message,
            chat_commands::send_message,
            chat_commands::cancel_generation,
            chat_commands::set_chat_use_global_rag,
            config_commands::get_app_config,
            config_commands::get_default_base_path,
            config_commands::pick_folder,
            config_commands::complete_onboarding,
            config_commands::update_theme,
            config_commands::update_language,
            config_commands::update_base_path,
            connection_commands::list_connections,
            connection_commands::add_connection,
            connection_commands::set_active_connection,
            connection_commands::clear_active_connection,
            connection_commands::refresh_connection_status,
            model_commands::list_downloadable_models,
            model_commands::list_installed_models,
            model_commands::pull_model,
            model_commands::set_active_model,
            model_commands::get_active_pair,
            model_commands::configure_model,
            embedded_commands::setup_embedded_runtime,
            embedded_commands::start_embedded_runtime,
            embedded_commands::stop_embedded_runtime,
            embedded_commands::embedded_runtime_status,
            embedded_commands::download_embedded_model,
            document_commands::import_documents,
            document_commands::list_documents,
            document_commands::delete_document,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    // Window events aren't enough to catch every quit path; RunEvent is the
    // reliable hook for killing the child process (EMBED-07).
    app.run(|handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            if let Some(mut sidecar) = handle
                .state::<SidecarState>()
                .0
                .lock()
                .ok()
                .and_then(|mut guard| guard.take())
            {
                sidecar.kill();
            }
        }
    });
}
