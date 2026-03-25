use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gpui::*;
use gpui::prelude::FluentBuilder as _;
use gpui_component::Root;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::Sizable;
use gpui_component::menu::ContextMenuExt;
use gpui_component::resizable::{h_resizable, v_resizable, resizable_panel};
use gpui_component::list::ListItem;
use gpui_component::tree::{tree, TreeItem, TreeState};

use embd_platform::fs::{FileEntry, ProjectFs};
use embd_platform::git::GitRepo;
use embd_platform::search::find_files;

use crate::assets;
use crate::git_panel::{self, GitPanelData};
use crate::pdf_render;
use crate::terminal::{TerminalEvent, TerminalPane};
use crate::theme::Colors;

// ── File viewer types ────────────────────────────────────────────────

enum FileViewer {
    /// Code / text editor (InputState)
    Editor(Entity<InputState>),
    /// Image viewer (path on disk)
    Image { path: PathBuf, zoom: f32 },
    /// SVG viewer (path on disk — rendered as image)
    Svg(PathBuf),
    /// PDF — rendered pages as GPUI images
    Pdf {
        pages: Vec<Arc<RenderImage>>,
        page_count: usize,
        zoom: f32,
    },
    /// Binary / non-UTF-8 file — show hex preview
    Binary { path: PathBuf, preview: String, size: u64 },
}

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
        // Context menu actions
        CtxNewFile,
        CtxNewFolder,
        CtxOpenInTerminal,
        CtxCut,
        CtxCopy,
        CtxDuplicate,
        CtxPaste,
        CtxRename,
        CtxTrash,
        CtxDelete,
        CtxRevealInFinder,
        CtxOpenDefaultApp,
        CtxCopyPath,
        CtxCopyRelativePath,
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

    // One viewer per open file, keyed by path
    file_viewers: HashMap<String, FileViewer>,
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

    // Context menu state
    context_path: Arc<Mutex<Option<String>>>,
    clipboard_path: Option<(String, bool)>, // (path, is_cut)

    // Tree items (kept in sync with TreeState for expansion state tracking)
    tree_items: Vec<TreeItem>,

    // Inline creation state: (parent_dir, is_directory)
    creating: Option<(String, bool)>,
    create_input: Entity<InputState>,

    // Pinned tabs (always shown at the left, cannot be closed via X)
    pinned_tabs: HashSet<String>,
}

impl WorkspaceView {
    fn new(
        tree_state: Entity<TreeState>,
        search_input: Entity<InputState>,
        commit_input: Entity<InputState>,
        terminal_pane: Entity<TerminalPane>,
        create_input: Entity<InputState>,
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
            file_viewers: HashMap::new(),
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
            context_path: Arc::new(Mutex::new(None)),
            clipboard_path: None,
            tree_items: Vec::new(),
            creating: None,
            create_input,
            pinned_tabs: HashSet::new(),
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
            if let Some(FileViewer::Editor(state)) = self.file_viewers.get(path) {
                let content = state.read(cx).value().to_string();
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
            if !self.pinned_tabs.contains(path) {
                self.tab_order.retain(|p| p != path);
                self.file_viewers.remove(path);
                self.modified_files.remove(path);
                self.active_file = self.tab_order.last().cloned();
            }
        }
        self.last_opened_path = None;
        cx.notify();
    }

    fn close_tab_by_path(&mut self, path: &str, cx: &mut Context<Self>) {
        self.tab_order.retain(|p| p != path);
        self.file_viewers.remove(path);
        self.modified_files.remove(path);
        self.pinned_tabs.remove(path);
        if self.active_file.as_deref() == Some(path) {
            self.active_file = self.tab_order.last().cloned();
        }
        self.last_opened_path = None;
        cx.notify();
    }

    fn toggle_pin_tab(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.pinned_tabs.contains(path) {
            self.pinned_tabs.remove(path);
        } else {
            self.pinned_tabs.insert(path.to_string());
        }
        // Re-sort: pinned tabs first, preserving relative order within each group
        let pinned = &self.pinned_tabs;
        self.tab_order.sort_by_key(|p| if pinned.contains(p) { 0 } else { 1 });
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
        // Escape during inline creation cancels it
        if self.creating.is_some() && event.keystroke.key.as_str() == "escape" {
            self.cancel_creating(cx);
            self.focus_handle.focus(window);
            return;
        }

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
                } else {
                    // No results or empty query — close search
                    self.search_visible = false;
                    self.focus_handle.focus(window);
                    cx.notify();
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

    // ── Context menu handlers ───────────────────────────────────────

    fn refresh_tree(&mut self, cx: &mut Context<Self>) {
        if let Some(ref fs) = self.project_fs {
            let mut expanded = HashSet::new();
            collect_expanded_ids(&self.tree_items, &mut expanded);
            let creating_parent = self.creating.as_ref().map(|(dir, _)| dir.as_str());

            if let Ok(entries) = fs.read_dir_tree(fs.root(), 3) {
                let root_path = fs.root().to_string_lossy().to_string();
                let root_name = Path::new(&root_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let is_root_target = creating_parent.is_some_and(|p| p == root_path);
                let mut child_items = build_tree_items(&entries, &expanded, creating_parent);
                if is_root_target {
                    child_items.insert(0, TreeItem::new("__creating__", ""));
                }
                let root_item = TreeItem::new(root_path.clone(), root_name)
                    .children(child_items)
                    .expanded(true);
                let items = vec![root_item];
                self.tree_items = items.clone();
                self.tree_state.update(cx, |state, cx| {
                    state.set_items(items, cx);
                });
            }
        }
    }

    fn ctx_new_file(&mut self, _: &CtxNewFile, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.context_path.lock().unwrap().clone() else { return };
        let dir = if Path::new(&path).is_dir() {
            path
        } else {
            Path::new(&path).parent().unwrap().to_string_lossy().to_string()
        };
        self.creating = Some((dir, false));
        self.create_input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.focus(window, cx);
        });
        self.refresh_tree(cx);
        cx.notify();
    }

