use std::path::PathBuf;

use gpui::*;
use gpui::prelude::FluentBuilder as _;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::Root;
use gpui_component::Sizable;

use crate::{CursorShape, Settings, SidebarPosition, UiDensity, WhitespaceRender};

// ── Available themes ────────────────────────────────────────────────

const THEMES: &[&str] = &[
    "catppuccin-mocha",
    "catppuccin-latte",
    "catppuccin-frappe",
    "catppuccin-macchiato",
    "tokyo-night",
    "tokyo-night-storm",
    "tokyo-night-light",
    "dracula",
    "gruvbox-dark",
    "gruvbox-light",
    "nord",
    "solarized-dark",
    "solarized-light",
    "one-dark",
    "one-light",
    "rose-pine",
    "rose-pine-moon",
    "rose-pine-dawn",
    "monokai",
    "github-dark",
    "github-light",
];

// ── Colors (Catppuccin Mocha) ───────────────────────────────────────

struct C;
impl C {
    fn p() -> crate::palette::Palette { crate::palette::active_palette() }

    fn bg_base() -> Hsla     { rgb(Self::p().bg_base).into() }
    fn bg_surface() -> Hsla   { rgb(Self::p().bg_surface).into() }
    fn bg_elevated() -> Hsla  { rgb(Self::p().bg_elevated).into() }
    fn bg_input() -> Hsla     { darken(Self::p().bg_base, 0x040404) }
    fn bg_sidebar() -> Hsla   { darken(Self::p().bg_base, 0x060609) }
    fn bg_selected() -> Hsla  { rgb(Self::p().bg_elevated).into() }
    fn text() -> Hsla         { rgb(Self::p().text).into() }
    fn text_muted() -> Hsla   { rgb(Self::p().text_muted).into() }
    fn text_faint() -> Hsla   { rgb(Self::p().text_faint).into() }
    fn accent() -> Hsla       { rgb(Self::p().accent).into() }
    fn accent_hover() -> Hsla { rgb(Self::p().accent_dim).into() }
    fn border() -> Hsla       { rgb(Self::p().border).into() }
    fn border_subtle() -> Hsla { rgb(Self::p().border_subtle).into() }
    fn success() -> Hsla      { rgb(Self::p().success).into() }
    fn error() -> Hsla        { rgb(Self::p().error).into() }
}

/// Darken a hex color by subtracting per-channel (clamped to 0).
fn darken(base: u32, amount: u32) -> Hsla {
    let r = ((base >> 16) & 0xFF).saturating_sub((amount >> 16) & 0xFF);
    let g = ((base >> 8) & 0xFF).saturating_sub((amount >> 8) & 0xFF);
    let b = (base & 0xFF).saturating_sub(amount & 0xFF);
    rgb((r << 16) | (g << 8) | b).into()
}

// ── Actions ─────────────────────────────────────────────────────────

actions!(embd_settings, [CloseSettings]);

// ── Categories ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Editor,
    Terminal,
    Appearance,
    Files,
    Git,
    Keymap,
    Session,
    About,
}

impl Category {
    const ALL: [Category; 8] = [
        Category::Editor,
        Category::Terminal,
        Category::Appearance,
        Category::Files,
        Category::Git,
        Category::Keymap,
        Category::Session,
        Category::About,
    ];

    fn label(&self) -> &'static str {
        match self {
            Category::Editor => "Editor",
            Category::Terminal => "Terminal",
            Category::Appearance => "Appearance",
            Category::Files => "Files",
            Category::Git => "Git",
            Category::Keymap => "Keymap",
            Category::Session => "Session",
            Category::About => "About",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Category::Editor => "Font, tabs, formatting, and editing behavior",
            Category::Terminal => "Terminal font, shell, cursor, and scrollback",
            Category::Appearance => "Theme, UI font, density, and layout",
            Category::Files => "Autosave, deletion, and symlinks",
            Category::Git => "Gutter, blame, and auto-fetch",
            Category::Keymap => "Keyboard shortcut base map",
            Category::Session => "Startup, tabs, and project limits",
            Category::About => "Version and configuration info",
        }
    }
}

// ── Settings view ───────────────────────────────────────────────────

struct SettingsView {
    focus: FocusHandle,
    settings: Settings,
    settings_path: PathBuf,
    dirty: bool,
    saved_flash: bool,
    save_error: Option<String>,
    active_category: Category,

