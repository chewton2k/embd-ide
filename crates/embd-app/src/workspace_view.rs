use std::collections::HashMap;
use std::path::PathBuf;

use gpui::*;
use gpui::prelude::FluentBuilder as _;
use gpui_component::list::ListItem;
use gpui_component::tree::{tree, TreeItem, TreeState};

use embd_core::types::Position;
use embd_editor::SelectionSet;
use embd_platform::fs::{FileEntry, ProjectFs};
use embd_platform::git::GitRepo;
use embd_workspace::project::Workspace;
use embd_workspace::settings::Settings;

use crate::git_panel::{self, GitPanelData};
use crate::search_modal::{SearchEvent, SearchModal};
use crate::terminal::TerminalPane;
use crate::theme::Colors;

// ── Actions ─────────────────────────────────────────────────────────

actions!(
    workspace,
    [
        Quit,
        ToggleSidebar,
        ToggleTerminal,
        ToggleGitPanel,
        OpenFolder,
        Save,
        Undo,
        Redo,
        SelectAll,
        Copy,
        Cut,
        Paste,
        CloseTab,
        NextTab,
        PrevTab,
        ToggleSearchModal,
    ]
);

// ── Workspace root view ─────────────────────────────────────────────

pub struct WorkspaceView {
    focus_handle: FocusHandle,
    editor_focus: FocusHandle,
    sidebar_open: bool,
    terminal_open: bool,
    git_panel_open: bool,
    sidebar_width: f32,
    terminal_height: f32,
    git_panel_width: f32,

    // Project state
    project_fs: Option<ProjectFs>,
    workspace: Option<Workspace>,
    tree_state: Entity<TreeState>,

    // Git state
    git_data: GitPanelData,
    commit_message: String,

    // Search modal
    search_modal: Entity<SearchModal>,
    search_visible: bool,

    // Terminal
    terminal_pane: Entity<TerminalPane>,

    // Git diff preview text
    diff_text: String,

    // File tree git status cache
    git_status_map: HashMap<String, String>,

    // Track last selected path to avoid re-opening the same file
    last_opened_path: Option<String>,

    // Editor scroll
    editor_scroll_offset: usize,
}