    fn ctx_new_folder(&mut self, _: &CtxNewFolder, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.context_path.lock().unwrap().clone() else { return };
        let dir = if Path::new(&path).is_dir() {
            path
        } else {
            Path::new(&path).parent().unwrap().to_string_lossy().to_string()
        };
        self.creating = Some((dir, true));
        self.create_input.update(cx, |s, cx| {
            s.set_value("", window, cx);
            s.focus(window, cx);
        });
        self.refresh_tree(cx);
        cx.notify();
    }

    fn finish_creating(&mut self, cx: &mut Context<Self>) {
        let Some((parent_dir, is_dir)) = self.creating.take() else { return };
        let name = self.create_input.read(cx).value().to_string();
        if name.is_empty() {
            // Empty name — cancel
            self.refresh_tree(cx);
            cx.notify();
            return;
        }
        let full_path = Path::new(&parent_dir).join(&name);
        if is_dir {
            let _ = std::fs::create_dir(&full_path);
        } else {
            let _ = std::fs::write(&full_path, "");
        }
        self.refresh_tree(cx);
        if !is_dir {
            self.open_file_deferred(&full_path.to_string_lossy());
        }
        cx.notify();
    }

    fn cancel_creating(&mut self, cx: &mut Context<Self>) {
        if self.creating.is_some() {
            self.creating = None;
            self.refresh_tree(cx);
            cx.notify();
        }
    }

    fn on_create_event(&mut self, _: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>) {
        match event {
            InputEvent::PressEnter { .. } => self.finish_creating(cx),
            InputEvent::Blur => self.cancel_creating(cx),
            _ => {}
        }
    }

    fn ctx_reveal_in_finder(&mut self, _: &CtxRevealInFinder, _w: &mut Window, _cx: &mut Context<Self>) {
        let Some(path) = self.context_path.lock().unwrap().clone() else { return };
        let _ = std::process::Command::new("open").args(["-R", &path]).spawn();
    }

    fn ctx_open_default_app(&mut self, _: &CtxOpenDefaultApp, _w: &mut Window, _cx: &mut Context<Self>) {
        let Some(path) = self.context_path.lock().unwrap().clone() else { return };
        let _ = std::process::Command::new("open").arg(&path).spawn();
    }

    fn ctx_open_in_terminal(&mut self, _: &CtxOpenInTerminal, _w: &mut Window, _cx: &mut Context<Self>) {
        let Some(path) = self.context_path.lock().unwrap().clone() else { return };
        let dir = if Path::new(&path).is_dir() {
            path
        } else {
            Path::new(&path).parent().unwrap().to_string_lossy().to_string()
        };
        let _ = std::process::Command::new("open").args(["-a", "Terminal", &dir]).spawn();
    }

    fn ctx_cut(&mut self, _: &CtxCut, _w: &mut Window, _cx: &mut Context<Self>) {
        if let Some(path) = self.context_path.lock().unwrap().clone() {
            self.clipboard_path = Some((path, true));
        }
    }

    fn ctx_copy(&mut self, _: &CtxCopy, _w: &mut Window, _cx: &mut Context<Self>) {
        if let Some(path) = self.context_path.lock().unwrap().clone() {
            self.clipboard_path = Some((path, false));
        }
    }

    fn ctx_duplicate(&mut self, _: &CtxDuplicate, _w: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.context_path.lock().unwrap().clone() else { return };
        let src = Path::new(&path);
        let parent = src.parent().unwrap();
        let stem = src.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let ext = src
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        let mut copy_name = format!("{} copy{}", stem, ext);
        let mut i = 2;
        while parent.join(&copy_name).exists() {
            copy_name = format!("{} copy {}{}", stem, i, ext);
            i += 1;
        }
        if src.is_dir() {
            let _ = std::process::Command::new("cp")
                .args(["-R", &path, &parent.join(&copy_name).to_string_lossy()])
                .output();
        } else {
            let _ = std::fs::copy(&path, parent.join(&copy_name));
        }
        self.refresh_tree(cx);
        cx.notify();
    }