    // ── Editor inputs ──
    editor_font_family: Entity<InputState>,
    editor_font_size: Entity<InputState>,
    editor_line_height: Entity<InputState>,
    editor_tab_size: Entity<InputState>,

    // ── Terminal inputs ──
    terminal_font_family: Entity<InputState>,
    terminal_font_size: Entity<InputState>,
    terminal_line_height: Entity<InputState>,
    terminal_shell: Entity<InputState>,
    terminal_scrollback: Entity<InputState>,

    // ── Appearance inputs ──
    ui_font_size: Entity<InputState>,
    ui_font_family: Entity<InputState>,
    theme_dropdown_open: bool,
    sidebar_width: Entity<InputState>,

    // ── File inputs ──
    autosave_delay: Entity<InputState>,

    // ── Git inputs ──
    git_fetch_interval: Entity<InputState>,

    // ── Keymap inputs ──
    base_keymap: Entity<InputState>,

    // ── Session inputs ──
    max_recent_projects: Entity<InputState>,
    max_tabs: Entity<InputState>,
}

impl SettingsView {
    fn new(
        settings: Settings,
        settings_path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mk = |window: &mut Window, cx: &mut Context<Self>, val: String| {
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(""));
            state.update(cx, |s, cx| s.set_value(&val, window, cx));
            state
        };

        // Editor
        let editor_font_family = mk(window, cx, settings.editor.font_family.clone());
        let editor_font_size = mk(window, cx, settings.editor.font_size.to_string());
        let editor_line_height = mk(window, cx, format!("{:.1}", settings.editor.line_height));
        let editor_tab_size = mk(window, cx, settings.editor.tab_size.to_string());

        // Terminal
        let terminal_font_family = mk(window, cx, settings.terminal.font_family.clone());
        let terminal_font_size = mk(window, cx, settings.terminal.font_size.to_string());
        let terminal_line_height = mk(window, cx, format!("{:.1}", settings.terminal.line_height));
        let terminal_shell = mk(window, cx, settings.terminal.shell.clone());
        let terminal_scrollback = mk(window, cx, settings.terminal.scrollback_lines.to_string());

        // Appearance
        let ui_font_size = mk(window, cx, settings.appearance.ui_font_size.to_string());
        let ui_font_family = mk(window, cx, settings.appearance.ui_font_family.clone());

        let sidebar_width = mk(window, cx, settings.appearance.sidebar_width.to_string());

        // Files
        let autosave_delay = mk(window, cx, settings.files.autosave_delay_ms.to_string());

        // Git
        let git_fetch_interval = mk(window, cx, settings.git.auto_fetch_interval_secs.to_string());

        // Keymap
        let base_keymap = mk(window, cx, settings.keymap.base_keymap.clone());

        // Session
        let max_recent_projects = mk(window, cx, settings.session.max_recent_projects.to_string());
        let max_tabs = mk(window, cx, settings.session.max_tabs.to_string());

        let inputs: Vec<Entity<InputState>> = vec![
            editor_font_family.clone(),
            editor_font_size.clone(),
            editor_line_height.clone(),
            editor_tab_size.clone(),
            terminal_font_family.clone(),
            terminal_font_size.clone(),
            terminal_line_height.clone(),
            terminal_shell.clone(),
            terminal_scrollback.clone(),
            ui_font_size.clone(),
            ui_font_family.clone(),
            sidebar_width.clone(),
            autosave_delay.clone(),
            git_fetch_interval.clone(),
            base_keymap.clone(),
            max_recent_projects.clone(),
            max_tabs.clone(),
        ];
        for input in &inputs {
            cx.subscribe(input, |this: &mut Self, _, _: &InputEvent, cx| {
                this.dirty = true;
                this.saved_flash = false;
                this.save_error = None;
                cx.notify();
            })
            .detach();
        }

