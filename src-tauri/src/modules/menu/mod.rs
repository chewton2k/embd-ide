use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    AppHandle, Emitter, Manager, Runtime,
};

/// Emit an event to the focused window only. Falls back to global emit
/// if no window is focused (e.g., all windows minimized).
fn emit_to_focused<R: Runtime>(app: &AppHandle<R>, event: &str) {
    // Find the focused webview window by checking is_focused() on each
    for (_, win) in app.webview_windows() {
        if win.is_focused().unwrap_or(false) {
            let _ = win.emit(event, ());
            return;
        }
    }
    // Fallback: emit globally (handles case where no window reports focus)
    let _ = app.emit(event, ());
}

pub fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    // ── leo (app) submenu ──
    let app_submenu = SubmenuBuilder::new(app, "leo")
        .item(&PredefinedMenuItem::about(app, Some("About leo"), None)?)
        .separator()
        .item(&MenuItemBuilder::with_id("settings", "Settings…").accelerator("CmdOrCtrl+,").build(app)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, Some("Hide leo"))?)
        .item(&PredefinedMenuItem::hide_others(app, Some("Hide Others"))?)
        .item(&PredefinedMenuItem::show_all(app, Some("Show All"))?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, Some("Quit leo"))?)
        .build()?;

    // ── File submenu ──
    let file_submenu = SubmenuBuilder::new(app, "File")
        .item(&MenuItemBuilder::with_id("new_window", "New Window").accelerator("CmdOrCtrl+Shift+N").build(app)?)
        .item(&MenuItemBuilder::with_id("new_file", "New File").accelerator("CmdOrCtrl+N").build(app)?)
        .item(&MenuItemBuilder::with_id("open_folder", "Open Folder…").accelerator("CmdOrCtrl+O").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("save", "Save").accelerator("CmdOrCtrl+S").build(app)?)
        .item(&MenuItemBuilder::with_id("save_all", "Save All").accelerator("CmdOrCtrl+Alt+S").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("close_tab", "Close Tab").accelerator("CmdOrCtrl+W").build(app)?)
        .item(&MenuItemBuilder::with_id("close_window", "Close Window").accelerator("CmdOrCtrl+Shift+W").build(app)?)
        .item(&MenuItemBuilder::with_id("revert_file", "Revert File").build(app)?)
        .build()?;

    // ── Edit submenu ──
    let edit_submenu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, Some("Undo"))?)
        .item(&PredefinedMenuItem::redo(app, Some("Redo"))?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, Some("Cut"))?)
        .item(&PredefinedMenuItem::copy(app, Some("Copy"))?)
        .item(&PredefinedMenuItem::paste(app, Some("Paste"))?)
        .separator()
        .item(&MenuItemBuilder::with_id("find", "Find").accelerator("CmdOrCtrl+F").build(app)?)
        .item(&MenuItemBuilder::with_id("replace", "Replace").accelerator("CmdOrCtrl+Alt+F").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("undo_ai_edit", "Undo Last AI Edit").accelerator("Ctrl+CmdOrCtrl+Z").build(app)?)
        .item(&MenuItemBuilder::with_id("toggle_comment", "Toggle Comment").accelerator("CmdOrCtrl+/").build(app)?)
        .item(&MenuItemBuilder::with_id("indent", "Indent").accelerator("CmdOrCtrl+]").build(app)?)
        .item(&MenuItemBuilder::with_id("outdent", "Outdent").accelerator("CmdOrCtrl+[").build(app)?)
        .build()?;

    // ── Selection submenu ──
    let selection_submenu = SubmenuBuilder::new(app, "Selection")
        .item(&PredefinedMenuItem::select_all(app, Some("Select All"))?)
        .build()?;

    // ── View submenu ──
    let mut view_builder = SubmenuBuilder::new(app, "View")
        .item(&MenuItemBuilder::with_id("toggle_file_tree", "Toggle File Tree").build(app)?)
        .item(&MenuItemBuilder::with_id("toggle_ai_panel", "Toggle AI Panel").build(app)?)
        .item(&MenuItemBuilder::with_id("toggle_terminal", "Toggle Terminal").build(app)?)
        .item(&MenuItemBuilder::with_id("toggle_sidebar", "Toggle Sidebar").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("toggle_fullscreen", "Toggle Fullscreen").build(app)?)
        .item(&MenuItemBuilder::with_id("reload", "Reload").accelerator("CmdOrCtrl+R").build(app)?);

    if cfg!(debug_assertions) {
        view_builder = view_builder
            .item(&MenuItemBuilder::with_id("toggle_devtools", "Toggle DevTools").accelerator("CmdOrCtrl+Alt+I").build(app)?);
    }
    let view_submenu = view_builder.build()?;

    // ── Go submenu ──
    let go_submenu = SubmenuBuilder::new(app, "Go")
        .item(&MenuItemBuilder::with_id("go_to_file", "Go to File").accelerator("CmdOrCtrl+P").build(app)?)
        .item(&MenuItemBuilder::with_id("go_to_line", "Go to Line").accelerator("Ctrl+G").build(app)?)
        .item(&MenuItemBuilder::with_id("go_to_symbol", "Go to Symbol").accelerator("CmdOrCtrl+Shift+O").build(app)?)
        .separator()
        .item(&MenuItemBuilder::with_id("back", "Back").accelerator("Ctrl+-").build(app)?)
        .item(&MenuItemBuilder::with_id("forward", "Forward").accelerator("Ctrl+Shift+-").build(app)?)
        .build()?;

    // ── Window submenu ──
    let window_submenu = SubmenuBuilder::new(app, "Window")
        .item(&PredefinedMenuItem::minimize(app, Some("Minimize"))?)
        .item(&PredefinedMenuItem::maximize(app, Some("Zoom"))?)
        .build()?;

    // ── Help submenu ──
    let help_submenu = SubmenuBuilder::new(app, "Help")
        .item(&MenuItemBuilder::with_id("documentation", "Documentation").build(app)?)
        .item(&MenuItemBuilder::with_id("report_issue", "Report Issue").build(app)?)
        .build()?;

    MenuBuilder::new(app)
        .item(&app_submenu)
        .item(&file_submenu)
        .item(&edit_submenu)
        .item(&selection_submenu)
        .item(&view_submenu)
        .item(&go_submenu)
        .item(&window_submenu)
        .item(&help_submenu)
        .build()
}