    fn ctx_paste(&mut self, _: &CtxPaste, _w: &mut Window, cx: &mut Context<Self>) {
        let Some((src_path, is_cut)) = self.clipboard_path.clone() else { return };
        let Some(target_path) = self.context_path.lock().unwrap().clone() else { return };
        let target_dir = if Path::new(&target_path).is_dir() {
            target_path
        } else {
            Path::new(&target_path).parent().unwrap().to_string_lossy().to_string()
        };
        let src = Path::new(&src_path);
        let filename = src.file_name().unwrap().to_string_lossy().to_string();
        let dest = Path::new(&target_dir).join(&filename);
        if is_cut {
            let _ = std::fs::rename(&src_path, &dest);
            self.clipboard_path = None;
        } else if src.is_dir() {
            let _ = std::process::Command::new("cp")
                .args(["-R", &src_path, &dest.to_string_lossy()])
                .output();
        } else {
            let _ = std::fs::copy(&src_path, &dest);
        }
        self.refresh_tree(cx);
        cx.notify();
    }

    fn ctx_copy_path(&mut self, _: &CtxCopyPath, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.context_path.lock().unwrap().clone() {
            cx.write_to_clipboard(ClipboardItem::new_string(path));
        }
    }

    fn ctx_copy_relative_path(&mut self, _: &CtxCopyRelativePath, _w: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.context_path.lock().unwrap().clone() {
            let relative = if let Some(ref root) = self.project_root {
                Path::new(&path)
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or(path)
            } else {
                path
            };
            cx.write_to_clipboard(ClipboardItem::new_string(relative));
        }
    }

