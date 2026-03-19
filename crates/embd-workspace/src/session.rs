use embd_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Persisted file state within a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    pub path: String,
    pub pinned: bool,
}

/// Snapshot of workspace state for session persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub open_files: Vec<SessionFile>,
    pub active_file: Option<String>,
}

/// A recently-opened project with its session data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    pub path: String,
    pub name: String,
    pub last_opened: u64,
    pub session: SessionData,
}

/// Application-level persistent state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppState {
    pub recent_projects: Vec<RecentProject>,
}

const MAX_SESSION_FILES: usize = 20;
const MAX_RECENT_PROJECTS: usize = 30;

impl AppState {
    /// Load state from a JSON file.
    pub fn load(path: &Path) -> Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                serde_json::from_str(&contents).map_err(|e| Error::Serialization(e.to_string()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::Io(e)),
        }
    }

    /// Save state to a JSON file atomically (write to temp, then rename).
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Save or update a project's session.
    pub fn save_session(
        &mut self,
        project_path: &str,
        mut session: SessionData,
        max_recent: usize,
    ) {
        session.open_files.truncate(MAX_SESSION_FILES);

        let name = Path::new(project_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| project_path.to_string());

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Remove existing entry for this path
        self.recent_projects
            .retain(|p| p.path != project_path);

        // Insert at front
        self.recent_projects.insert(
            0,
            RecentProject {
                path: project_path.to_string(),
                name,
                last_opened: now,
                session,
            },
        );

        self.recent_projects
            .truncate(max_recent.min(MAX_RECENT_PROJECTS));
    }

    /// Remove a project from the recent list.
    pub fn remove_project(&mut self, project_path: &str) {
        self.recent_projects
            .retain(|p| p.path != project_path);
    }

    /// Find a recent project by path.
    pub fn find_project(&self, project_path: &str) -> Option<&RecentProject> {
        self.recent_projects.iter().find(|p| p.path == project_path)
    }
}

/// Validate a project path is absolute and has no traversal.
pub fn validate_project_path(path: &str) -> Result<()> {
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(Error::Path("project path must be absolute".to_string()));
    }
    for component in p.components() {
        if let std::path::Component::ParentDir = component {
            return Err(Error::AccessDenied(
                "project path must not contain '..'".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_session() {
        let mut state = AppState::default();

        let session = SessionData {
            open_files: vec![SessionFile {
                path: "/proj/main.rs".to_string(),
                pinned: false,
            }],
            active_file: Some("/proj/main.rs".to_string()),
        };

        state.save_session("/home/user/proj", session, 10);
        assert_eq!(state.recent_projects.len(), 1);
        assert_eq!(state.recent_projects[0].name, "proj");
        assert_eq!(state.recent_projects[0].session.open_files.len(), 1);
    }

    #[test]
    fn test_upsert() {
        let mut state = AppState::default();

        let s1 = SessionData { open_files: vec![], active_file: None };
        state.save_session("/proj", s1, 10);
        state.save_session("/other", SessionData { open_files: vec![], active_file: None }, 10);
        state.save_session("/proj", SessionData { open_files: vec![], active_file: Some("/proj/a.rs".into()) }, 10);

        assert_eq!(state.recent_projects.len(), 2);
        // /proj should be first (most recent)
        assert_eq!(state.recent_projects[0].path, "/proj");
        assert_eq!(
            state.recent_projects[0].session.active_file,
            Some("/proj/a.rs".into())
        );
    }

    #[test]
    fn test_remove_project() {
        let mut state = AppState::default();
        state.save_session("/proj", SessionData { open_files: vec![], active_file: None }, 10);
        state.remove_project("/proj");
        assert!(state.recent_projects.is_empty());
    }

    #[test]
    fn test_validate_project_path() {
        assert!(validate_project_path("/valid/path").is_ok());
        assert!(validate_project_path("relative/path").is_err());
        assert!(validate_project_path("/path/../escape").is_err());
    }

    #[test]
    fn test_round_trip() {
        let tmp = std::env::temp_dir().join(format!("embd_session_test_{}", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let mut state = AppState::default();
        state.save_session("/proj", SessionData { open_files: vec![], active_file: None }, 10);
        state.save(&tmp).unwrap();

        let loaded = AppState::load(&tmp).unwrap();
        assert_eq!(loaded.recent_projects.len(), 1);
        assert_eq!(loaded.recent_projects[0].path, "/proj");

        let _ = std::fs::remove_file(&tmp);
    }
}
