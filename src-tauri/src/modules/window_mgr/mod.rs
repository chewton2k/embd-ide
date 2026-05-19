use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

static NEXT_WINDOW_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(2);

/// Stores the initial project path for newly-created windows.
/// The frontend pulls from this on mount via `get_initial_project`.
pub struct InitialProjectState(pub Mutex<HashMap<String, String>>);

pub fn open_new_window_impl<R: Runtime>(
    app: &AppHandle<R>,
    initial_project: Option<String>,
) -> Result<String, String> {
    let id = NEXT_WINDOW_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let label = format!("win-{}", id);

    // Pre-register the per-window state slot
    {
        let project_state: tauri::State<crate::modules::fs::ProjectRootState> = app.state();
        let mut map = project_state.blocking_write();
        map.insert(label.clone(), None);
    }

    // Store initial project for the frontend to pull on mount
    if let Some(ref project) = initial_project {
        let init_state: tauri::State<InitialProjectState> = app.state();
        let mut map = init_state.0.lock().unwrap_or_else(|e| e.into_inner());
        map.insert(label.clone(), project.clone());
    }

    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("leo")
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()
        .map_err(|e| format!("failed to spawn window: {e}"))?;

    let _ = window;
    Ok(label)
}

/// Called by the frontend on mount to check if this window was opened with a project.
/// Returns and removes the entry (one-shot).
#[tauri::command]
pub fn get_initial_project(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, InitialProjectState>,
) -> Option<String> {
    let mut map = state.0.lock().unwrap_or_else(|e| e.into_inner());
    map.remove(window.label())
}

#[tauri::command]
pub fn open_new_window(app: AppHandle, initial_project: Option<String>) -> Result<String, String> {
    open_new_window_impl(&app, initial_project)
}

#[tauri::command]
pub fn open_folder_in_new_window(app: AppHandle, path: String) -> Result<String, String> {
    open_new_window_impl(&app, Some(path))
}

#[tauri::command]
pub fn close_focused_window(window: tauri::WebviewWindow) -> Result<(), String> {
    window.close().map_err(|e| format!("failed to close window: {e}"))
}