        Self {
            focus: cx.focus_handle(),
            settings,
            settings_path,
            dirty: false,
            saved_flash: false,
            save_error: None,
            active_category: Category::Editor,
            editor_font_family,
            editor_font_size,
            editor_line_height,
            editor_tab_size,
            terminal_font_family,
            terminal_font_size,
            terminal_line_height,
            terminal_shell,
            terminal_scrollback,
            ui_font_size,
            ui_font_family,
            theme_dropdown_open: false,
            sidebar_width,
            autosave_delay,
            git_fetch_interval,
            base_keymap,
            max_recent_projects,
            max_tabs,
        }
    }

    fn read_u32(state: &Entity<InputState>, cx: &App) -> Option<u32> {
        state.read(cx).value().trim().parse().ok()
    }

    fn read_usize(state: &Entity<InputState>, cx: &App) -> Option<usize> {
        state.read(cx).value().trim().parse().ok()
    }

    fn read_f32(state: &Entity<InputState>, cx: &App) -> Option<f32> {
        state.read(cx).value().trim().parse().ok()
    }

    fn read_str(state: &Entity<InputState>, cx: &App) -> Option<String> {
        let v = state.read(cx).value().trim().to_string();
        if v.is_empty() { None } else { Some(v) }
    }

    fn apply_and_save(&mut self, cx: &mut Context<Self>) {
        // Editor
        if let Some(v) = Self::read_str(&self.editor_font_family, cx) {
            self.settings.editor.font_family = v;
        }
        if let Some(v) = Self::read_u32(&self.editor_font_size, cx) {
            self.settings.editor.font_size = v;
        }
        if let Some(v) = Self::read_f32(&self.editor_line_height, cx) {
            self.settings.editor.line_height = v;
        }
        if let Some(v) = Self::read_u32(&self.editor_tab_size, cx) {
            self.settings.editor.tab_size = v;
        }

        // Terminal
        if let Some(v) = Self::read_str(&self.terminal_font_family, cx) {
            self.settings.terminal.font_family = v;
        }
        if let Some(v) = Self::read_u32(&self.terminal_font_size, cx) {
            self.settings.terminal.font_size = v;
        }
        if let Some(v) = Self::read_f32(&self.terminal_line_height, cx) {
            self.settings.terminal.line_height = v;
        }
        if let Some(v) = Self::read_str(&self.terminal_shell, cx) {
            self.settings.terminal.shell = v;
        }
        if let Some(v) = Self::read_u32(&self.terminal_scrollback, cx) {
            self.settings.terminal.scrollback_lines = v;
        }

        // Appearance
        if let Some(v) = Self::read_u32(&self.ui_font_size, cx) {
            self.settings.appearance.ui_font_size = v;
        }
        if let Some(v) = Self::read_str(&self.ui_font_family, cx) {
            self.settings.appearance.ui_font_family = v;
        }
        // theme_id is updated via SelectEvent, no need to read from input
        if let Some(v) = Self::read_u32(&self.sidebar_width, cx) {
            self.settings.appearance.sidebar_width = v;
        }

        // Files
        if let Some(v) = Self::read_u32(&self.autosave_delay, cx) {
            self.settings.files.autosave_delay_ms = v;
        }

        // Git
        if let Some(v) = Self::read_u32(&self.git_fetch_interval, cx) {
            self.settings.git.auto_fetch_interval_secs = v;
        }

        // Keymap
        if let Some(v) = Self::read_str(&self.base_keymap, cx) {
            self.settings.keymap.base_keymap = v;
        }

        // Session
        if let Some(v) = Self::read_usize(&self.max_recent_projects, cx) {
            self.settings.session.max_recent_projects = v;
        }
        if let Some(v) = Self::read_usize(&self.max_tabs, cx) {
            self.settings.session.max_tabs = v;
        }

        match self.settings.save(&self.settings_path) {
            Ok(()) => {
                self.dirty = false;
                self.saved_flash = true;
                self.save_error = None;
                // Apply theme change immediately
                crate::palette::set_theme(&self.settings.appearance.theme_id);
            }
            Err(e) => {
                self.save_error = Some(e.to_string());
            }
        }
        cx.notify();
    }

    fn toggle_bool(
        &mut self,
        getter: fn(&Settings) -> bool,
        setter: fn(&mut Settings, bool),
        cx: &mut Context<Self>,
    ) {
        let current = getter(&self.settings);
        setter(&mut self.settings, !current);
        self.dirty = true;
        self.saved_flash = false;
        self.save_error = None;
        cx.notify();
    }

    fn cycle_whitespace(&mut self, cx: &mut Context<Self>) {
        self.settings.editor.show_whitespace = match self.settings.editor.show_whitespace {
            WhitespaceRender::None => WhitespaceRender::Selection,
            WhitespaceRender::Selection => WhitespaceRender::All,
            WhitespaceRender::All => WhitespaceRender::None,
        };
        self.dirty = true;
        self.saved_flash = false;
        cx.notify();
    }

    fn cycle_cursor_shape(&mut self, cx: &mut Context<Self>) {
        self.settings.terminal.cursor_shape = match self.settings.terminal.cursor_shape {
            CursorShape::Block => CursorShape::Bar,
            CursorShape::Bar => CursorShape::Underline,
            CursorShape::Underline => CursorShape::Block,
        };
        self.dirty = true;
        self.saved_flash = false;
        cx.notify();
    }

    fn cycle_density(&mut self, cx: &mut Context<Self>) {
        self.settings.appearance.density = match self.settings.appearance.density {
            UiDensity::Compact => UiDensity::Comfortable,
            UiDensity::Comfortable => UiDensity::Compact,
        };
        self.dirty = true;
        self.saved_flash = false;
        cx.notify();
    }

    fn cycle_sidebar_position(&mut self, cx: &mut Context<Self>) {
        self.settings.appearance.sidebar_position = match self.settings.appearance.sidebar_position {
            SidebarPosition::Left => SidebarPosition::Right,
            SidebarPosition::Right => SidebarPosition::Left,
        };
        self.dirty = true;
        self.saved_flash = false;
        cx.notify();
    }
}