impl WorkspaceView {
    fn new(
        tree_state: Entity<TreeState>,
        search_modal: Entity<SearchModal>,
        terminal_pane: Entity<TerminalPane>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            editor_focus: cx.focus_handle(),
            sidebar_open: true,
            terminal_open: false,
            git_panel_open: false,
            sidebar_width: 240.0,
            terminal_height: 200.0,
            git_panel_width: 280.0,
            project_fs: None,
            workspace: None,
            tree_state,
            git_data: GitPanelData::empty(),
            commit_message: String::new(),
            search_modal,
            search_visible: false,
            terminal_pane,
            diff_text: String::new(),
            git_status_map: HashMap::new(),
            last_opened_path: None,
            editor_scroll_offset: 0,
        }
    }

    // ── Action handlers ─────────────────────────────────────────────

    fn toggle_sidebar(&mut self, _: &ToggleSidebar, _w: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_open = !self.sidebar_open;
        cx.notify();
    }

    fn toggle_terminal(&mut self, _: &ToggleTerminal, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_open = !self.terminal_open;
        if self.terminal_open {
            // Auto-spawn a session if none exist
            self.terminal_pane.update(cx, |pane, cx| {
                if !pane.has_sessions() {
                    pane.spawn_session(cx);
                }
            });
        } else {
            // Return focus to editor
            self.focus_handle.focus(window);
        }
        cx.notify();
    }

    fn toggle_git_panel(&mut self, _: &ToggleGitPanel, _w: &mut Window, cx: &mut Context<Self>) {
        self.git_panel_open = !self.git_panel_open;
        if self.git_panel_open {
            self.refresh_git();
        }
        cx.notify();
    }

    fn quit(&mut self, _: &Quit, _w: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    fn open_folder(&mut self, _: &OpenFolder, _w: &mut Window, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.into_iter().next() {
                    let _ = this.update(cx, |this, cx| {
                        this.load_project(path, cx);
                    });
                }
            }
        })
        .detach();
    }

    fn save(&mut self, _: &Save, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(tab) = ws.tabs.active_tab() {
                let bid = tab.buffer_id;
                if let Err(e) = ws.save_buffer(bid) {
                    eprintln!("Save error: {e}");
                }
            }
        }
        cx.notify();
    }

    fn undo(&mut self, _: &Undo, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                buf.undo();
            }
            if let Some(tab) = ws.tabs.active_tab() {
                ws.reparse_syntax(tab.buffer_id);
            }
        }
        cx.notify();
    }

    fn redo(&mut self, _: &Redo, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                buf.redo();
            }
            if let Some(tab) = ws.tabs.active_tab() {
                ws.reparse_syntax(tab.buffer_id);
            }
        }
        cx.notify();
    }

    fn select_all(&mut self, _: &SelectAll, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                let end_offset = buf.char_count();
                let end_pos = buf.offset_to_pos(end_offset);
                let sel = embd_editor::Selection::range(Position::zero(), end_pos);
                buf.set_selections(SelectionSet::single(sel));
            }
        }
        cx.notify();
    }

    fn copy(&mut self, _: &Copy, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref ws) = self.workspace {
            if let Some(buf) = ws.active_buffer() {
                let sel = buf.selections().primary();
                if !sel.is_cursor() {
                    let (start, end) = sel.ordered();
                    let s = buf.pos_to_offset(start);
                    let e = buf.pos_to_offset(end);
                    if let Ok(text) = buf.slice(s, e) {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                    }
                }
            }
        }
    }

    fn cut(&mut self, _: &Cut, _w: &mut Window, cx: &mut Context<Self>) {
        // Copy then delete selection
        self.copy(&Copy, _w, cx);
        self.delete_selection(cx);
    }

    fn paste(&mut self, _: &Paste, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                let text = text.to_string();
                self.insert_text_at_cursor(&text, cx);
            }
        }
    }

    fn close_tab(&mut self, _: &CloseTab, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(tab) = ws.tabs.active_tab() {
                let path = tab.path.clone();
                ws.close_file(&path);
            }
        }
        self.last_opened_path = None;
        cx.notify();
    }

    fn next_tab(&mut self, _: &NextTab, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            ws.tabs.next_tab();
        }
        cx.notify();
    }

    fn prev_tab(&mut self, _: &PrevTab, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            ws.tabs.prev_tab();
        }
        cx.notify();
    }

    fn toggle_search(&mut self, _: &ToggleSearchModal, window: &mut Window, cx: &mut Context<Self>) {
        self.search_visible = !self.search_visible;
        if self.search_visible {
            self.search_modal.update(cx, |modal, cx| {
                modal.show(window, cx);
            });
        } else {
            self.focus_handle.focus(window);
        }
        cx.notify();
    }

    // ── Editor keyboard input ───────────────────────────────────────

    fn handle_editor_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Only handle keystrokes when the workspace root is focused (not terminal/search)
        if !self.focus_handle.is_focused(window) {
            return;
        }

        let ks = &event.keystroke;

        // Skip modifier combos — let action system handle Cmd+S, etc.
        if ks.modifiers.platform || ks.modifiers.control || ks.modifiers.alt {
            return;
        }

        let shift = ks.modifiers.shift;
        let key = ks.key.as_str();

        match key {
            "backspace" => self.editor_backspace(cx),
            "delete" => self.editor_delete(cx),
            "enter" => self.insert_text_at_cursor("\n", cx),
            "tab" => self.insert_text_at_cursor("  ", cx),
            "up" => self.editor_move_cursor(0, -1, shift, cx),
            "down" => self.editor_move_cursor(0, 1, shift, cx),
            "left" => self.editor_move_cursor(-1, 0, shift, cx),
            "right" => self.editor_move_cursor(1, 0, shift, cx),
            "home" => self.editor_home(shift, cx),
            "end" => self.editor_end(shift, cx),
            "pageup" => self.editor_move_cursor(0, -30, shift, cx),
            "pagedown" => self.editor_move_cursor(0, 30, shift, cx),
            _ => {
                // Printable character
                if let Some(ref ch) = ks.key_char {
                    self.insert_text_at_cursor(ch, cx);
                } else if key.len() == 1 && !key.is_empty() {
                    self.insert_text_at_cursor(key, cx);
                }
            }
        }
    }

    fn insert_text_at_cursor(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                let sel = buf.selections().primary().clone();
                if sel.is_cursor() {
                    let offset = buf.pos_to_offset(sel.head);
                    let _ = buf.insert(offset, text);
                    let new_offset = offset + text.chars().count();
                    let new_pos = buf.offset_to_pos(new_offset);
                    buf.set_selections(SelectionSet::single_at(new_pos));
                } else {
                    let (start, end) = sel.ordered();
                    let s = buf.pos_to_offset(start);
                    let e = buf.pos_to_offset(end);
                    let _ = buf.replace(s, e, text);
                    let new_offset = s + text.chars().count();
                    let new_pos = buf.offset_to_pos(new_offset);
                    buf.set_selections(SelectionSet::single_at(new_pos));
                }
            }
            if let Some(tab) = ws.tabs.active_tab() {
                ws.reparse_syntax(tab.buffer_id);
            }
        }
        cx.notify();
    }

    fn delete_selection(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                let sel = buf.selections().primary().clone();
                if !sel.is_cursor() {
                    let (start, end) = sel.ordered();
                    let s = buf.pos_to_offset(start);
                    let e = buf.pos_to_offset(end);
                    let _ = buf.delete(s, e);
                    buf.set_selections(SelectionSet::single_at(start));
                }
            }
            if let Some(tab) = ws.tabs.active_tab() {
                ws.reparse_syntax(tab.buffer_id);
            }
        }
        cx.notify();
    }

    fn editor_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                let sel = buf.selections().primary().clone();
                if !sel.is_cursor() {
                    let (start, end) = sel.ordered();
                    let s = buf.pos_to_offset(start);
                    let e = buf.pos_to_offset(end);
                    let _ = buf.delete(s, e);
                    buf.set_selections(SelectionSet::single_at(start));
                } else {
                    let offset = buf.pos_to_offset(sel.head);
                    if offset > 0 {
                        let _ = buf.delete(offset - 1, offset);
                        let new_pos = buf.offset_to_pos(offset - 1);
                        buf.set_selections(SelectionSet::single_at(new_pos));
                    }
                }
            }
            if let Some(tab) = ws.tabs.active_tab() {
                ws.reparse_syntax(tab.buffer_id);
            }
        }
        cx.notify();
    }

    fn editor_delete(&mut self, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                let sel = buf.selections().primary().clone();
                if !sel.is_cursor() {
                    let (start, end) = sel.ordered();
                    let s = buf.pos_to_offset(start);
                    let e = buf.pos_to_offset(end);
                    let _ = buf.delete(s, e);
                    buf.set_selections(SelectionSet::single_at(start));
                } else {
                    let offset = buf.pos_to_offset(sel.head);
                    if offset < buf.char_count() {
                        let _ = buf.delete(offset, offset + 1);
                    }
                }
            }
            if let Some(tab) = ws.tabs.active_tab() {
                ws.reparse_syntax(tab.buffer_id);
            }
        }
        cx.notify();
    }

    fn editor_move_cursor(
        &mut self,
        dx: i32,
        dy: i32,
        extend_selection: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                let sel = buf.selections().primary().clone();
                let pos = sel.head;
                let line = pos.line as i32;
                let col = pos.col as i32;

                let new_line = (line + dy).max(0).min(buf.line_count() as i32 - 1) as u32;
                let line_len = buf
                    .line(new_line as usize)
                    .map(|l| l.chars().count() as i32)
                    .unwrap_or(0);
                let new_col = if dy != 0 {
                    // Vertical: try to keep column, clamp to line length
                    col.min(line_len).max(0) as u32
                } else {
                    // Horizontal
                    let new = col + dx;
                    if new < 0 {
                        // Move to end of previous line
                        if new_line > 0 || line > 0 {
                            let prev_line = (line - 1).max(0) as u32;
                            let prev_len = buf
                                .line(prev_line as usize)
                                .map(|l| l.chars().count() as u32)
                                .unwrap_or(0);
                            // Set line and col for previous line end
                            let new_pos = Position::new(prev_line, prev_len);
                            if extend_selection {
                                let sel = embd_editor::Selection::range(sel.anchor, new_pos);
                                buf.set_selections(SelectionSet::single(sel));
                            } else {
                                buf.set_selections(SelectionSet::single_at(new_pos));
                            }
                            cx.notify();
                            return;
                        }
                        0u32
                    } else if new > line_len {
                        // Move to start of next line
                        if (new_line as usize) < buf.line_count() - 1 {
                            let next_pos = Position::new(new_line + 1, 0);
                            if extend_selection {
                                let sel = embd_editor::Selection::range(sel.anchor, next_pos);
                                buf.set_selections(SelectionSet::single(sel));
                            } else {
                                buf.set_selections(SelectionSet::single_at(next_pos));
                            }
                            cx.notify();
                            return;
                        }
                        line_len as u32
                    } else {
                        new as u32
                    }
                };

                let new_pos = Position::new(new_line, new_col);
                if extend_selection {
                    let sel = embd_editor::Selection::range(sel.anchor, new_pos);
                    buf.set_selections(SelectionSet::single(sel));
                } else {
                    buf.set_selections(SelectionSet::single_at(new_pos));
                }
            }
        }
        cx.notify();
    }

    fn editor_home(&mut self, extend: bool, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                let sel = buf.selections().primary().clone();
                let new_pos = Position::new(sel.head.line, 0);
                if extend {
                    buf.set_selections(SelectionSet::single(
                        embd_editor::Selection::range(sel.anchor, new_pos),
                    ));
                } else {
                    buf.set_selections(SelectionSet::single_at(new_pos));
                }
            }
        }
        cx.notify();
    }

    fn editor_end(&mut self, extend: bool, cx: &mut Context<Self>) {
        if let Some(ref mut ws) = self.workspace {
            if let Some(buf) = ws.active_buffer_mut() {
                let sel = buf.selections().primary().clone();
                let line_len = buf
                    .line(sel.head.line as usize)
                    .map(|l| l.chars().count() as u32)
                    .unwrap_or(0);
                let new_pos = Position::new(sel.head.line, line_len);
                if extend {
                    buf.set_selections(SelectionSet::single(
                        embd_editor::Selection::range(sel.anchor, new_pos),
                    ));
                } else {
                    buf.set_selections(SelectionSet::single_at(new_pos));
                }
            }
        }
        cx.notify();
    }

    // ── Project loading ─────────────────────────────────────────────

    fn load_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let fs = match ProjectFs::new(&path) {
            Ok(fs) => fs,
            Err(e) => {
                eprintln!("Failed to open project: {e}");
                return;
            }
        };

        let entries = match fs.read_dir_tree(fs.root(), 3) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Failed to read directory: {e}");
                return;
            }
        };

        let tree_items = file_entries_to_tree_items(&entries);
        self.tree_state.update(cx, |state, cx| {
            state.set_items(tree_items, cx);
        });

        // Set search root
        let root = path.clone();
        self.search_modal.update(cx, |modal, _cx| {
            modal.set_root(root.clone());
        });

        // Set terminal cwd
        let cwd = path.clone();
        self.terminal_pane.update(cx, |pane, _cx| {
            pane.set_cwd(cwd);
        });

        self.project_fs = Some(fs);
        self.workspace = Some(Workspace::new(path, Settings::default()));
        self.last_opened_path = None;

        // Load git status
        self.refresh_git();

        cx.notify();
    }

    fn refresh_git(&mut self) {
        if let Some(ref ws) = self.workspace {
            self.git_data = git_panel::refresh_git(ws.root());
            // Build status map for file tree
            let repo = GitRepo::new(ws.root());
            self.git_status_map = repo.status().unwrap_or_default();
        }
    }

    fn on_tree_changed(&mut self, _: Entity<TreeState>, cx: &mut Context<Self>) {
        let selected = {
            let state = self.tree_state.read(cx);
            state
                .selected_entry()
                .map(|e| (e.item().id.to_string(), e.is_folder()))
        };

        if let Some((path, is_folder)) = selected {
            if !is_folder && self.last_opened_path.as_deref() != Some(&path) {
                self.last_opened_path = Some(path.clone());
                self.open_file(&path, cx);
            }
        }
    }

    fn open_file(&mut self, path_str: &str, cx: &mut Context<Self>) {
        let path = PathBuf::from(path_str);
        if let Some(ref mut ws) = self.workspace {
            match ws.open_file(&path) {
                Ok(_) => {
                    self.editor_scroll_offset = 0;
                    cx.notify();
                }
                Err(e) => eprintln!("Failed to open file: {e}"),
            }
        }
    }

    fn on_search_event(
        &mut self,
        _: Entity<SearchModal>,
        event: &SearchEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            SearchEvent::Open(path) => {
                self.search_visible = false;
                self.open_file(path, cx);
            }
            SearchEvent::Dismiss => {
                self.search_visible = false;
            }
        }
        cx.notify();
    }

    // ── Rendering ───────────────────────────────────────────────────

    fn render_titlebar(&self, _cx: &Context<Self>) -> impl IntoElement {
        let project_name = self
            .workspace
            .as_ref()
            .map(|ws| {
                ws.root()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Project")
                    .to_string()
            })
            .unwrap_or_else(|| "No folder open".to_string());

        div()
            .id("titlebar")
            .h(px(38.0))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .bg(Colors::bg_base())
            .border_b_1()
            .border_color(Colors::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .pl(px(80.0))
                    .child(div().text_sm().text_color(Colors::text_muted()).child("embd")),
            )
            .child(div().text_sm().text_color(Colors::text()).child(project_name))
            .child(div().w(px(80.0)))
    }

    fn render_sidebar(&self, _cx: &Context<Self>) -> impl IntoElement {
        let has_project = self.project_fs.is_some();
        let git_map = self.git_status_map.clone();

        div()
            .id("sidebar")
            .w(px(self.sidebar_width))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(Colors::bg_surface())
            .border_r_1()
            .border_color(Colors::border())
            .child(
                div()
                    .p(px(12.0))
                    .text_xs()
                    .text_color(Colors::text_muted())
                    .child("EXPLORER"),
            )
            .child(if has_project {
                div()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        tree(&self.tree_state, move |ix, entry, selected, _window, _cx| {
                            let item = entry.item();
                            let depth = entry.depth();
                            let is_dir = entry.is_folder();
                            let label = item.label.clone();
                            let path_id = item.id.to_string();

                            // Git status indicator
                            let git_code = git_map.get(&path_id).cloned();
                            let status_char = git_code
                                .as_deref()
                                .map(git_panel::status_label)
                                .unwrap_or("");
                            let status_color = git_code
                                .as_deref()
                                .map(git_panel::status_color)
                                .unwrap_or(Colors::text());

                            let prefix = if is_dir {
                                if entry.is_expanded() { "▾ " } else { "▸ " }
                            } else {
                                "  "
                            };

                            ListItem::new(ix)
                                .selected(selected)
                                .pl(px(16.0 * depth as f32 + 8.0))
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(4.0))
                                        .items_center()
                                        .text_sm()
                                        .text_color(if is_dir {
                                            Colors::text()
                                        } else if git_code.is_some() {
                                            status_color
                                        } else {
                                            Colors::text_muted()
                                        })
                                        .child(prefix)
                                        .child(label)
                                        .when(!status_char.is_empty() && !is_dir, |d: Div| {
                                            d.child(
                                                div()
                                                    .ml_auto()
                                                    .text_xs()
                                                    .text_color(status_color)
                                                    .child(status_char.to_string()),
                                            )
                                        }),
                                )
                        }),
                    )
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .p(px(8.0))
                    .text_sm()
                    .text_color(Colors::text_muted())
                    .child("Cmd+O to open a folder")
                    .into_any_element()
            })
    }

    fn render_tab_bar(&self, _cx: &Context<Self>) -> impl IntoElement {
        let tabs: Vec<_> = self
            .workspace
            .as_ref()
            .map(|ws| {
                ws.tabs
                    .tabs()
                    .iter()
                    .enumerate()
                    .map(|(i, tab)| {
                        let is_active = ws.tabs.active_index() == Some(i);
                        (tab.name.clone(), is_active, tab.modified, tab.pinned, i)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let has_tabs = !tabs.is_empty();

        div()
            .h(px(35.0))
            .w_full()
            .bg(Colors::bg_surface())
            .border_b_1()
            .border_color(Colors::border())
            .flex()
            .items_center()
            .px(px(4.0))
            .text_sm()
            .children(if has_tabs {
                tabs.into_iter()
                    .map(|(name, is_active, modified, pinned, _idx)| {
                        let label = if modified {
                            format!("{} ●", name)
                        } else if pinned {
                            format!("📌 {}", name)
                        } else {
                            name.clone()
                        };
                        div()
                            .id(SharedString::from(format!("tab-{}", name)))
                            .px(px(12.0))
                            .py(px(6.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(Colors::surface_hover()))
                            .text_color(if is_active {
                                Colors::text()
                            } else {
                                Colors::text_muted()
                            })
                            .when(is_active, |d| {
                                d.border_b_2().border_color(Colors::accent())
                            })
                            .child(label)
                            .when(!pinned, |d| {
                                d.child(
                                    div()
                                        .text_xs()
                                        .text_color(Colors::text_muted())
                                        .hover(|s| s.text_color(Colors::text()))
                                        .child("×"),
                                )
                            })
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![div()
                    .text_color(Colors::text_muted())
                    .child("No files open")
                    .into_any_element()]
            })
    }

    fn render_editor_area(&self, cx: &Context<Self>) -> impl IntoElement {
        let focus = self.focus_handle.clone();
        div()
            .id("editor-area")
            .flex_1()
            .h_full()
            .bg(Colors::bg_base())
            .flex()
            .flex_col()
            .on_click(move |_ev, window, _cx| {
                focus.focus(window);
            })
            .child(self.render_tab_bar(cx))
            .child(self.render_buffer_content(cx))
    }

    fn render_buffer_content(&self, _cx: &Context<Self>) -> impl IntoElement {
        let ws = match self.workspace.as_ref() {
            Some(ws) => ws,
            None => {
                return div()
                    .id("editor-content")
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(Colors::text_muted())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(8.0))
                            .child(div().text_xl().child("embd"))
                            .child(div().text_sm().child("Cmd+O to open a folder")),
                    )
                    .into_any_element();
            }
        };

        let active_tab = match ws.tabs.active_tab() {
            Some(tab) => tab,
            None => {
                return div()
                    .id("editor-content")
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(Colors::text_muted())
                    .child("Select a file to view")
                    .into_any_element();
            }
        };

        let buffer_id = active_tab.buffer_id;
        let buffer = match ws.buffer(buffer_id) {
            Some(b) => b,
            None => {
                return div()
                    .id("editor-content")
                    .flex_1()
                    .child("Buffer not found")
                    .into_any_element();
            }
        };

        let line_count = buffer.line_count();
        let highlights = ws.highlights(buffer_id, 0, line_count);

        // Cursor position
        let cursor_pos = {
            let sel = buffer.selections().primary();
            (sel.head.line as usize, sel.head.col as usize)
        };
        let selection = {
            let sel = buffer.selections().primary();
            if sel.is_cursor() {
                None
            } else {
                let (s, e) = sel.ordered();
                Some((
                    (s.line as usize, s.col as usize),
                    (e.line as usize, e.col as usize),
                ))
            }
        };

        let mut lines_elements: Vec<AnyElement> = Vec::with_capacity(line_count);

        for line_idx in 0..line_count {
            let line_text = buffer.line(line_idx).unwrap_or_default();
            let line_start_byte = buffer.line_to_byte(line_idx);
            let line_end_byte = line_start_byte + line_text.len();

            let line_highlights: Vec<_> = highlights
                .iter()
                .filter(|h| h.byte_range.start < line_end_byte && h.byte_range.end > line_start_byte)
                .collect();

            let line_num = format!("{:>4}", line_idx + 1);
            let is_cursor_line = cursor_pos.0 == line_idx;

            // Build text spans with highlighting and cursor
            let text_element = build_line_spans(
                &line_text,
                line_start_byte,
                &line_highlights,
                if is_cursor_line { Some(cursor_pos.1) } else { None },
                selection.as_ref(),
                line_idx,
            );

            let content = div()
                .flex()
                .when(is_cursor_line, |d: Div| d.bg(hsla(0.0, 0.0, 1.0, 0.03)))
                .child(
                    div()
                        .w(px(48.0))
                        .flex_shrink_0()
                        .text_color(if is_cursor_line {
                            Colors::text()
                        } else {
                            Colors::text_muted()
                        })
                        .text_right()
                        .pr(px(12.0))
                        .child(line_num),
                )
                .child(
                    div().flex_1().flex().children(text_element),
                );

            lines_elements.push(content.into_any_element());
        }

        div()
            .id("editor-content")
            .flex_1()
            .overflow_y_scroll()
            .bg(Colors::bg_base())
            .py(px(4.0))
            .text_sm()
            .font_family("monospace")
            .children(lines_elements)
            .into_any_element()
    }

    fn render_terminal_panel(&self, _cx: &Context<Self>) -> impl IntoElement {
        div()
            .id("terminal-panel")
            .h(px(self.terminal_height))
            .w_full()
            .flex_shrink_0()
            .border_t_1()
            .border_color(Colors::border())
            .child(self.terminal_pane.clone())
    }

    fn render_status_bar(&self, _cx: &Context<Self>) -> impl IntoElement {
        let (left_info, right_info) = if let Some(ref ws) = self.workspace {
            let branch = self.git_data.branch.as_deref().unwrap_or("");
            let branch_text = if branch.is_empty() {
                String::new()
            } else {
                format!("⎇ {}", branch)
            };

            let file_info = ws
                .active_buffer()
                .map(|buf| {
                    let sel = buf.selections().primary();
                    let lang = ws
                        .tabs
                        .active_tab()
                        .and_then(|t| ws.language_name(t.buffer_id))
                        .unwrap_or("Plain Text");
                    let saved = if buf.is_modified() { "●" } else { "✓" };
                    format!(
                        "Ln {}, Col {}  {}  UTF-8  {}",
                        sel.head.line + 1,
                        sel.head.col + 1,
                        lang,
                        saved
                    )
                })
                .unwrap_or_else(|| "Ready".to_string());

            (branch_text, file_info)
        } else {
            (String::new(), "Ready".to_string())
        };

        div()
            .id("status-bar")
            .h(px(24.0))
            .w_full()
            .flex_shrink_0()
            .bg(Colors::bg_surface())
            .border_t_1()
            .border_color(Colors::border())
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.0))
            .text_xs()
            .text_color(Colors::text_muted())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(left_info),
            )
            .child(right_info)
    }
}

// ── Render impl ─────────────────────────────────────────────────────

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus management: only grab focus for the workspace root if
        // neither search modal nor terminal should have it.
        // Search modal and terminal manage their own focus handles.
        if !self.search_visible && !self.terminal_open {
            self.focus_handle.focus(window);
        }

        let mut root = div()
            .id("workspace")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(Colors::bg_base())
            .text_color(Colors::text())
            // Global actions
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_terminal))
            .on_action(cx.listener(Self::toggle_git_panel))
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::prev_tab))
            .on_action(cx.listener(Self::toggle_search))
            // Editor key input
            .on_key_down(cx.listener(Self::handle_editor_key_down))
            // Titlebar
            .child(self.render_titlebar(cx))
            // Main content area
            .child(
                div()
                    .flex_1()
                    .flex()
                    .min_h_0()
                    .child({
                        let mut row = div().flex_1().flex().min_h_0();

                        // Sidebar
                        if self.sidebar_open {
                            row = row.child(self.render_sidebar(cx));
                        }

                        // Editor + terminal stack
                        let mut col = div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .child(self.render_editor_area(cx));
                        if self.terminal_open {
                            col = col.child(self.render_terminal_panel(cx));
                        }
                        row = row.child(col);

                        // Git panel
                        if self.git_panel_open {
                            let root = self
                                .workspace
                                .as_ref()
                                .map(|ws| ws.root().to_path_buf())
                                .unwrap_or_default();
                            row = row.child(git_panel::render_git_panel(
                                &self.git_data,
                                &self.commit_message,
                                self.git_panel_width,
                                &root,
                                &self.diff_text,
                            ));
                        }

                        row
                    }),
            )
            // Status bar
            .child(self.render_status_bar(cx));

        // Search modal overlay
        if self.search_visible {
            root = root.child(self.search_modal.clone());
        }

        root
    }
}

