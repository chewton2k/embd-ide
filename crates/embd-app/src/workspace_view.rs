use std::collections::HashMap;
use std::path::PathBuf;

use gpui::*;
use gpui::prelude::FluentBuilder as _;
use gpui_component::Root;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::resizable::{h_resizable, v_resizable, resizable_panel};
use gpui_component::list::ListItem;
use gpui_component::tree::{tree, TreeItem, TreeState};

use embd_platform::fs::{FileEntry, ProjectFs};
use embd_platform::git::GitRepo;
use embd_platform::search::find_files;

use crate::git_panel::{self, GitPanelData};
use crate::terminal::{TerminalEvent, TerminalPane};
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
        CloseTab,
        NextTab,
        PrevTab,
        ToggleSearchModal,
    ]
);

// ── Workspace root view ─────────────────────────────────────────────

pub struct WorkspaceView {
    focus_handle: FocusHandle,
    sidebar_open: bool,
    terminal_open: bool,
    git_panel_open: bool,

    // Project state
    project_fs: Option<ProjectFs>,
    project_root: Option<PathBuf>,
    tree_state: Entity<TreeState>,

    // Editor: one InputState per open file, keyed by path
    editor_states: HashMap<String, Entity<InputState>>,
    active_file: Option<String>,
    tab_order: Vec<String>, // ordered list of open file paths
    modified_files: HashMap<String, bool>,

    // Git state
    git_data: GitPanelData,
    commit_input: Entity<InputState>,

    // Search
    search_input: Entity<InputState>,
    search_visible: bool,
    search_results: Vec<embd_platform::search::FileMatch>,
    search_selected: usize,

    // Terminal
    terminal_pane: Entity<TerminalPane>,

    // Git diff preview text
    diff_text: String,
    git_status_map: HashMap<String, String>,

    // Track last selected tree path
    last_opened_path: Option<String>,

    // Pending file to load (deferred to render for window access)
    pending_open: Option<(String, String)>, // (path, content)

    // Search debouncing: only apply results from the latest query
    search_generation: u64,
}