// ── Render impl ─────────────────────────────────────────────────────

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focus.contains_focused(window, cx) {
            self.focus.focus(window);
        }

        let ui_size = self.settings.appearance.ui_font_size.max(8).min(24) as f32;

        div()
            .id("settings-root")
            .track_focus(&self.focus)
            .size_full()
            .bg(C::bg_base())
            .text_color(C::text())
            .text_size(px(ui_size))
            .flex()
            .flex_col()
            .on_action(cx.listener(|_, _: &CloseSettings, _w, cx| cx.quit()))
            // ── Titlebar ────────────────────────────────────────
            .child(
                div()
                    .id("settings-titlebar")
                    .h(px(36.0))
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(C::bg_base())
                    .border_b_1()
                    .border_color(C::border_subtle())
                    .on_mouse_down(MouseButton::Left, |_, window, _| {
                        window.start_window_move();
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(C::text_faint())
                            .child("Settings"),
                    ),
            )
            // ── Main body: sidebar + content ────────────────────
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_row()
                    // ── Left sidebar ──
                    .child(self.render_sidebar(cx))
                    // ── Right content panel ──
                    .child(self.render_content_panel(cx)),
            )
            // ── Footer ──────────────────────────────────────────
            .child(self.render_footer(cx))
    }
}

impl SettingsView {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let active = self.active_category;

        let mut sidebar = div()
            .w(px(160.0))
            .flex_shrink_0()
            .h_full()
            .bg(C::bg_sidebar())
            .border_r_1()
            .border_color(C::border_subtle())
            .flex()
            .flex_col()
            .py(px(8.0))
            .px(px(8.0))
            .gap(px(2.0));

        for cat in Category::ALL {
            let is_active = cat == active;
            sidebar = sidebar.child(
                div()
                    .id(SharedString::from(format!("cat-{}", cat.label())))
                    .w_full()
                    .h(px(32.0))
                    .px(px(10.0))
                    .rounded(px(6.0))
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .text_xs()
                    .when(is_active, |d| {
                        d.bg(C::bg_selected())
                            .text_color(C::text())
                            .font_weight(FontWeight::MEDIUM)
                    })
                    .when(!is_active, |d| {
                        d.text_color(C::text_muted())
                            .hover(|s| s.bg(C::bg_elevated()).text_color(C::text()))
                    })
                    .child(cat.label())
                    .on_click(cx.listener(move |this, _, _w, cx| {
                        this.active_category = cat;
                        cx.notify();
                    })),
            );
        }

