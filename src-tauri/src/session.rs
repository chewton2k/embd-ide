use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    pub path: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub open_files: Vec<SessionFile>,
    pub active_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    pub path: String,
    pub name: String,
    pub last_opened: u64,
    pub session: SessionData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub recent_projects: Vec<RecentProject>,
}

pub struct AppStateHandle(pub Mutex<AppState>);

const MAX_SESSION_FILES: usize = 20;

fn state_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app_data_dir: {e}"))?;
    Ok(dir.join("state.json"))
}

/// Validate that a project path is absolute and doesn't contain traversal sequences.
fn validate_path(path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return Err("project path must be absolute".into());
    }
    // Reject path components that are ".."
    for component in p.components() {
        if let std::path::Component::ParentDir = component {
            return Err("project path must not contain '..' traversal".into());
        }
    }
    Ok(())
}

pub fn load_state_from_disk(app: &AppHandle) -> Result<AppState, String> {
    let path = state_path(app)?;
    match std::fs::read_to_string(&path) {
        Ok(contents) => Ok(serde_json::from_str(&contents).unwrap_or_default()),
        Err(_) => Ok(AppState::default()),
    }
}

/// Atomic write: write to a temp file in the same directory, then rename.
fn save_state_to_disk(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let path = state_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("failed to create state dir: {e}"))?;
    }
    let json =
        serde_json::to_string_pretty(state).map_err(|e| format!("failed to serialize: {e}"))?;

    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &json).map_err(|e| format!("failed to write temp state: {e}"))?;
    std::fs::rename(&tmp_path, &path).map_err(|e| format!("failed to rename state file: {e}"))?;

    Ok(())
}

#[tauri::command]
pub fn get_recent_projects(app: AppHandle) -> Result<Vec<RecentProject>, String> {
    let handle = app.state::<AppStateHandle>();
    let guard = handle.0.lock().map_err(|e| format!("state lock failed: {e}"))?;
    Ok(guard.recent_projects.clone())
}

#[tauri::command]
pub fn save_session(
    app: AppHandle,
    project_path: String,
    mut session: SessionData,
    max_recent: usize,
) -> Result<(), String> {
    validate_path(&project_path)?;

    // Cap open_files to prevent unbounded growth
    session.open_files.truncate(MAX_SESSION_FILES);

    let state_snapshot = {
        let handle = app.state::<AppStateHandle>();
        let mut guard = handle.0.lock().map_err(|e| format!("state lock failed: {e}"))?;

        let name = std::path::Path::new(&project_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| project_path.clone());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Upsert: remove existing entry for this path
        guard.recent_projects.retain(|p| p.path != project_path);

        // Insert at front (most recent first)
        guard.recent_projects.insert(
            0,
            RecentProject {
                path: project_path,
                name,
                last_opened: now,
                session,
            },
        );

        // Truncate to max (clamped to 0..=30)
        guard.recent_projects.truncate(max_recent.min(30));

        guard.clone()
    }; // guard dropped here, mutex unlocked

    save_state_to_disk(&app, &state_snapshot)
}

#[tauri::command]
pub fn remove_recent_project(app: AppHandle, project_path: String) -> Result<(), String> {
    validate_path(&project_path)?;

    let state_snapshot = {
        let handle = app.state::<AppStateHandle>();
        let mut guard = handle.0.lock().map_err(|e| format!("state lock failed: {e}"))?;
        guard.recent_projects.retain(|p| p.path != project_path);
        guard.clone()
    }; // guard dropped here, mutex unlocked

    save_state_to_disk(&app, &state_snapshot)
}