impl WorkspaceView {
    fn new(
        tree_state: Entity<TreeState>,
        search_input: Entity<InputState>,
        commit_input: Entity<InputState>,
        terminal_pane: Entity<TerminalPane>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            sidebar_open: true,
            terminal_open: false,
            git_panel_open: false,
            project_fs: None,
            project_root: None,
            tree_state,
            editor_states: HashMap::new(),
            active_file: None,
            tab_order: Vec::new(),
            modified_files: HashMap::new(),
            git_data: GitPanelData::empty(),
            commit_input,
            search_input,
            search_visible: false,
            search_results: Vec::new(),
            search_selected: 0,
            terminal_pane,
            diff_text: String::new(),
            git_status_map: HashMap::new(),
            last_opened_path: None,
            pending_open: None,
            search_generation: 0,
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
            self.terminal_pane.update(cx, |pane, cx| {
                if !pane.has_sessions() {
                    pane.spawn_session(cx);
                }
                pane.focus(window);
            });
        } else {
            self.focus_handle.focus(window);
        }
        cx.notify();
    }

    fn toggle_git_panel(&mut self, _: &ToggleGitPanel, _w: &mut Window, cx: &mut Context<Self>) {
        self.git_panel_open = !self.git_panel_open;
        if self.git_panel_open {
            self.refresh_git(cx);
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
        if let (Some(ref path), Some(ref _root)) = (self.active_file.clone(), self.project_root.clone()) {
            if let Some(state) = self.editor_states.get(path) {
                let content = state.read(cx).value().to_string();
                // Route through ProjectFs when available for path validation
                let result = if let Some(ref fs) = self.project_fs {
                    fs.write_file(path, &content).map_err(|e| e.to_string())
                } else {
                    std::fs::write(path, &content).map_err(|e| e.to_string())
                };
                match result {
                    Ok(()) => {
                        self.modified_files.insert(path.clone(), false);
                    }
                    Err(e) => eprintln!("Save error: {e}"),
                }
            }
        }
        cx.notify();
    }

    fn close_tab(&mut self, _: &CloseTab, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref path) = self.active_file.clone() {
            self.tab_order.retain(|p| p != path);
            self.editor_states.remove(path);
            self.modified_files.remove(path);
            self.active_file = self.tab_order.last().cloned();
        }
        self.last_opened_path = None;
        cx.notify();
    }

    fn next_tab(&mut self, _: &NextTab, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref active) = self.active_file {
            if let Some(idx) = self.tab_order.iter().position(|p| p == active) {
                let next = (idx + 1) % self.tab_order.len();
                self.active_file = Some(self.tab_order[next].clone());
            }
        }
        cx.notify();
    }

    fn prev_tab(&mut self, _: &PrevTab, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(ref active) = self.active_file {
            if let Some(idx) = self.tab_order.iter().position(|p| p == active) {
                let prev = if idx == 0 { self.tab_order.len() - 1 } else { idx - 1 };
                self.active_file = Some(self.tab_order[prev].clone());
            }
        }
        cx.notify();
    }

    fn toggle_search(&mut self, _: &ToggleSearchModal, window: &mut Window, cx: &mut Context<Self>) {
        self.search_visible = !self.search_visible;
        if self.search_visible {
            self.search_results.clear();
            self.search_selected = 0;
            self.search_input.update(cx, |state, cx| {
                state.set_value("", window, cx);
                state.focus(window, cx);
            });
        } else {
            self.focus_handle.focus(window);
        }
        cx.notify();
    }

    // ── Search key handling ─────────────────────────────────────────

    fn handle_search_keys(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.search_visible {
            return;
        }

        let key = event.keystroke.key.as_str();
        match key {
            "escape" => {
                self.search_visible = false;
                self.focus_handle.focus(window);
                cx.notify();
            }
            "enter" => {
                if let Some(result) = self.search_results.get(self.search_selected) {
                    if let Some(ref root) = self.project_root {
                        let abs = root.join(&result.relative_path);
                        let path = abs.to_string_lossy().to_string();
                        self.search_visible = false;
                        self.focus_handle.focus(window);
                        self.open_file_deferred(&path);
                        cx.notify();
                    }
                }
            }
            "up" => {
                if self.search_selected > 0 {
                    self.search_selected -= 1;
                }
                cx.notify();
            }
            "down" => {
                if self.search_selected + 1 < self.search_results.len() {
                    self.search_selected += 1;
                }
                cx.notify();
            }
            _ => {}
        }
    }

    fn on_search_changed(&mut self, _: Entity<InputState>, _: &InputEvent, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().to_string();

        if query.is_empty() {
            self.search_results.clear();
            self.search_selected = 0;
            cx.notify();
            return;
        }

        let Some(root) = self.project_root.clone() else {
            return;
        };

        // Bump generation so stale results from earlier keystrokes are discarded
        self.search_generation += 1;
        let generation = self.search_generation;

        let (tx, rx) = flume::bounded(1);
        std::thread::spawn(move || {
            let results = find_files(&root, &query, 50).unwrap_or_default();
            let _ = tx.send(results);
        });

        cx.spawn(async move |this, cx| {
            if let Ok(results) = rx.recv_async().await {
                let _ = this.update(cx, |this, cx| {
                    if this.search_generation == generation {
                        this.search_results = results;
                        this.search_selected = 0;
                        cx.notify();
                    }
                });
            }
        })
        .detach();
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

        // Set terminal cwd
        let cwd = path.clone();
        self.terminal_pane.update(cx, |pane, _cx| {
            pane.set_cwd(cwd);
        });

        self.project_fs = Some(fs);
        self.project_root = Some(path);
        self.editor_states.clear();
        self.tab_order.clear();
        self.active_file = None;
        self.modified_files.clear();
        self.last_opened_path = None;

        self.refresh_git(cx);
        cx.notify();
    }

    fn refresh_git(&mut self, cx: &mut Context<Self>) {
        let Some(root) = self.project_root.clone() else { return };

        let (tx, rx) = flume::bounded(1);
        std::thread::spawn(move || {
            let git_data = git_panel::refresh_git(&root);
            let repo = GitRepo::new(&root);
            let status_map = repo.status().unwrap_or_default();
            let _ = tx.send((git_data, status_map));
        });

        cx.spawn(async move |this, cx| {
            if let Ok((git_data, status_map)) = rx.recv_async().await {
                let _ = this.update(cx, |this, cx| {
                    this.git_data = git_data;
                    this.git_status_map = status_map;
                    cx.notify();
                });
            }
        })
        .detach();
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
                self.open_file_deferred(&path);
            }
        }
    }

    /// Queue a file open — the actual InputState creation happens in render()
    /// where we have window access.
    fn open_file_deferred(&mut self, path: &str) {
        if self.editor_states.contains_key(path) {
            // Already open, just switch to it
            self.active_file = Some(path.to_string());
            return;
        }

        // Check file size before reading (10 MB limit)
        const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
        match std::fs::metadata(path) {
            Ok(meta) if meta.len() > MAX_FILE_SIZE => {
                eprintln!("File too large to open ({} bytes): {}", meta.len(), path);
                return;
            }
            Err(e) => {
                eprintln!("Failed to open file: {e}");
                return;
            }
            _ => {}
        }

        // Read file content
        match std::fs::read_to_string(path) {
            Ok(content) => {
                self.pending_open = Some((path.to_string(), content));
            }
            Err(e) => eprintln!("Failed to open file: {e}"),
        }
    }

    /// Called in render() to process pending file opens (needs window access)
    fn process_pending_open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((path, content)) = self.pending_open.take() {
            let lang = language_from_path(&path).unwrap_or_else(|| SharedString::from("text"));
            let state = cx.new(|cx| {
                InputState::new(window, cx)
                    .multi_line(true)
                    .soft_wrap(false)
                    .code_editor(lang)
                    .line_number(true)
            });
            state.update(cx, |s, cx| {
                s.set_value(&content, window, cx);
            });

            // Subscribe to changes to track modified state
            let path_clone = path.clone();
            cx.subscribe(&state, move |this: &mut Self, _, _event: &InputEvent, cx| {
                this.modified_files.insert(path_clone.clone(), true);
                cx.notify();
            })
            .detach();

            if !self.tab_order.contains(&path) {
                self.tab_order.push(path.clone());
            }
            self.editor_states.insert(path.clone(), state);
            self.active_file = Some(path);
            cx.notify();
        }
    }

    // ── Rendering ───────────────────────────────────────────────────

    fn render_titlebar(&self) -> impl IntoElement {
        let project_name = self
            .project_root
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("embd")
            .to_string();

        div()
            .id("titlebar")
            .h(px(36.0))
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(Colors::bg_base())
            .border_b_1()
            .border_color(Colors::border_subtle())
            .on_mouse_down(MouseButton::Left, |_: &MouseDownEvent, window, _cx| {
                window.start_window_move();
            })
            .on_click(|event: &ClickEvent, window, _cx| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
            // Centered project name — clean, minimal
            .child(
                div()
                    .text_xs()
                    .text_color(Colors::text_faint())
                    .child(project_name),
            )
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_project = self.project_fs.is_some();
        let git_map = self.git_status_map.clone();

        let project_name = self
            .project_root
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Open Folder")
            .to_string();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(Colors::bg_surface())
            .border_r_1()
            .border_color(Colors::border())
            // Project header — click to open a different folder
            .child(
                div()
                    .id("sidebar-project-header")
                    .w_full()
                    .flex()
                    .items_center()
                    .px(px(14.0))
                    .h(px(36.0))
                    .border_b_1()
                    .border_color(Colors::border_subtle())
                    .cursor_pointer()
                    .hover(|s| s.bg(Colors::surface_hover()))
                    .on_click(cx.listener(|this, _, _w, cx| {
                        this.open_folder(&OpenFolder, _w, cx);
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(Colors::text_faint())
                                    .child("EXPLORER"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Colors::text_faint())
                                    .child("·"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Colors::text_muted())
                                    .child(project_name),
                            ),
                    ),
            )
            .child(if has_project {
                div()
                    .flex_1()
                    .overflow_hidden()
                    .pt(px(4.0))
                    .child(
                        tree(&self.tree_state, move |ix, entry, selected, _window, _cx| {
                            let item = entry.item();
                            let depth = entry.depth();
                            let is_dir = entry.is_folder();
                            let label = item.label.clone();
                            let path_id = item.id.to_string();

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
                                .pl(px(12.0 * depth as f32 + 10.0))
                                .child(
                                    div()
                                        .flex()
                                        .gap(px(4.0))
                                        .items_center()
                                        .text_xs()
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
                                                    .pr(px(8.0))
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
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(Colors::text_faint())
                    .child("Cmd+O to open a folder")
                    .into_any_element()
            })
    }

    fn render_search_overlay(&self, _cx: &Context<Self>) -> impl IntoElement {
        let max_visible = 12;

        div()
            .id("search-overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.5))
            .flex()
            .flex_col()
            .items_center()
            .pt(px(60.0))
            .child(
                div()
                    .id("search-modal")
                    .w(px(480.0))
                    .max_h(px(380.0))
                    .bg(Colors::bg_elevated())
                    .border_1()
                    .border_color(Colors::border())
                    .rounded(px(6.0))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    // Search input
                    .child(
                        div()
                            .px(px(14.0))
                            .py(px(10.0))
                            .border_b_1()
                            .border_color(Colors::border_subtle())
                            .child(
                                Input::new(&self.search_input)
                                    .appearance(false)
                                    .bordered(false),
                            ),
                    )
                    // Results
                    .child(
                        div()
                            .id("search-results")
                            .flex_1()
                            .overflow_y_scroll()
                            .py(px(4.0))
                            .children(
                                self.search_results
                                    .iter()
                                    .take(max_visible)
                                    .enumerate()
                                    .map(|(ix, result)| {
                                        let is_selected = ix == self.search_selected;
                                        let filename = result
                                            .relative_path
                                            .rsplit('/')
                                            .next()
                                            .unwrap_or(&result.relative_path)
                                            .to_string();
                                        let dir_path = if result.relative_path.contains('/') {
                                            result.relative_path
                                                [..result.relative_path.len() - filename.len()]
                                                .trim_end_matches('/')
                                                .to_string()
                                        } else {
                                            String::new()
                                        };

                                        div()
                                            .px(px(14.0))
                                            .py(px(5.0))
                                            .mx(px(4.0))
                                            .rounded(px(4.0))
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .when(is_selected, |d| d.bg(Colors::bg_overlay()))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Colors::text())
                                                    .child(filename),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Colors::text_faint())
                                                    .child(dir_path),
                                            )
                                    }),
                            ),
                    )
                    // Footer
                    .child(
                        div()
                            .px(px(14.0))
                            .py(px(6.0))
                            .border_t_1()
                            .border_color(Colors::border_subtle())
                            .text_xs()
                            .text_color(Colors::text_faint())
                            .child(format!(
                                "{} files  ↑↓ navigate  ⏎ open  esc close",
                                self.search_results.len()
                            )),
                    ),
            )
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let branch = self.git_data.branch.as_deref().unwrap_or("");
        let branch_text = if branch.is_empty() {
            String::new()
        } else {
            format!("⎇ {}", branch)
        };

        let file_info = if let Some(ref path) = self.active_file {
            let lang = language_from_path(path).unwrap_or("Plain Text".into());
            let modified = self.modified_files.get(path).copied().unwrap_or(false);
            if let Some(state) = self.editor_states.get(path) {
                let pos = state.read(cx).cursor_position();
                let mut parts = format!(
                    "Ln {}, Col {}  {}  UTF-8",
                    pos.line + 1,
                    pos.character + 1,
                    lang,
                );
                if modified {
                    parts.push_str("  ●");
                }
                parts
            } else {
                format!("{}  UTF-8", lang)
            }
        } else {
            String::new()
        };

        let sidebar_active = self.sidebar_open;
        let terminal_active = self.terminal_open;
        let git_active = self.git_panel_open;

        let status_button = |id: &'static str, label: &'static str, active: bool| {
            div()
                .id(id)
                .px(px(8.0))
                .h_full()
                .flex()
                .items_center()
                .cursor_pointer()
                .text_color(if active {
                    Colors::text_muted()
                } else {
                    Colors::text_faint()
                })
                .when(active, |d| d.border_b_1().border_color(Colors::accent_dim()))
                .hover(|s| s.text_color(Colors::text_muted()))
                .child(label)
        };

        div()
            .id("status-bar")
            .h(px(24.0))
            .w_full()
            .flex_shrink_0()
            .bg(Colors::bg_surface())
            .border_t_1()
            .border_color(Colors::border_subtle())
            .flex()
            .items_center()
            .justify_between()
            .px(px(6.0))
            .text_xs()
            .text_color(Colors::text_faint())
            // Left side: toggle buttons
            .child(
                div()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        status_button("sb-explorer", "Explorer", sidebar_active)
                            .on_click(cx.listener(|this, _, _w, cx| {
                                this.sidebar_open = !this.sidebar_open;
                                cx.notify();
                            })),
                    )
                    .child(
                        status_button("sb-terminal", "Terminal", terminal_active)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_terminal(&ToggleTerminal, window, cx);
                            })),
                    )
                    .child(
                        status_button("sb-git", "Git", git_active)
                            .on_click(cx.listener(|this, _, _w, cx| {
                                this.git_panel_open = !this.git_panel_open;
                                if this.git_panel_open {
                                    this.refresh_git(cx);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        status_button("sb-search", "Search", false)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_search(&ToggleSearchModal, window, cx);
                            })),
                    )
                    .when(!branch_text.is_empty(), |d| {
                        d.child(
                            div()
                                .pl(px(8.0))
                                .text_color(Colors::text_faint())
                                .child(branch_text.clone()),
                        )
                    }),
            )
            // Right side: file info
            .child(
                div()
                    .text_color(Colors::text_faint())
                    .child(file_info),
            )
    }
}