        sidebar
    }

    fn render_content_panel(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let cat = self.active_category;

        let content = match cat {
            Category::Editor => self.render_editor_section(cx),
            Category::Terminal => self.render_terminal_section(cx),
            Category::Appearance => self.render_appearance_section(cx),
            Category::Files => self.render_files_section(cx),
            Category::Git => self.render_git_section(cx),
            Category::Keymap => self.render_keymap_section(cx),
            Category::Session => self.render_session_section(cx),
            Category::About => self.render_about_section(),
        };

        div()
            .id("settings-content-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .child(
                div()
                    .w_full()
                    .px(px(20.0))
                    .pt(px(16.0))
                    .pb(px(20.0))
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    // Category header
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(C::text())
                                    .child(cat.label()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(C::text_faint())
                                    .child(cat.description()),
                            ),
                    )
                    // Settings card
                    .child(content),
            )
    }

    fn render_editor_section(&self, cx: &mut Context<Self>) -> Div {
        let s = &self.settings.editor;
        let ws_label = match s.show_whitespace {
            WhitespaceRender::None => "none",
            WhitespaceRender::Selection => "selection",
            WhitespaceRender::All => "all",
        };

        card()
            .child(field_row("Font Family", &self.editor_font_family, "", false))
            .child(field_row("Font Size", &self.editor_font_size, "px", false))
            .child(field_row("Line Height", &self.editor_line_height, "", false))
            .child(field_row("Tab Size", &self.editor_tab_size, "spaces", false))
            .child(
                toggle_row("toggle-hard-tabs", "Hard Tabs", s.hard_tabs, "Use tabs instead of spaces", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.hard_tabs, |s, v| s.editor.hard_tabs = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-wordwrap", "Word Wrap", s.word_wrap, "Wrap lines at viewport width", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.word_wrap, |s, v| s.editor.word_wrap = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-linenums", "Line Numbers", s.line_numbers, "Show line numbers in the gutter", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.line_numbers, |s, v| s.editor.line_numbers = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-highlight-active", "Highlight Active Line", s.highlight_active_line, "Highlight the current cursor line", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.highlight_active_line, |s, v| s.editor.highlight_active_line = v, cx);
                    })),
            )
            .child(
                cycle_row("cycle-whitespace", "Show Whitespace", ws_label, "Render whitespace characters", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.cycle_whitespace(cx);
                    })),
            )
            .child(
                toggle_row("toggle-cursor-blink", "Cursor Blink", s.cursor_blink, "Blink the editor cursor", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.cursor_blink, |s, v| s.editor.cursor_blink = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-bracket-pair", "Bracket Pair Highlight", s.bracket_pair_highlight, "Highlight matching brackets", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.bracket_pair_highlight, |s, v| s.editor.bracket_pair_highlight = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-indent-guides", "Indent Guides", s.indent_guides, "Show indentation guide lines", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.indent_guides, |s, v| s.editor.indent_guides = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-scroll-beyond", "Scroll Beyond Last Line", s.scroll_beyond_last_line, "Allow scrolling past the last line", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.scroll_beyond_last_line, |s, v| s.editor.scroll_beyond_last_line = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-minimap", "Minimap", s.minimap, "Show a minimap of the document", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.minimap, |s, v| s.editor.minimap = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-sticky-scroll", "Sticky Scroll", s.sticky_scroll, "Pin scope headers while scrolling", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.sticky_scroll, |s, v| s.editor.sticky_scroll = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-format-on-save", "Format on Save", s.format_on_save, "Auto-format when saving a file", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.format_on_save, |s, v| s.editor.format_on_save = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-trim-whitespace", "Trim Trailing Whitespace", s.trim_trailing_whitespace, "Remove trailing spaces on save", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.trim_trailing_whitespace, |s, v| s.editor.trim_trailing_whitespace = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-trim-newlines", "Trim Final Newlines", s.trim_final_newlines, "Remove extra newlines at end of file", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.trim_final_newlines, |s, v| s.editor.trim_final_newlines = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-insert-newline", "Insert Final Newline", s.insert_final_newline, "Ensure file ends with a newline", true)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.editor.insert_final_newline, |s, v| s.editor.insert_final_newline = v, cx);
                    })),
            )
    }

    fn render_terminal_section(&self, cx: &mut Context<Self>) -> Div {
        let s = &self.settings.terminal;
        let cursor_label = match s.cursor_shape {
            CursorShape::Block => "block",
            CursorShape::Bar => "bar",
            CursorShape::Underline => "underline",
        };

        card()
            .child(field_row("Font Family", &self.terminal_font_family, "", false))
            .child(field_row("Font Size", &self.terminal_font_size, "px", false))
            .child(field_row("Line Height", &self.terminal_line_height, "", false))
            .child(field_row("Shell", &self.terminal_shell, "", false))
            .child(
                cycle_row("cycle-cursor-shape", "Cursor Shape", cursor_label, "Terminal cursor style", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.cycle_cursor_shape(cx);
                    })),
            )
            .child(
                toggle_row("toggle-copy-select", "Copy on Select", s.copy_on_select, "Copy text to clipboard when selected", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.terminal.copy_on_select, |s, v| s.terminal.copy_on_select = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-term-blink", "Cursor Blinking", s.blinking, "Blink the terminal cursor", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.terminal.blinking, |s, v| s.terminal.blinking = v, cx);
                    })),
            )
            .child(field_row("Scrollback Lines", &self.terminal_scrollback, "lines", true))
    }

    fn render_appearance_section(&self, cx: &mut Context<Self>) -> Div {
        let s = &self.settings.appearance;
        let density_label = match s.density {
            UiDensity::Compact => "compact",
            UiDensity::Comfortable => "comfortable",
        };
        let sidebar_label = match s.sidebar_position {
            SidebarPosition::Left => "left",
            SidebarPosition::Right => "right",
        };

        card()
            .child(field_row("UI Font Size", &self.ui_font_size, "px", false))
            .child(field_row("UI Font Family", &self.ui_font_family, "", false))
            .child(
                cycle_row("cycle-density", "UI Density", density_label, "Compact or comfortable spacing", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.cycle_density(cx);
                    })),
            )
            .child(self.render_theme_dropdown(cx))
            .child(
                toggle_row("toggle-tab-bar", "Show Tab Bar", s.show_tab_bar, "Show the tab bar at the top", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.appearance.show_tab_bar, |s, v| s.appearance.show_tab_bar = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-status-bar", "Show Status Bar", s.show_status_bar, "Show the status bar at the bottom", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.appearance.show_status_bar, |s, v| s.appearance.show_status_bar = v, cx);
                    })),
            )
            .child(
                cycle_row("cycle-sidebar-pos", "Sidebar Position", sidebar_label, "Left or right sidebar", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.cycle_sidebar_position(cx);
                    })),
            )
            .child(field_row("Sidebar Width", &self.sidebar_width, "px", true))
    }

    fn render_files_section(&self, cx: &mut Context<Self>) -> Div {
        let s = &self.settings.files;

        card()
            .child(
                toggle_row("toggle-autosave", "Autosave", s.autosave, "Automatically save files after changes", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.files.autosave, |s, v| s.files.autosave = v, cx);
                    })),
            )
            .child(field_row("Autosave Delay", &self.autosave_delay, "ms", false))
            .child(
                toggle_row("toggle-confirm-delete", "Confirm Delete", s.confirm_delete, "Ask before deleting files", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.files.confirm_delete, |s, v| s.files.confirm_delete = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-follow-symlinks", "Follow Symlinks", s.follow_symlinks, "Resolve symbolic links in file tree", true)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.files.follow_symlinks, |s, v| s.files.follow_symlinks = v, cx);
                    })),
            )
    }

    fn render_git_section(&self, cx: &mut Context<Self>) -> Div {
        let s = &self.settings.git;

        card()
            .child(
                toggle_row("toggle-git-gutter", "Git Gutter", s.git_gutter, "Show git change indicators in the gutter", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.git.git_gutter, |s, v| s.git.git_gutter = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-inline-blame", "Inline Blame", s.inline_blame, "Show git blame inline at end of line", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.git.inline_blame, |s, v| s.git.inline_blame = v, cx);
                    })),
            )
            .child(
                toggle_row("toggle-auto-fetch", "Auto Fetch", s.auto_fetch, "Periodically fetch from remote", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.git.auto_fetch, |s, v| s.git.auto_fetch = v, cx);
                    })),
            )
            .child(field_row("Fetch Interval", &self.git_fetch_interval, "secs", true))
    }

    fn render_keymap_section(&self, _cx: &mut Context<Self>) -> Div {
        card()
            .child(field_row("Base Keymap", &self.base_keymap, "", true))
    }

    fn render_session_section(&self, cx: &mut Context<Self>) -> Div {
        let s = &self.settings.session;

        card()
            .child(
                toggle_row("toggle-restore", "Restore on Startup", s.restore_on_startup, "Reopen previous session on launch", false)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.session.restore_on_startup, |s, v| s.session.restore_on_startup = v, cx);
                    })),
            )
            .child(field_row("Max Recent Projects", &self.max_recent_projects, "", false))
            .child(field_row("Max Open Tabs", &self.max_tabs, "", false))
            .child(
                toggle_row("toggle-close-last-tab", "Close Window on Last Tab", s.close_window_on_last_tab, "Close the window when the last tab is closed", true)
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.toggle_bool(|s| s.session.close_window_on_last_tab, |s, v| s.session.close_window_on_last_tab = v, cx);
                    })),
            )
    }

    fn render_theme_dropdown(&self, cx: &mut Context<Self>) -> Div {
        let current = &self.settings.appearance.theme_id;
        let is_open = self.theme_dropdown_open;

        let mut wrapper = div()
            .w_full()
            .relative()
            .min_h(px(42.0))
            .mx(px(14.0))
            .pr(px(28.0))
            .py(px(6.0))
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(C::border_subtle());

        // Row: label + button
        wrapper = wrapper.child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .child(
                            div()
                                .text_xs()
                                .text_color(C::text_muted())
                                .child("Theme"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(C::text_faint())
                                .child("Color theme for the editor"),
                        ),
                )
                .child(
                    div()
                        .id("theme-dropdown-btn")
                        .h(px(26.0))
                        .px(px(10.0))
                        .rounded(px(5.0))
                        .bg(C::bg_elevated())
                        .border_1()
                        .border_color(C::border())
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .cursor_pointer()
                        .hover(|s| s.bg(C::bg_input()))
                        .text_xs()
                        .text_color(C::text_muted())
                        .child(current.clone())
                        .child(
                            div()
                                .text_size(px(9.0))
                                .text_color(C::text_faint())
                                .child(if is_open { "\u{25B2}" } else { "\u{25BC}" }),
                        )
                        .on_click(cx.listener(|this, _, _w, cx| {
                            this.theme_dropdown_open = !this.theme_dropdown_open;
                            cx.notify();
                        })),
                ),
        );

        // Dropdown list (if open)
        if is_open {
            let mut list = div()
                .id("theme-dropdown-list")
                .mt(px(6.0))
                .w_full()
                .max_h(px(200.0))
                .overflow_y_scroll()
                .bg(C::bg_elevated())
                .border_1()
                .border_color(C::border())
                .rounded(px(6.0))
                .py(px(4.0));

            for theme in THEMES {
                let is_selected = *theme == current.as_str();
                let theme_str = theme.to_string();
                list = list.child(
                    div()
                        .id(SharedString::from(format!("theme-{}", theme)))
                        .w_full()
                        .h(px(28.0))
                        .px(px(10.0))
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .text_xs()
                        .rounded(px(4.0))
                        .mx(px(4.0))
                        .when(is_selected, |d| {
                            d.bg(C::accent())
                                .text_color(C::bg_base())
                                .font_weight(FontWeight::MEDIUM)
                        })
                        .when(!is_selected, |d| {
                            d.text_color(C::text_muted())
                                .hover(|s| s.bg(C::bg_surface()).text_color(C::text()))
                        })
                        .child(*theme)
                        .on_click(cx.listener(move |this, _, _w, cx| {
                            this.settings.appearance.theme_id = theme_str.clone();
                            this.theme_dropdown_open = false;
                            this.dirty = true;
                            this.saved_flash = false;
                            this.save_error = None;
                            cx.notify();
                        })),
                );
            }

            wrapper = wrapper.child(list);
        }

        wrapper
    }

    fn render_about_section(&self) -> Div {
        card()
            .child(static_row("Version", "0.2.0", false))
            .child(static_row("Runtime", "GPUI", false))
            .child(static_row(
                "Config",
                &self.settings_path.to_string_lossy(),
                true,
            ))
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .h(px(44.0))
            .flex_shrink_0()
            .border_t_1()
            .border_color(C::border())
            .bg(C::bg_surface())
            .px(px(16.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_xs()
                    .text_color(if self.saved_flash {
                        C::success()
                    } else if self.save_error.is_some() {
                        C::error()
                    } else {
                        C::text_faint()
                    })
                    .child(if self.saved_flash {
                        "Settings saved".to_string()
                    } else if let Some(ref e) = self.save_error {
                        format!("Error: {e}")
                    } else if self.dirty {
                        "Unsaved changes".to_string()
                    } else {
                        "Up to date".to_string()
                    }),
            )
            .child(
                div()
                    .id("save-btn")
                    .h(px(28.0))
                    .px(px(18.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(self.dirty, |d| {
                        d.bg(C::accent())
                            .text_color(C::bg_base())
                            .hover(|s| s.bg(C::accent_hover()))
                    })
                    .when(!self.dirty, |d| {
                        d.bg(C::bg_elevated())
                            .text_color(C::text_faint())
                    })
                    .child("Save")
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.apply_and_save(cx);
                    })),
            )
    }
}

