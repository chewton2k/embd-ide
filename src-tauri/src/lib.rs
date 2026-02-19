mod ai;
mod fs_commands;
mod terminal;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let terminal_state = terminal::create_terminal_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(terminal_state)
        .invoke_handler(tauri::generate_handler![
            fs_commands::read_dir_tree,
            fs_commands::read_file_content,
            fs_commands::write_file_content,
            fs_commands::read_file_binary,
            fs_commands::get_home_dir,
            fs_commands::create_file,
            fs_commands::create_folder,
            fs_commands::delete_entries,
            fs_commands::rename_entry,
            fs_commands::paste_entries,
            fs_commands::get_git_status,
            fs_commands::list_all_files,
            fs_commands::get_git_branch,
            terminal::spawn_terminal,
            terminal::write_terminal,
            terminal::kill_terminal,
            terminal::resize_terminal,
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