pub fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event_id: &str) {
    match event_id {
        "settings" => { emit_to_focused(app, "menu:open-settings"); }
        "new_window" => { let _ = crate::modules::window_mgr::open_new_window_impl(app, None); }
        "new_file" => { emit_to_focused(app, "menu:new-file"); }
        "open_folder" => { emit_to_focused(app, "menu:open-folder"); }
        "save" => { emit_to_focused(app, "menu:save"); }
        "save_all" => { emit_to_focused(app, "menu:save-all"); }
        "close_tab" => { emit_to_focused(app, "menu:close-tab"); }
        "close_window" => { emit_to_focused(app, "menu:close-window"); }
        "revert_file" => { emit_to_focused(app, "menu:revert-file"); }
        "find" => { emit_to_focused(app, "menu:find"); }
        "replace" => { emit_to_focused(app, "menu:replace"); }
        "undo_ai_edit" => { emit_to_focused(app, "menu:undo-last-ai-edit"); }
        "toggle_comment" => { emit_to_focused(app, "menu:toggle-comment"); }
        "indent" => { emit_to_focused(app, "menu:indent"); }
        "outdent" => { emit_to_focused(app, "menu:outdent"); }
        "toggle_file_tree" => { emit_to_focused(app, "menu:toggle-file-tree"); }
        "toggle_ai_panel" => { emit_to_focused(app, "menu:toggle-ai-panel"); }
        "toggle_terminal" => { emit_to_focused(app, "menu:toggle-terminal"); }
        "toggle_sidebar" => { emit_to_focused(app, "menu:toggle-sidebar"); }
        "toggle_fullscreen" => { emit_to_focused(app, "menu:toggle-fullscreen"); }
        "reload" => { emit_to_focused(app, "menu:reload"); }
        "toggle_devtools" => { emit_to_focused(app, "menu:toggle-devtools"); }
        "go_to_file" => { emit_to_focused(app, "menu:go-to-file"); }
        "go_to_line" => { emit_to_focused(app, "menu:go-to-line"); }
        "go_to_symbol" => { emit_to_focused(app, "menu:go-to-symbol"); }
        "back" => { emit_to_focused(app, "menu:back"); }
        "forward" => { emit_to_focused(app, "menu:forward"); }
        "documentation" => { emit_to_focused(app, "menu:documentation"); }
        "report_issue" => { emit_to_focused(app, "menu:report-issue"); }
        _ => {}
    }
}