// ── Render helpers ──────────────────────────────────────────────────

fn card() -> Div {
    div()
        .w_full()
        .bg(C::bg_surface())
        .border_1()
        .border_color(C::border())
        .rounded(px(8.0))
}

/// An input field row with label, input box, and optional suffix.
fn field_row(
    label: &'static str,
    input: &Entity<InputState>,
    suffix: &'static str,
    last: bool,
) -> Div {
    div()
        .w_full()
        .h(px(38.0))
        .mx(px(14.0))
        .pr(px(28.0))
        .flex()
        .items_center()
        .justify_between()
        .when(!last, |d| d.border_b_1().border_color(C::border_subtle()))
        .child(
            div()
                .text_xs()
                .text_color(C::text_muted())
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(
                    div()
                        .w(px(80.0))
                        .h(px(26.0))
                        .bg(C::bg_input())
                        .rounded(px(5.0))
                        .border_1()
                        .border_color(C::border_subtle())
                        .flex()
                        .items_center()
                        .px(px(8.0))
                        .child(
                            Input::new(input)
                                .appearance(false)
                                .bordered(false)
                                .xsmall(),
                        ),
                )
                .when(!suffix.is_empty(), |d| {
                    d.child(
                        div()
                            .text_size(px(10.0))
                            .text_color(C::text_faint())
                            .child(suffix),
                    )
                }),
        )
}

