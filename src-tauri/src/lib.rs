pub mod modules;

use modules::{ai, fs, git, graph, knowledge, log as app_log, menu, session, shell, symbols, window_mgr};
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let terminal_state = shell::create_terminal_state();
    let project_root_state = fs::create_project_root_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .manage(terminal_state)
        .manage(project_root_state)
        .manage(Arc::new(ai::AiState::new()))
        .manage(Arc::new(knowledge::KnowledgeState::new()))
        .manage(app_log::LogState::new())
        .manage(session::AppStateHandle(std::sync::Mutex::new(
            session::AppState::default(),
        )))
        .manage(window_mgr::InitialProjectState(std::sync::Mutex::new(
            std::collections::HashMap::new(),
        )))
        .invoke_handler(tauri::generate_handler![
            // Logging
            app_log::log_record,
            // File system
            fs::set_project_root,
            fs::read_dir_tree,
            fs::read_file_content,
            fs::write_file_content,
            fs::read_file_binary,
            fs::get_home_dir,
            fs::create_project_dir,
            fs::create_file,
            fs::create_folder,
            fs::delete_entries,
            fs::rename_entry,
            fs::move_entries,
            fs::import_external_files,
            fs::paste_entries,
            fs::duplicate_entry,
            fs::reveal_in_file_manager,
            fs::list_all_files,
            // Git
            git::get_git_status,
            git::get_git_remote_status,
            git::get_git_ignored,
            git::get_git_branch,
            git::git_diff,
            git::git_stage,
            git::git_unstage,
            git::git_discard,
            git::git_commit,
            git::git_push,
            git::git_fetch,
            git::git_pull,
            git::git_pull_rebase,
            git::git_delete_branch,
            git::git_ahead_behind,
            git::git_diff_line_ranges,
            git::git_log,
            git::git_list_branches,
            git::git_checkout_branch,
            git::git_resolve_conflict,
            git::git_create_checkpoint,
            git::git_restore_checkpoint,
            git::git_list_checkpoints,
            git::git_clone,
            git::find_git_repos,
            // Shell
            shell::spawn_terminal,
            shell::write_terminal,
            shell::kill_terminal,
            shell::resize_terminal,
            shell::run_command_capture,
            // AI
            ai::set_api_key,
            ai::set_provider_key,
            ai::get_provider_key,
            ai::ai_chat,
            ai::ai_chat_stream,
            ai::ai_chat_cancel,
            // Session
            session::get_recent_projects,
            session::save_session,
            session::remove_recent_project,
            // Graph
            graph::analyze_file_graph,
            // Knowledge
            knowledge::knowledge_init,
            knowledge::knowledge_index,
            knowledge::knowledge_get_context,
            knowledge::knowledge_save_conversation,
            knowledge::knowledge_list_conversations,
            knowledge::knowledge_load_conversation,
            knowledge::knowledge_delete_conversations,
            knowledge::knowledge_delete_conversation,
            knowledge::knowledge_list_projects,
            knowledge::knowledge_delete_project,
            knowledge::knowledge_delete_by_hash,
            knowledge::knowledge_delete_all_projects,
            // Symbols
            symbols::symbols_extract,
            symbols::symbols_get_body,
            // Window management
            window_mgr::open_new_window,
            window_mgr::open_folder_in_new_window,
            window_mgr::close_focused_window,
            window_mgr::get_initial_project,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            // One-shot: re-key any plaintext keys.json from older builds
            // into the encrypted store. No-op when there's nothing to
            // migrate. Set LEO_DISABLE_KEY_MIGRATION=1 to skip.
            ai::migrate_plaintext_keys();
            let loaded = session::load_state_from_disk(app.handle()).unwrap_or_default();
            let handle = app.state::<session::AppStateHandle>();
            let mut guard = handle
                .0
                .lock()
                .map_err(|e| format!("failed to lock app state during setup: {e}"))?;
            *guard = loaded;

            // Register the main window's slot in the per-window state map
            {
                let state: tauri::State<fs::ProjectRootState> = app.state();
                let mut map = state.blocking_write();
                map.insert("main".to_string(), None);
            }

            // Build and attach the native menu
            let menu = menu::build_menu(app.handle())?;
            app.set_menu(menu)?;
            app.on_menu_event(|app, event| {
                menu::handle_menu_event(app, event.id().as_ref());
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let label = window.label().to_string();
                // Remove project root entry for this window
                if let Some(state) = window.try_state::<fs::ProjectRootState>() {
                    let mut map = state.blocking_write();
                    map.remove(&label);
                }
                // Kill terminals for this window
                if let Some(state) = window.try_state::<shell::TerminalState>() {
                    if let Ok(mut managers) = state.lock() {
                        if let Some(mut manager) = managers.remove(&label) {
                            manager.kill_all();
                        }
                    }
                }
                // Drop cached knowledge DB connection for this window
                if let Some(state) = window.try_state::<Arc<knowledge::KnowledgeState>>() {
                    state.remove_window(&label);
                }
                // Remove any unclaimed initial project entry
                if let Some(state) = window.try_state::<window_mgr::InitialProjectState>() {
                    if let Ok(mut map) = state.0.lock() {
                        map.remove(&label);
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