// ── Helpers ─────────────────────────────────────────────────────────

fn file_entries_to_tree_items(entries: &[FileEntry]) -> Vec<TreeItem> {
    entries
        .iter()
        .map(|entry| {
            let mut item = TreeItem::new(entry.path.clone(), entry.name.clone());
            if let Some(ref children) = entry.children {
                let child_items = file_entries_to_tree_items(children);
                item = item.children(child_items);
            }
            item
        })
        .collect()
}

fn capture_name_to_color(name: &str) -> Hsla {
    match name {
        "keyword" => Colors::accent(),
        "string" | "string.special" => Colors::success(),
        "comment" => Colors::text_muted(),
        "number" | "float" => Colors::warning(),
        "function" | "function.call" => Colors::accent(),
        "type" | "type.builtin" => Colors::warning(),
        "constant.builtin" => Colors::warning(),
        "variable.builtin" => Colors::error(),
        "attribute" | "decorator" => Colors::warning(),
        "tag" => Colors::error(),
        "property" | "field_identifier" => Colors::text(),
        "operator" => Colors::text(),
        _ => Colors::text(),
    }
}

/// Build highlighted text spans for a single line, including cursor.
fn build_line_spans(
    line_text: &str,
    line_start_byte: usize,
    highlights: &[&embd_editor::HighlightSpan],
    cursor_col: Option<usize>,
    selection: Option<&((usize, usize), (usize, usize))>,
    line_idx: usize,
) -> Vec<AnyElement> {
    if line_text.is_empty() {
        // Empty line — just show cursor if on this line
        if cursor_col.is_some() {
            return vec![div()
                .w(px(2.0))
                .h(px(18.0))
                .bg(Colors::accent())
                .into_any_element()];
        }
        return vec![div().child(" ").into_any_element()];
    }

    let chars: Vec<char> = line_text.chars().collect();
    let mut spans: Vec<AnyElement> = Vec::new();

    // Simple approach: render character by character for cursor accuracy
    // Group consecutive characters with same style for efficiency
    let mut i = 0;
    while i < chars.len() {
        let byte_offset = line_start_byte + line_text[..chars[..i].iter().collect::<String>().len()].len();

        // Find highlight color for this character
        let color = highlights
            .iter()
            .find(|h| h.byte_range.start <= byte_offset && h.byte_range.end > byte_offset)
            .map(|h| capture_name_to_color(&h.capture_name))
            .unwrap_or(Colors::text());

        // Check if character is in selection
        let in_selection = selection
            .map(|((sl, sc), (el, ec))| {
                if line_idx < *sl || line_idx > *el {
                    false
                } else if line_idx == *sl && line_idx == *el {
                    i >= *sc && i < *ec
                } else if line_idx == *sl {
                    i >= *sc
                } else if line_idx == *el {
                    i < *ec
                } else {
                    true
                }
            })
            .unwrap_or(false);

        // Group consecutive chars with same color and selection state
        let start_i = i;
        let mut group_end = i + 1;
        while group_end < chars.len() {
            let gb = line_start_byte + chars[..group_end].iter().collect::<String>().len();
            let gc = highlights
                .iter()
                .find(|h| h.byte_range.start <= gb && h.byte_range.end > gb)
                .map(|h| capture_name_to_color(&h.capture_name))
                .unwrap_or(Colors::text());

            let g_in_sel = selection
                .map(|((sl, sc), (el, ec))| {
                    if line_idx < *sl || line_idx > *el {
                        false
                    } else if line_idx == *sl && line_idx == *el {
                        group_end >= *sc && group_end < *ec
                    } else if line_idx == *sl {
                        group_end >= *sc
                    } else if line_idx == *el {
                        group_end < *ec
                    } else {
                        true
                    }
                })
                .unwrap_or(false);

            // Break if cursor is between start_i and group_end
            if let Some(cc) = cursor_col {
                if cc > start_i && cc <= group_end {
                    group_end = cc;
                    break;
                }
            }

            if gc != color || g_in_sel != in_selection {
                break;
            }
            group_end += 1;
        }

        let text: String = chars[start_i..group_end].iter().collect();

        // Insert cursor before this group if needed
        if let Some(cc) = cursor_col {
            if cc == start_i {
                spans.push(
                    div()
                        .w(px(2.0))
                        .h(px(18.0))
                        .bg(Colors::accent())
                        .flex_shrink_0()
                        .into_any_element(),
                );
            }
        }

        let mut span = div().text_color(color);
        if in_selection {
            span = span.bg(hsla(0.58, 0.7, 0.5, 0.3));
        }
        spans.push(span.child(text).into_any_element());

        i = group_end;
    }

    // Cursor at end of line
    if let Some(cc) = cursor_col {
        if cc >= chars.len() {
            spans.push(
                div()
                    .w(px(2.0))
                    .h(px(18.0))
                    .bg(Colors::accent())
                    .flex_shrink_0()
                    .into_any_element(),
            );
        }
    }

    spans
}