/// A toggle switch row with label, description, and on/off switch.
fn toggle_row(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    description: &'static str,
    last: bool,
) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("{}-row", id)))
        .w_full()
        .min_h(px(38.0))
        .mx(px(14.0))
        .pr(px(28.0))
        .py(px(6.0))
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .when(!last, |d| d.border_b_1().border_color(C::border_subtle()))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(C::text_muted())
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(C::text_faint())
                        .child(description),
                ),
        )
        .child(toggle_switch(id, enabled))
}

/// A cycle-value row (click to cycle through options).
fn cycle_row(
    id: &'static str,
    label: &'static str,
    value: &str,
    description: &'static str,
    last: bool,
) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("{}-row", id)))
        .w_full()
        .min_h(px(38.0))
        .mx(px(14.0))
        .pr(px(28.0))
        .py(px(6.0))
        .flex()
        .items_center()
        .justify_between()
        .cursor_pointer()
        .when(!last, |d| d.border_b_1().border_color(C::border_subtle()))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.0))
                .child(
                    div()
                        .text_xs()
                        .text_color(C::text_muted())
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(C::text_faint())
                        .child(description),
                ),
        )
        .child(
            div()
                .h(px(24.0))
                .px(px(10.0))
                .rounded(px(5.0))
                .bg(C::bg_elevated())
                .border_1()
                .border_color(C::border())
                .flex()
                .items_center()
                .text_xs()
                .text_color(C::text_muted())
                .child(value.to_string()),
        )
}

