// SPEC: embedded-runtime (EMBED-06), self-contained-runtime (SELF-01, SELF-18),
//       conversation-memory (MEM-14, MEM-17)

mod chat;
mod chat_commands;
mod commands;
mod config;
mod config_commands;
mod db;
mod document_commands;
mod runtime_commands;
mod models;
mod providers;
mod rag;
mod runtime;
mod system_info;
mod update;
mod update_commands;

use db::DbState;
use runtime::process::SidecarState;
use std::sync::Mutex;
use tauri::{Emitter, Manager, RunEvent};

/// Starts the sidecar at boot when it was already set up, so the user doesn't have to re-click anything after a
/// restart (EMBED-06). Any failure here is logged and ignored: the app must
/// still open, with the connection simply reporting unavailable.
fn autostart_sidecar(app: &tauri::AppHandle) {
    let row = {
        let db = app.state::<DbState>();
        let Ok(guard) = db.0.lock() else { return };
        let Some(sql) = guard.as_ref() else { return };
        let Ok(row) = runtime::store::load(sql) else {
            return;
        };
        row
    };

    // There is no connection to be active any more: the question is simply
    // whether the runtime was installed and a model chosen (SELF-05).
    if !row.is_ready() {
        return;
    }

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        match runtime_commands::start_sidecar_from_row(&handle, &row).await {
            Ok(port) => {
                println!("embedded runtime listening on 127.0.0.1:{port}");
                // The UI read the runtime status while this was still loading
                // its model, so it saw "ready" instead of "running"; without
                // this it would keep showing that until the user hit refresh.
                let _ = handle.emit("runtime-changed", ());
            }
            Err(e) => eprintln!("failed to start embedded runtime: {e}"),
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Removes the executable a previous portable update renamed aside.
            // Runs before anything else so a leftover can never accumulate.
            update_commands::cleanup_after_update();

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
            // One job for the whole process, created before anything can be
            // spawned into it. Its handle closing — which happens however this
            // process ends, including a forced kill — is what makes the kernel
            // take the sidecar down with us.
            app.manage(runtime::job::JobState::create());
            app.manage(chat::cancellation::CancellationRegistry::new());

            if let Ok(Some(cfg)) = config::load_config(app.handle()) {
                // Keeps the embedding model inside the user's chosen folder
                // (AD-008) instead of a hidden per-user cache.
                rag::embedding::set_cache_dir(cfg.base_path_buf().join("models"));

                // The ~150 MB earlier versions downloaded into the base folder
                // is dead weight now that the components ship in the installer
                // (SELF-18). Best-effort: a locked file must not block the boot.
                let removed = runtime::bundled::remove_legacy_downloads(
                    &cfg.base_path_buf().join("runtime"),
                );
                if removed > 0 {
                    println!("runtime: removed {removed} component folder(s) downloaded by an earlier version");
                }
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
            chat_commands::set_chat_use_memory,
            chat_commands::index_chat_history,
            chat_commands::list_chat_attachments,
            config_commands::get_app_config,
            config_commands::get_default_base_path,
            config_commands::get_storage_status,
            config_commands::pick_folder,
            config_commands::complete_onboarding,
            config_commands::update_theme,
            config_commands::update_language,
            config_commands::update_base_path,
            runtime_commands::prepare_runtime,
            runtime_commands::start_runtime,
            runtime_commands::stop_runtime,
            runtime_commands::runtime_status,
            runtime_commands::download_model,
            runtime_commands::list_downloadable_models,
            runtime_commands::list_installed_models,
            runtime_commands::model_limits,
            runtime_commands::get_active_model,
            runtime_commands::set_active_model,
            runtime_commands::configure_model,
            document_commands::import_documents,
            document_commands::list_documents,
            document_commands::delete_document,
            update_commands::check_for_update,
            update_commands::install_update,
            update_commands::skip_update_version,
            update_commands::get_update_settings,
            update_commands::set_auto_update_check,
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