// ── Public entry point ──────────────────────────────────────────────

pub fn build_workspace(_window: &mut Window, app: &mut App) -> Entity<WorkspaceView> {
    gpui_component::init(app);

    app.bind_keys([
        // Workspace
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-j", ToggleTerminal, None),
        KeyBinding::new("cmd-o", OpenFolder, None),
        KeyBinding::new("cmd-g", ToggleGitPanel, None),
        KeyBinding::new("cmd-p", ToggleSearchModal, None),
        // Editor
        KeyBinding::new("cmd-s", Save, None),
        KeyBinding::new("cmd-z", Undo, None),
        KeyBinding::new("cmd-shift-z", Redo, None),
        KeyBinding::new("cmd-a", SelectAll, None),
        KeyBinding::new("cmd-c", Copy, None),
        KeyBinding::new("cmd-x", Cut, None),
        KeyBinding::new("cmd-v", Paste, None),
        // Tabs
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PrevTab, None),
    ]);

    let tree_state = app.new(|cx| TreeState::new(cx));
    let search_modal = app.new(|cx| SearchModal::new(cx));
    let terminal_pane = app.new(|cx| TerminalPane::new(cx));

    let ts = tree_state.clone();
    let sm = search_modal.clone();
    let tp = terminal_pane.clone();
    let view = app.new(|cx| {
        cx.observe(&ts, WorkspaceView::on_tree_changed).detach();
        cx.subscribe(&sm, WorkspaceView::on_search_event).detach();
        WorkspaceView::new(ts, sm, tp, cx)
    });

    view
}