    fn ctx_rename(&mut self, _: &CtxRename, _w: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.context_path.lock().unwrap().clone() else { return };
        let old_name = Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let path_clone = path.clone();

        let (tx, rx) = flume::bounded(1);
        std::thread::spawn(move || {
            let output = std::process::Command::new("osascript")
                .args([
                    "-e",
                    &format!(
                        "text returned of (display dialog \"Rename:\" default answer \"{}\")",
                        old_name
                    ),
                ])
                .output();
            if let Ok(out) = output {
                let new_name = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !new_name.is_empty() {
                    let _ = tx.send(new_name);
                }
            }
        });

        cx.spawn(async move |this, cx| {
            if let Ok(new_name) = rx.recv_async().await {
                let _ = this.update(cx, |this, cx| {
                    let src = Path::new(&path_clone);
                    let dest = src.parent().unwrap().join(&new_name);
                    if std::fs::rename(src, &dest).is_ok() {
                        let old_path = path_clone.clone();
                        let new_path = dest.to_string_lossy().to_string();
                        if let Some(state) = this.file_viewers.remove(&old_path) {
                            this.file_viewers.insert(new_path.clone(), state);
                            if let Some(pos) = this.tab_order.iter().position(|p| p == &old_path) {
                                this.tab_order[pos] = new_path.clone();
                            }
                            if this.active_file.as_deref() == Some(&old_path) {
                                this.active_file = Some(new_path);
                            }
                        }
                        this.refresh_tree(cx);
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn ctx_trash(&mut self, _: &CtxTrash, _w: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.context_path.lock().unwrap().clone() else { return };
        self.tab_order.retain(|p| p != &path);
        self.file_viewers.remove(&path);
        self.modified_files.remove(&path);
        if self.active_file.as_deref() == Some(&path) {
            self.active_file = self.tab_order.last().cloned();
        }
        let _ = std::process::Command::new("osascript")
            .args([
                "-e",
                &format!(
                    "tell app \"Finder\" to delete POSIX file \"{}\"",
                    path
                ),
            ])
            .output();
        self.refresh_tree(cx);
        cx.notify();
    }

    fn ctx_delete(&mut self, _: &CtxDelete, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.context_path.lock().unwrap().clone() else { return };
        let filename = Path::new(&path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let rx = window.prompt(
            PromptLevel::Warning,
            &format!("Delete {}?", filename),
            None,
            &[PromptButton::cancel("Cancel"), PromptButton::ok("Delete")],
            cx,
        );

        cx.spawn(async move |this, cx| {
            if let Ok(1) = rx.await {
                let _ = this.update(cx, |this, cx| {
                    this.tab_order.retain(|p| p != &path);
                    this.file_viewers.remove(&path);
                    this.modified_files.remove(&path);
                    if this.active_file.as_deref() == Some(&path) {
                        this.active_file = this.tab_order.last().cloned();
                    }
                    let p = Path::new(&path);
                    if p.is_dir() {
                        let _ = std::fs::remove_dir_all(p);
                    } else {
                        let _ = std::fs::remove_file(p);
                    }
                    this.refresh_tree(cx);
                    cx.notify();
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

        let root_path = path.to_string_lossy().to_string();
        let root_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let child_items = file_entries_to_tree_items(&entries);
        let root_item = TreeItem::new(root_path, root_name)
            .children(child_items)
            .expanded(true);
        let tree_items = vec![root_item];
        self.tree_items = tree_items.clone();
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
        self.file_viewers.clear();
        self.tab_order.clear();
        self.active_file = None;
        self.modified_files.clear();
        self.pinned_tabs.clear();
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
        if self.file_viewers.contains_key(path) {
            self.active_file = Some(path.to_string());
            return;
        }

        const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Failed to open file: {e}");
                return;
            }
        };
        if meta.len() > MAX_FILE_SIZE {
            eprintln!("File too large to open ({} bytes): {}", meta.len(), path);
            return;
        }

        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        // Image files — open directly with GPUI img()
        let image_exts = [
            "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tiff", "tif",
            "avif", "tga", "qoi",
        ];
        if image_exts.contains(&ext.as_str()) {
            let viewer = FileViewer::Image { path: PathBuf::from(path), zoom: 1.0 };
            if !self.tab_order.contains(&path.to_string()) {
                self.tab_order.push(path.to_string());
            }
            self.file_viewers.insert(path.to_string(), viewer);
            self.active_file = Some(path.to_string());
            return;
        }

        // SVG — render as image
        if ext == "svg" {
            let viewer = FileViewer::Svg(PathBuf::from(path));
            if !self.tab_order.contains(&path.to_string()) {
                self.tab_order.push(path.to_string());
            }
            self.file_viewers.insert(path.to_string(), viewer);
            self.active_file = Some(path.to_string());
            return;
        }

        // PDF — render pages inline
        if ext == "pdf" {
            let pdf_path = PathBuf::from(path);
            let (pages, page_count) = pdf_render::render_pdf(&pdf_path, 2.0)
                .unwrap_or_else(|| (vec![], 0));
            let viewer = FileViewer::Pdf {
                pages,
                page_count,
                zoom: 1.0,
            };
            if !self.tab_order.contains(&path.to_string()) {
                self.tab_order.push(path.to_string());
            }
            self.file_viewers.insert(path.to_string(), viewer);
            self.active_file = Some(path.to_string());
            return;
        }

        // Try reading as UTF-8 text
        match std::fs::read(path) {
            Ok(bytes) => {
                if let Ok(content) = String::from_utf8(bytes.clone()) {
                    // Text file — open in editor
                    self.pending_open = Some((path.to_string(), content));
                } else {
                    // Binary file — show hex preview
                    let preview = format_hex_preview(&bytes, 2048);
                    let viewer = FileViewer::Binary {
                        path: PathBuf::from(path),
                        preview,
                        size: meta.len(),
                    };
                    if !self.tab_order.contains(&path.to_string()) {
                        self.tab_order.push(path.to_string());
                    }
                    self.file_viewers.insert(path.to_string(), viewer);
                    self.active_file = Some(path.to_string());
                }
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

            let path_clone = path.clone();
            cx.subscribe(&state, move |this: &mut Self, _, _event: &InputEvent, cx| {
                this.modified_files.insert(path_clone.clone(), true);
                cx.notify();
            })
            .detach();

            if !self.tab_order.contains(&path) {
                self.tab_order.push(path.clone());
            }
            self.file_viewers.insert(path.clone(), FileViewer::Editor(state));
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
        let context_path = self.context_path.clone();
        let focus = self.focus_handle.clone();
        let creating = self.creating.clone();
        let create_input_for_tree = self.create_input.clone();

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
                let focus_for_menu = focus.clone();
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

                            // Inline creation placeholder
                            if path_id == "__creating__" {
                                let is_creating_dir = creating.as_ref().map(|(_, d)| *d).unwrap_or(false);
                                let icon = if is_creating_dir { "icons/folder.svg" } else { "icons/file.svg" };
                                return ListItem::new(ix)
                                    .pl(px(12.0 * depth as f32 + 10.0))
                                    .child(
                                        div()
                                            .flex()
                                            .w_full()
                                            .gap(px(5.0))
                                            .items_center()
                                            .text_xs()
                                            .text_color(Colors::text_muted())
                                            .child(
                                                svg()
                                                    .path(icon)
                                                    .size(px(14.0))
                                                    .flex_shrink_0()
                                                    .text_color(assets::icon_color(icon)),
                                            )
                                            .child(
                                                Input::new(&create_input_for_tree)
                                                    .appearance(false)
                                                    .bordered(false)
                                                    .xsmall(),
                                            ),
                                    );
                            }

                            let git_code = git_map.get(&path_id).cloned();
                            let status_char = git_code
                                .as_deref()
                                .map(git_panel::status_label)
                                .unwrap_or("");
                            let status_color = git_code
                                .as_deref()
                                .map(git_panel::status_color)
                                .unwrap_or(Colors::text());

                            let icon_path = assets::icon_for_path(
                                &path_id,
                                is_dir,
                                entry.is_expanded(),
                            );

                            let ctx_path = context_path.clone();
                            let ctx_str = path_id.clone();

                            ListItem::new(ix)
                                .selected(selected)
                                .pl(px(12.0 * depth as f32 + 10.0))
                                .child(
                                    div()
                                        .flex()
                                        .w_full()
                                        .gap(px(5.0))
                                        .items_center()
                                        .text_xs()
                                        .text_color(if is_dir {
                                            Colors::text()
                                        } else if git_code.is_some() {
                                            status_color
                                        } else {
                                            Colors::text_muted()
                                        })
                                        .child(
                                            svg()
                                                .path(icon_path)
                                                .size(px(14.0))
                                                .flex_shrink_0()
                                                .text_color(assets::icon_color(icon_path)),
                                        )
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
                                        })
                                        .on_mouse_down(MouseButton::Right, move |_, _, _| {
                                            *ctx_path.lock().unwrap() = Some(ctx_str.clone());
                                        }),
                                )
                        }),
                    )
                    .context_menu(move |menu, _window, _cx| {
                        menu.action_context(focus_for_menu.clone())
                            .menu("New File", Box::new(CtxNewFile))
                            .menu("New Folder", Box::new(CtxNewFolder))
                            .separator()
                            .menu("Reveal in Finder", Box::new(CtxRevealInFinder))
                            .menu("Open in Default App", Box::new(CtxOpenDefaultApp))
                            .menu("Open in Terminal", Box::new(CtxOpenInTerminal))
                            .separator()
                            .menu("Cut", Box::new(CtxCut))
                            .menu("Copy", Box::new(CtxCopy))
                            .menu("Duplicate", Box::new(CtxDuplicate))
                            .menu("Paste", Box::new(CtxPaste))
                            .separator()
                            .menu("Copy Path", Box::new(CtxCopyPath))
                            .menu("Copy Relative Path", Box::new(CtxCopyRelativePath))
                            .separator()
                            .menu("Rename", Box::new(CtxRename))
                            .menu("Trash", Box::new(CtxTrash))
                            .menu("Delete", Box::new(CtxDelete))
                    })
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

    fn render_search_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let max_visible = 12;

        div()
            .id("search-overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.0))
            .flex()
            .flex_col()
            .items_center()
            .pt(px(60.0))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                this.search_visible = false;
                this.focus_handle.focus(window);
                cx.notify();
            }))
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
                    // Stop clicks inside the modal from reaching the backdrop
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    })
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
            match self.file_viewers.get(path) {
                Some(FileViewer::Editor(state)) => {
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
                }
                Some(FileViewer::Image { .. }) => "Image".to_string(),
                Some(FileViewer::Svg(_)) => "SVG".to_string(),
                Some(FileViewer::Pdf { page_count, .. }) => {
                    format!("PDF — {} page{}", page_count, if *page_count == 1 { "" } else { "s" })
                }
                Some(FileViewer::Binary { size, .. }) => {
                    format!("Binary — {}", format_file_size(*size))
                }
                None => format!("{}  UTF-8", lang),
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
            // Right side: file info + settings gear
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_color(Colors::text_faint())
                            .child(file_info),
                    )
                    .child(
                        div()
                            .id("settings-btn")
                            .size(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .hover(|s| s.bg(Colors::surface_hover()))
                            .child(
                                svg()
                                    .path("icons/settings.svg")
                                    .size(px(14.0))
                                    .text_color(Colors::text_faint()),
                            )
                            .on_click(|_, _, cx| {
                                embd_settings::open_settings_window(cx);
                            }),
                    ),
            )
    }
}

// ── Render impl ─────────────────────────────────────────────────────

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Process any pending file opens (needs window for InputState creation)
        self.process_pending_open(window, cx);

        // Focus management — don't steal focus when search overlay or inline creation is active
        if !self.search_visible && self.creating.is_none() && !self.focus_handle.contains_focused(window, cx) {
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
                    let is_pinned = self.pinned_tabs.contains(path);
                    let name = PathBuf::from(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("untitled")
                        .to_string();
                    let modified = self.modified_files.get(path).copied().unwrap_or(false);
                    let tab_icon = assets::icon_for_path(path, false, false);
                    let path_for_close = path.clone();
                    let path_for_pin = path.clone();

                    div()
                        .id(SharedString::from(format!("tab-{}", i)))
                        .pl(px(12.0))
                        .pr(px(4.0))
                        .py(px(6.0))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .group("tab")
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
                        .when(is_pinned, |d| {
                            d.border_b_1()
                                .border_color(Colors::accent_dim())
                        })
                        // File type icon
                        .child(
                            svg()
                                .path(tab_icon)
                                .size(px(13.0))
                                .flex_shrink_0()
                                .text_color(assets::icon_color(tab_icon)),
                        )
                        // File name
                        .child(name)
                        // Modified indicator
                        .when(modified, |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(Colors::text_faint())
                                    .child("\u{25CF}"),
                            )
                        })
                        // Pin button — visible on hover or when pinned
                        .child(
                            div()
                                .id(SharedString::from(format!("tab-pin-{}", i)))
                                .flex_shrink_0()
                                .size(px(16.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(3.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(Colors::bg_overlay()))
                                .when(!is_pinned, |d| {
                                    d.invisible()
                                        .group_hover("tab", |s| s.visible())
                                })
                                .child(
                                    svg()
                                        .path(if is_pinned {
                                            "icons/pin-filled.svg"
                                        } else {
                                            "icons/pin.svg"
                                        })
                                        .size(px(12.0))
                                        .text_color(if is_pinned {
                                            Colors::accent_dim()
                                        } else {
                                            Colors::text_faint()
                                        }),
                                )
                                .on_click(cx.listener(move |this, _, _w, cx| {
                                    this.toggle_pin_tab(&path_for_pin, cx);
                                })),
                        )
                        // Close button — visible on hover, hidden for pinned tabs
                        .child(
                            div()
                                .id(SharedString::from(format!("tab-close-{}", i)))
                                .flex_shrink_0()
                                .size(px(16.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(3.0))
                                .cursor_pointer()
                                .hover(|s| s.bg(Colors::bg_overlay()))
                                .when(is_pinned, |d| d.hidden())
                                .when(!is_pinned, |d| {
                                    d.invisible()
                                        .group_hover("tab", |s| s.visible())
                                })
                                .child(
                                    svg()
                                        .path("icons/close.svg")
                                        .size(px(12.0))
                                        .text_color(Colors::text_faint()),
                                )
                                .on_click(cx.listener(move |this, _, _w, cx| {
                                    this.close_tab_by_path(&path_for_close, cx);
                                })),
                        )
                        // Click on the tab body to activate
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
            match self.file_viewers.get(path) {
                Some(FileViewer::Editor(state)) => {
                    div()
                        .size_full()
                        .child(
                            Input::new(state)
                                .appearance(false)
                                .bordered(false)
                                .h_full(),
                        )
                        .into_any_element()
                }
                Some(FileViewer::Image { path: img_path, zoom }) => {
                    let src = img_path.clone();
                    let z = *zoom;
                    let zoom_pct = format!("{}%", (z * 100.0) as u32);
                    div()
                        .id("image-viewer")
                        .size_full()
                        .flex()
                        .flex_col()
                        .bg(Colors::bg_base())
                        .child(render_zoom_bar(&zoom_pct, cx))
                        .child(
                            div()
                                .id("image-scroll")
                                .flex_1()
                                .overflow_scroll()
                                .flex()
                                .items_center()
                                .justify_center()
                                .p(px(20.0))
                                .on_scroll_wheel(cx.listener(
                                    |this, event: &ScrollWheelEvent, _w, cx| {
                                        if event.modifiers.platform {
                                            apply_viewer_zoom(this, event.delta.pixel_delta(px(1.0)).y);
                                            cx.notify();
                                        }
                                    },
                                ))
                                .child(
                                    img(ImageSource::from(src))
                                        .w(px(800.0 * z))
                                        .object_fit(ObjectFit::Contain),
                                ),
                        )
                        .into_any_element()
                }
                Some(FileViewer::Svg(svg_path)) => {
                    let src = svg_path.clone();
                    div()
                        .id("svg-viewer")
                        .size_full()
                        .overflow_scroll()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(Colors::bg_base())
                        .p(px(20.0))
                        .child(
                            img(ImageSource::from(src))
                                .max_w_full()
                                .max_h_full()
                                .object_fit(ObjectFit::Contain),
                        )
                        .into_any_element()
                }
                Some(FileViewer::Pdf { pages, page_count, zoom }) => {
                    let z = *zoom;
                    let pc = *page_count;
                    if pages.is_empty() {
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_xs()
                            .text_color(Colors::text_faint())
                            .child("Failed to render PDF")
                            .into_any_element()
                    } else {
                        let zoom_pct = format!("{}%", (z * 100.0) as u32);
                        let page_images: Vec<AnyElement> = pages
                            .iter()
                            .enumerate()
                            .map(|(i, render_img)| {
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .mb(px(16.0))
                                    .child(
                                        img(ImageSource::Render(render_img.clone()))
                                            .w(px(700.0 * z))
                                            .object_fit(ObjectFit::ScaleDown),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .text_color(hsla(0.0, 0.0, 0.7, 1.0))
                                            .mt(px(4.0))
                                            .child(format!(
                                                "Page {} of {}",
                                                i + 1,
                                                pc
                                            )),
                                    )
                                    .into_any_element()
                            })
                            .collect();

                        div()
                            .size_full()
                            .flex()
                            .flex_col()
                            .bg(rgb(0x3a3a3a))
                            .child(render_zoom_bar(&zoom_pct, cx))
                            .child(
                                div()
                                    .id("pdf-viewer")
                                    .flex_1()
                                    .overflow_scroll()
                                    .p(px(24.0))
                                    .on_scroll_wheel(cx.listener(
                                        |this, event: &ScrollWheelEvent, _w, cx| {
                                            if event.modifiers.platform {
                                                apply_viewer_zoom(this, event.delta.pixel_delta(px(1.0)).y);
                                                cx.notify();
                                            }
                                        },
                                    ))
                                    .child(
                                        div()
                                            .w_full()
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .children(page_images),
                                    ),
                            )
                            .into_any_element()
                    }
                }
                Some(FileViewer::Binary { path: bin_path, preview, size }) => {
                    let filename = bin_path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let size_str = format_file_size(*size);
                    let preview_text = preview.clone();
                    let open_path = bin_path.clone();
                    div()
                        .size_full()
                        .flex()
                        .flex_col()
                        .bg(Colors::bg_base())
                        .child(
                            div()
                                .px(px(16.0))
                                .py(px(10.0))
                                .flex()
                                .items_center()
                                .gap(px(10.0))
                                .border_b_1()
                                .border_color(Colors::border_subtle())
                                .child(
                                    svg()
                                        .path("icons/file.svg")
                                        .size(px(16.0))
                                        .text_color(Colors::text_faint()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Colors::text_muted())
                                        .child(format!("{} — Binary file ({})", filename, size_str)),
                                )
                                .child(
                                    div()
                                        .id("open-binary-external")
                                        .ml_auto()
                                        .px(px(10.0))
                                        .py(px(4.0))
                                        .rounded(px(3.0))
                                        .bg(Colors::bg_surface())
                                        .border_1()
                                        .border_color(Colors::border_subtle())
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(Colors::text_muted())
                                        .hover(|s| s.bg(Colors::surface_hover()))
                                        .child("Open Externally")
                                        .on_click({
                                            let p = open_path.clone();
                                            move |_, _, _| {
                                                let _ = std::process::Command::new("open")
                                                    .arg(&p)
                                                    .spawn();
                                            }
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .id("hex-preview")
                                .flex_1()
                                .overflow_scroll()
                                .px(px(16.0))
                                .py(px(10.0))
                                .font_family("monospace")
                                .text_size(px(12.0))
                                .line_height(px(18.0))
                                .text_color(Colors::text_faint())
                                .child(preview_text),
                        )
                        .into_any_element()
                }
                None => {
                    div()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(Colors::text_muted())
                        .child("Loading...")
                        .into_any_element()
                }
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
            .on_action(cx.listener(Self::ctx_new_file))
            .on_action(cx.listener(Self::ctx_new_folder))
            .on_action(cx.listener(Self::ctx_reveal_in_finder))
            .on_action(cx.listener(Self::ctx_open_default_app))
            .on_action(cx.listener(Self::ctx_open_in_terminal))
            .on_action(cx.listener(Self::ctx_cut))
            .on_action(cx.listener(Self::ctx_copy))
            .on_action(cx.listener(Self::ctx_duplicate))
            .on_action(cx.listener(Self::ctx_paste))
            .on_action(cx.listener(Self::ctx_copy_path))
            .on_action(cx.listener(Self::ctx_copy_relative_path))
            .on_action(cx.listener(Self::ctx_rename))
            .on_action(cx.listener(Self::ctx_trash))
            .on_action(cx.listener(Self::ctx_delete))
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

/// Render a small zoom toolbar with −/+/Reset buttons and current percentage.
fn render_zoom_bar(zoom_pct: &str, cx: &mut Context<WorkspaceView>) -> impl IntoElement {
    div()
        .id("zoom-bar")
        .w_full()
        .h(px(28.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .bg(Colors::bg_surface())
        .border_b_1()
        .border_color(Colors::border_subtle())
        .child(
            div()
                .id("zoom-out-btn")
                .px(px(8.0))
                .py(px(2.0))
                .rounded(px(3.0))
                .cursor_pointer()
                .text_xs()
                .text_color(Colors::text_muted())
                .hover(|s| s.bg(Colors::surface_hover()))
                .child("\u{2212}") // minus sign
                .on_click(cx.listener(|this, _, _w, cx| {
                    // Zoom out by a fixed step
                    apply_viewer_zoom_step(this, -0.1);
                    cx.notify();
                })),
        )
        .child(
            div()
                .min_w(px(40.0))
                .flex()
                .justify_center()
                .text_xs()
                .text_color(Colors::text_muted())
                .child(zoom_pct.to_string()),
        )
        .child(
            div()
                .id("zoom-in-btn")
                .px(px(8.0))
                .py(px(2.0))
                .rounded(px(3.0))
                .cursor_pointer()
                .text_xs()
                .text_color(Colors::text_muted())
                .hover(|s| s.bg(Colors::surface_hover()))
                .child("+")
                .on_click(cx.listener(|this, _, _w, cx| {
                    // Zoom in by a fixed step
                    apply_viewer_zoom_step(this, 0.1);
                    cx.notify();
                })),
        )
        .child(
            div()
                .id("zoom-reset-btn")
                .px(px(8.0))
                .py(px(2.0))
                .rounded(px(3.0))
                .cursor_pointer()
                .text_xs()
                .text_color(Colors::text_faint())
                .hover(|s| s.bg(Colors::surface_hover()))
                .child("Reset")
                .on_click(cx.listener(|this, _, _w, cx| {
                    if let Some(ref path) = this.active_file {
                        match this.file_viewers.get_mut(path) {
                            Some(FileViewer::Image { zoom, .. }) => *zoom = 1.0,
                            Some(FileViewer::Pdf { zoom, .. }) => *zoom = 1.0,
                            _ => {}
                        }
                    }
                    cx.notify();
                })),
        )
}

/// Apply zoom from Cmd+scroll wheel delta (negative y = scroll up = zoom in).
fn apply_viewer_zoom(this: &mut WorkspaceView, delta_y: Pixels) {
    let Some(ref path) = this.active_file else { return };
    let zoom = match this.file_viewers.get_mut(path) {
        Some(FileViewer::Image { zoom, .. }) => zoom,
        Some(FileViewer::Pdf { zoom, .. }) => zoom,
        _ => return,
    };
    let change = -f32::from(delta_y) * 0.005;
    *zoom = (*zoom + change).clamp(0.1, 5.0);
}

/// Apply a fixed zoom step (positive = zoom in, negative = zoom out).
fn apply_viewer_zoom_step(this: &mut WorkspaceView, step: f32) {
    let Some(ref path) = this.active_file else { return };
    let zoom = match this.file_viewers.get_mut(path) {
        Some(FileViewer::Image { zoom, .. }) => zoom,
        Some(FileViewer::Pdf { zoom, .. }) => zoom,
        _ => return,
    };
    *zoom = (*zoom + step).clamp(0.1, 5.0);
}

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

/// Build tree items preserving expansion state and optionally inserting a creation placeholder.
fn build_tree_items(
    entries: &[FileEntry],
    expanded: &HashSet<String>,
    creating_parent: Option<&str>,
) -> Vec<TreeItem> {
    entries
        .iter()
        .map(|entry| {
            let is_target = creating_parent.is_some_and(|p| entry.path == p);
            let mut item = TreeItem::new(entry.path.clone(), entry.name.clone());
            if let Some(ref children) = entry.children {
                let mut child_items = build_tree_items(children, expanded, creating_parent);
                if is_target {
                    child_items.insert(0, TreeItem::new("__creating__", ""));
                }
                item = item.children(child_items);
                if expanded.contains(&entry.path) || is_target {
                    item = item.expanded(true);
                }
            }
            item
        })
        .collect()
}

/// Recursively collect IDs of expanded folders from stored tree items.
fn collect_expanded_ids(items: &[TreeItem], set: &mut HashSet<String>) {
    for item in items {
        if item.is_expanded() {
            set.insert(item.id.to_string());
        }
        collect_expanded_ids(&item.children, set);
    }
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_hex_preview(bytes: &[u8], max_bytes: usize) -> String {
    let mut out = String::new();
    let limit = bytes.len().min(max_bytes);
    for (i, chunk) in bytes[..limit].chunks(16).enumerate() {
        // Offset
        out.push_str(&format!("{:08x}  ", i * 16));
        // Hex bytes
        for (j, b) in chunk.iter().enumerate() {
            out.push_str(&format!("{:02x} ", b));
            if j == 7 {
                out.push(' ');
            }
        }
        // Padding for short lines
        let pad = 16 - chunk.len();
        for j in 0..pad {
            out.push_str("   ");
            if chunk.len() + j == 7 {
                out.push(' ');
            }
        }
        out.push(' ');
        // ASCII
        out.push('|');
        for b in chunk {
            if b.is_ascii_graphic() || *b == b' ' {
                out.push(*b as char);
            } else {
                out.push('.');
            }
        }
        out.push('|');
        out.push('\n');
    }
    if limit < bytes.len() {
        out.push_str(&format!("... ({} more bytes)\n", bytes.len() - limit));
    }
    out
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
    let create_input = app.new(|cx| {
        InputState::new(window, cx).placeholder("Enter name...")
    });

    let ts = tree_state.clone();
    let si = search_input.clone();
    let ci = commit_input.clone();
    let tp = terminal_pane.clone();
    let cri = create_input.clone();
    let view = app.new(|cx| {
        cx.observe(&ts, WorkspaceView::on_tree_changed).detach();
        cx.subscribe(&si, WorkspaceView::on_search_changed).detach();
        cx.subscribe(&cri, WorkspaceView::on_create_event).detach();
        cx.subscribe(&tp, |this: &mut WorkspaceView, _, event: &TerminalEvent, cx| {
            match event {
                TerminalEvent::AllSessionsClosed => {
                    this.terminal_open = false;
                    cx.notify();
                }
            }
        }).detach();
        WorkspaceView::new(ts, si, ci, tp, cri, cx)
    });

    let view: AnyView = view.into();
    app.new(|cx| Root::new(view, window, cx))
}
