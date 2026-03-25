use serde::{Deserialize, Serialize};

// ── Editor ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSettings {
    pub font_family: String,
    pub font_size: u32,
    pub line_height: f32,
    pub tab_size: u32,
    pub hard_tabs: bool,
    pub word_wrap: bool,
    pub line_numbers: bool,
    pub highlight_active_line: bool,
    pub show_whitespace: WhitespaceRender,
    pub cursor_blink: bool,
    pub bracket_pair_highlight: bool,
    pub indent_guides: bool,
    pub scroll_beyond_last_line: bool,
    pub minimap: bool,
    pub sticky_scroll: bool,
    pub format_on_save: bool,
    pub trim_trailing_whitespace: bool,
    pub trim_final_newlines: bool,
    pub insert_final_newline: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_family: "monospace".to_string(),
            font_size: 13,
            line_height: 1.6,
            tab_size: 4,
            hard_tabs: false,
            word_wrap: false,
            line_numbers: true,
            highlight_active_line: true,
            show_whitespace: WhitespaceRender::Selection,
            cursor_blink: true,
            bracket_pair_highlight: true,
            indent_guides: true,
            scroll_beyond_last_line: true,
            minimap: false,
            sticky_scroll: false,
            format_on_save: false,
            trim_trailing_whitespace: true,
            trim_final_newlines: false,
            insert_final_newline: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WhitespaceRender {
    None,
    Selection,
    All,
}

// ── Terminal ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSettings {
    pub font_family: String,
    pub font_size: u32,
    pub line_height: f32,
    pub shell: String,
    pub cursor_shape: CursorShape,
    pub copy_on_select: bool,
    pub blinking: bool,
    pub scrollback_lines: u32,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            font_family: "monospace".to_string(),
            font_size: 13,
            line_height: 1.4,
            shell: "default".to_string(),
            cursor_shape: CursorShape::Block,
            copy_on_select: false,
            blinking: true,
            scrollback_lines: 10000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CursorShape {
    Block,
    Bar,
    Underline,
}

// ── Appearance ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppearanceSettings {
    pub ui_font_size: u32,
    pub ui_font_family: String,
    pub density: UiDensity,
    pub theme_id: String,
    pub show_tab_bar: bool,
    pub show_status_bar: bool,
    pub sidebar_position: SidebarPosition,
    pub sidebar_width: u32,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            ui_font_size: 13,
            ui_font_family: "system-ui".to_string(),
            density: UiDensity::Comfortable,
            theme_id: "catppuccin-mocha".to_string(),
            show_tab_bar: true,
            show_status_bar: true,
            sidebar_position: SidebarPosition::Left,
            sidebar_width: 240,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiDensity {
    Compact,
    Comfortable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SidebarPosition {
    Left,
    Right,
}

// ── File Management ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSettings {
    pub autosave: bool,
    pub autosave_delay_ms: u32,
    pub confirm_delete: bool,
    pub follow_symlinks: bool,
    pub hidden_patterns: Vec<HiddenPattern>,
}

impl Default for FileSettings {
    fn default() -> Self {
        Self {
            autosave: true,
            autosave_delay_ms: 1000,
            confirm_delete: true,
            follow_symlinks: true,
            hidden_patterns: default_hidden_patterns(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiddenPattern {
    pub pattern: String,
    pub enabled: bool,
}

// ── Git ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSettings {
    pub git_gutter: bool,
    pub inline_blame: bool,
    pub auto_fetch: bool,
    pub auto_fetch_interval_secs: u32,
}

impl Default for GitSettings {
    fn default() -> Self {
        Self {
            git_gutter: true,
            inline_blame: false,
            auto_fetch: true,
            auto_fetch_interval_secs: 300,
        }
    }
}

// ── Keybindings hint ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeymapSettings {
    pub base_keymap: String,
}

impl Default for KeymapSettings {
    fn default() -> Self {
        Self {
            base_keymap: "default".to_string(),
        }
    }
}

// ── Session / Window ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSettings {
    pub restore_on_startup: bool,
    pub max_recent_projects: usize,
    pub max_tabs: usize,
    pub close_window_on_last_tab: bool,
}

impl Default for SessionSettings {
    fn default() -> Self {
        Self {
            restore_on_startup: true,
            max_recent_projects: 5,
            max_tabs: 20,
            close_window_on_last_tab: false,
        }
    }
}

// ── Root settings ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub editor: EditorSettings,
    pub terminal: TerminalSettings,
    pub appearance: AppearanceSettings,
    pub files: FileSettings,
    pub git: GitSettings,
    pub keymap: KeymapSettings,
    pub session: SessionSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            editor: EditorSettings::default(),
            terminal: TerminalSettings::default(),
            appearance: AppearanceSettings::default(),
            files: FileSettings::default(),
            git: GitSettings::default(),
            keymap: KeymapSettings::default(),
            session: SessionSettings::default(),
        }
    }
}

impl Settings {
    pub fn load(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &std::path::Path) -> embd_core::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| embd_core::Error::Serialization(e.to_string()))?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn default_path() -> std::path::PathBuf {
        let mut p = dirs_fallback();
        p.push("embd");
        p.push("settings.json");
        p
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

fn dirs_fallback() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".config")
    } else {
        std::path::PathBuf::from(".config")
    }
}

// ── Backward-compat re-exports ──────────────────────────────────────
// Old code may reference these directly; keep them reachable.

pub type AutosaveSettings = FileSettings;
pub type UiSettings = AppearanceSettings;
pub type SessionLimits = SessionSettings;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let s = Settings::default();
        assert_eq!(s.editor.font_size, 13);
        assert_eq!(s.editor.tab_size, 4);
        assert!(s.files.autosave);
        assert_eq!(s.files.hidden_patterns.len(), 5);
    }

    #[test]
    fn test_settings_roundtrip() {
        let tmp =
            std::env::temp_dir().join(format!("embd_settings_test_{}", std::process::id()));
        let _ = std::fs::remove_file(&tmp);

        let mut s = Settings::default();
        s.editor.font_size = 16;
        s.appearance.theme_id = "tokyo-night".to_string();
        s.save(&tmp).unwrap();

        let loaded = Settings::load(&tmp);
        assert_eq!(loaded.editor.font_size, 16);
        assert_eq!(loaded.appearance.theme_id, "tokyo-night");

        let _ = std::fs::remove_file(&tmp);
    }
}
