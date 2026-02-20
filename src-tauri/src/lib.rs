mod ai;
mod fs_commands;
mod terminal;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let terminal_state = terminal::create_terminal_state();
    let project_root_state = fs_commands::create_project_root_state();
    let api_key_state = ai::create_api_key_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_drag::init())
        .manage(terminal_state)
        .manage(project_root_state)
        .manage(api_key_state)
        .invoke_handler(tauri::generate_handler![
            fs_commands::set_project_root,
            fs_commands::read_dir_tree,
            fs_commands::read_file_content,
            fs_commands::write_file_content,
            fs_commands::read_file_binary,
            fs_commands::get_home_dir,
            fs_commands::create_file,
            fs_commands::create_folder,
            fs_commands::delete_entries,
            fs_commands::rename_entry,
            fs_commands::move_entries,
            fs_commands::import_external_files,
            fs_commands::paste_entries,
            fs_commands::get_git_status,
            fs_commands::get_git_ignored,
            fs_commands::list_all_files,
            fs_commands::get_git_branch,
            fs_commands::git_diff,
            fs_commands::git_stage,
            fs_commands::git_unstage,
            fs_commands::git_commit,
            fs_commands::git_push,
            fs_commands::git_ahead_behind,
            fs_commands::git_diff_line_ranges,
            fs_commands::git_log,
            terminal::spawn_terminal,
            terminal::write_terminal,
            terminal::kill_terminal,
            terminal::resize_terminal,
            ai::set_api_key,
            ai::ai_chat,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