fn toggle_switch(id: &'static str, enabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(36.0))
        .h(px(20.0))
        .rounded(px(10.0))
        .bg(if enabled { C::accent() } else { C::bg_elevated() })
        .border_1()
        .border_color(if enabled { C::accent() } else { C::border() })
        .flex()
        .items_center()
        .px(px(2.0))
        .flex_shrink_0()
        .child(
            div()
                .size(px(14.0))
                .rounded_full()
                .bg(if enabled { C::bg_base() } else { C::text_faint() })
                .when(enabled, |d| d.ml(px(14.0))),
        )
}

/// A read-only info row.
fn static_row(label: &'static str, value: &str, last: bool) -> Div {
    div()
        .w_full()
        .h(px(38.0))
        .mx(px(14.0))
        .pr(px(28.0))
        .flex()
        .items_center()
        .justify_between()
        .when(!last, |d| d.border_b_1().border_color(C::border_subtle()))
        .child(
            div()
                .text_xs()
                .text_color(C::text_muted())
                .child(label),
        )
        .child(
            div()
                .text_xs()
                .text_color(C::text_faint())
                .max_w(px(220.0))
                .overflow_x_hidden()
                .flex_shrink()
                .child(value.to_string()),
        )
}

// ── Public API ──────────────────────────────────────────────────────

/// Open a separate settings window. Call from the main app.
pub fn open_settings_window(app: &mut App) {
    let settings_path = Settings::default_path();
    let settings = Settings::load(&settings_path);

    let bounds = Bounds::centered(None, size(px(600.0), px(500.0)), app);
    if let Err(e) = app.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Settings".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(9.0), px(9.0))),
            }),
            ..Default::default()
        },
        move |window, app| {
            gpui_component::init(app);
            app.bind_keys([KeyBinding::new("cmd-w", CloseSettings, None)]);

            let view = app.new(|cx| {
                SettingsView::new(settings.clone(), settings_path.clone(), window, cx)
            });
            let view: AnyView = view.into();
            app.new(|cx| Root::new(view, window, cx))
        },
    ) {
        eprintln!("Failed to open settings window: {e}");
    }
}
