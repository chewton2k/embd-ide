use serde::{Deserialize, Serialize};

/// Editor-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSettings {
    pub font_size: u32,
    pub tab_size: u32,
    pub word_wrap: bool,
    pub line_numbers: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_size: 13,
            tab_size: 2,
            word_wrap: false,
            line_numbers: true,
        }
    }
}

/// Terminal-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSettings {
    pub font_size: u32,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self { font_size: 13 }
    }
}

/// UI-level settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    pub font_size: u32,
    pub density: UiDensity,
    pub theme_id: String,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            font_size: 13,
            density: UiDensity::Comfortable,
            theme_id: "catppuccin-mocha".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiDensity {
    Compact,
    Comfortable,
}

/// Autosave settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutosaveSettings {
    pub enabled: bool,
    pub delay_ms: u32,
}

impl Default for AutosaveSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            delay_ms: 1000,
        }
    }
}

/// File tree filter patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenPattern {
    pub pattern: String,
    pub enabled: bool,
}

/// Session limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLimits {
    pub max_recent_projects: usize,
    pub max_tabs: usize,
}

impl Default for SessionLimits {
    fn default() -> Self {
        Self {
            max_recent_projects: 3,
            max_tabs: 9,
        }
    }
}

/// All user-configurable settings consolidated in one place.
///
/// This replaces the scattered localStorage reads in stores.ts with
/// a single serializable struct. Settings are loaded from a JSON/TOML
/// file on disk and can be exported/imported.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub editor: EditorSettings,
    pub terminal: TerminalSettings,
    pub ui: UiSettings,
    pub autosave: AutosaveSettings,
    pub hidden_patterns: Vec<HiddenPattern>,
    pub session_limits: SessionLimits,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            editor: EditorSettings::default(),
            terminal: TerminalSettings::default(),
            ui: UiSettings::default(),
            autosave: AutosaveSettings::default(),
            hidden_patterns: default_hidden_patterns(),
            session_limits: SessionLimits::default(),
        }
    }
}

impl Settings {
    /// Load settings from a JSON file, falling back to defaults.
    pub fn load(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save settings to a JSON file.
    pub fn save(&self, path: &std::path::Path) -> embd_core::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| embd_core::Error::Serialization(e.to_string()))?;
        std::fs::write(path, json)?;
        Ok(())
    }
}

fn default_hidden_patterns() -> Vec<HiddenPattern> {
    ["node_modules", "target", ".git", ".vscode", ".DS_Store"]
        .iter()
        .map(|p| HiddenPattern {
            pattern: p.to_string(),
            enabled: true,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = Settings::default();
        assert_eq!(s.editor.font_size, 13);
        assert_eq!(s.editor.tab_size, 2);
        assert!(s.autosave.enabled);
        assert_eq!(s.hidden_patterns.len(), 5);
    }

    #[test]
    fn test_settings_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("embd_settings_test_{}", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let mut s = Settings::default();
        s.editor.font_size = 16;
        s.ui.theme_id = "tokyo-night".to_string();
        s.save(&tmp).unwrap();

        let loaded = Settings::load(&tmp);
        assert_eq!(loaded.editor.font_size, 16);
        assert_eq!(loaded.ui.theme_id, "tokyo-night");

        let _ = std::fs::remove_file(&tmp);
    }
}