// ── Render impl ─────────────────────────────────────────────────────

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Process any pending file opens (needs window for InputState creation)
        self.process_pending_open(window, cx);

        // Focus management — don't steal focus when search overlay is open
        if !self.search_visible && !self.focus_handle.contains_focused(window, cx) {
            self.focus_handle.focus(window);
        }

        // Tab click handler (we need this in render since TabBar::on_click
        // doesn't give us mutable access to self)
        // Instead, render tab bar manually with click handlers
        let tab_bar = if self.tab_order.is_empty() {
            div()
                .h(px(32.0))
                .w_full()
                .bg(Colors::bg_surface())
                .border_b_1()
                .border_color(Colors::border_subtle())
                .into_any_element()
        } else {
            let active_idx = self
                .active_file
                .as_ref()
                .and_then(|a| self.tab_order.iter().position(|p| p == a));

            div()
                .h(px(32.0))
                .w_full()
                .bg(Colors::bg_surface())
                .border_b_1()
                .border_color(Colors::border_subtle())
                .flex()
                .items_end()
                .px(px(4.0))
                .text_xs()
                .children(self.tab_order.iter().enumerate().map(|(i, path)| {
                    let is_active = active_idx == Some(i);
                    let name = PathBuf::from(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("untitled")
                        .to_string();
                    let modified = self.modified_files.get(path).copied().unwrap_or(false);

                    div()
                        .id(SharedString::from(format!("tab-{}", i)))
                        .px(px(12.0))
                        .py(px(6.0))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .hover(|s| s.bg(Colors::surface_hover()))
                        .text_color(if is_active {
                            Colors::text()
                        } else {
                            Colors::text_faint()
                        })
                        .when(is_active, |d| {
                            d.bg(Colors::bg_base())
                                .rounded_t(px(4.0))
                        })
                        .child(name)
                        .when(modified, |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(Colors::text_faint())
                                    .child("●"),
                            )
                        })
                        .on_click(cx.listener(move |this, _, _w, cx| {
                            if let Some(path) = this.tab_order.get(i) {
                                this.active_file = Some(path.clone());
                                cx.notify();
                            }
                        }))
                        .into_any_element()
                }))
                .into_any_element()
        };

        // Editor content
        let editor_content = if let Some(ref path) = self.active_file {
            if let Some(state) = self.editor_states.get(path) {
                div()
                    .size_full()
                    .child(
                        Input::new(state)
                            .appearance(false)
                            .bordered(false)
                            .h_full(),
                    )
                    .into_any_element()
            } else {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(Colors::text_muted())
                    .child("Loading...")
                    .into_any_element()
            }
        } else if self.project_root.is_some() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(Colors::text_faint())
                .child("Select a file to open")
                .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(Colors::text_muted())
                                .child("embd"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(Colors::text_faint())
                                .child("Cmd+O to open a folder"),
                        ),
                )
                .into_any_element()
        };

        // Editor area: tabs + scrollable code
        let editor_area = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(Colors::bg_base())
            .child(tab_bar)
            .child(
                div()
                    .id("editor-content")
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .min_w_0()
                    .child(editor_content),
            );

        // Build the editor column (tabs + editor + optional terminal)
        let editor_col = if self.terminal_open {
            v_resizable("editor-terminal-split")
                .child(resizable_panel().child(editor_area))
                .child(
                    resizable_panel()
                        .size(px(200.0))
                        .size_range(px(80.0)..px(600.0))
                        .child(
                            div()
                                .size_full()
                                .border_t_1()
                                .border_color(Colors::border_subtle())
                                .child(self.terminal_pane.clone()),
                        ),
                )
                .into_any_element()
        } else {
            editor_area.into_any_element()
        };

        // Build main horizontal layout
        // Sidebar is separate from the editor+git resizable group
        // so dragging the git panel never affects the sidebar.
        let sidebar_open = self.sidebar_open;
        let git_panel_open = self.git_panel_open;

        // Editor + git panel (resizable together)
        let right_area = if git_panel_open {
            let root = self.project_root.clone().unwrap_or_default();
            let diff_text = self.diff_text.clone();
            let git_data = &self.git_data;
            let commit_msg = self.commit_input.read(cx).value().to_string();

            h_resizable("editor-git-split")
                .child(
                    resizable_panel()
                        .size_range(px(300.0)..Pixels::MAX)
                        .child(editor_col),
                )
                .child(
                    resizable_panel()
                        .size(px(280.0))
                        .size_range(px(180.0)..px(500.0))
                        .child(git_panel::render_git_panel(
                            git_data,
                            &commit_msg,
                            &root,
                            &diff_text,
                        )),
                )
                .into_any_element()
        } else {
            div().size_full().child(editor_col).into_any_element()
        };

        // Sidebar + right area
        let main_content = if sidebar_open {
            h_resizable("sidebar-split")
                .child(
                    resizable_panel()
                        .size(px(240.0))
                        .size_range(px(140.0)..px(400.0))
                        .child(self.render_sidebar(cx)),
                )
                .child(
                    resizable_panel().child(right_area),
                )
                .into_any_element()
        } else {
            div().size_full().child(right_area).into_any_element()
        };

        let mut root = div()
            .id("workspace")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(Colors::bg_base())
            .text_color(Colors::text())
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_action(cx.listener(Self::toggle_terminal))
            .on_action(cx.listener(Self::toggle_git_panel))
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::close_tab))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::prev_tab))
            .on_action(cx.listener(Self::toggle_search))
            .on_key_down(cx.listener(Self::handle_search_keys))
            .child(self.render_titlebar())
            .child(
                div()
                    .flex_1()
                    .flex()
                    .min_h_0()
                    .child(main_content),
            )
            .child(self.render_status_bar(cx));

        if self.search_visible {
            root = root.child(self.render_search_overlay(cx));
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

fn language_from_path(path: &str) -> Option<SharedString> {
    let ext = path.rsplit('.').next()?;
    let lang = match ext {
        "rs" => "rust",
        "js" | "mjs" | "cjs" => "javascript",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "typescript",
        "jsx" => "javascript",
        "py" => "python",
        "rb" => "ruby",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "java" => "java",
        "swift" => "swift",
        "zig" => "zig",
        "html" | "htm" => "html",
        "css" => "css",
        "json" => "json",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "md" | "markdown" => "markdown",
        "sh" | "bash" | "zsh" => "bash",
        "sql" => "sql",
        "xml" | "svg" => "html",
        "scala" => "scala",
        "ex" | "exs" => "elixir",
        "proto" => "proto",
        "graphql" | "gql" => "graphql",
        "cmake" => "cmake",
        "diff" | "patch" => "diff",
        "Makefile" | "makefile" => "make",
        _ => return None,
    };
    Some(SharedString::from(lang))
}

// ── Public entry point ──────────────────────────────────────────────

pub fn build_workspace(window: &mut Window, app: &mut App) -> Entity<Root> {
    gpui_component::init(app);

    app.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-j", ToggleTerminal, None),
        KeyBinding::new("cmd-o", OpenFolder, None),
        KeyBinding::new("cmd-g", ToggleGitPanel, None),
        KeyBinding::new("cmd-p", ToggleSearchModal, None),
        KeyBinding::new("cmd-s", Save, None),
        KeyBinding::new("cmd-w", CloseTab, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PrevTab, None),
    ]);

    let tree_state = app.new(|cx| TreeState::new(cx));
    let search_input = app.new(|cx| {
        InputState::new(window, cx).placeholder("Search files...")
    });
    let commit_input = app.new(|cx| {
        InputState::new(window, cx).placeholder("Commit message...")
    });
    let terminal_pane = app.new(|cx| TerminalPane::new(cx));

    let ts = tree_state.clone();
    let si = search_input.clone();
    let ci = commit_input.clone();
    let tp = terminal_pane.clone();
    let view = app.new(|cx| {
        cx.observe(&ts, WorkspaceView::on_tree_changed).detach();
        cx.subscribe(&si, WorkspaceView::on_search_changed).detach();
        cx.subscribe(&tp, |this: &mut WorkspaceView, _, event: &TerminalEvent, cx| {
            match event {
                TerminalEvent::AllSessionsClosed => {
                    this.terminal_open = false;
                    cx.notify();
                }
            }
        }).detach();
        WorkspaceView::new(ts, si, ci, tp, cx)
    });

    let view: AnyView = view.into();
    app.new(|cx| Root::new(view, window, cx))
}
