use std::path::PathBuf;

use gpui::*;
use gpui::prelude::FluentBuilder as _;

use embd_platform::search::{find_files, FileMatch};

use crate::theme::Colors;

// ── Events emitted by SearchModal ───────────────────────────────────

pub enum SearchEvent {
    Open(String),  // absolute path to open
    Dismiss,
}

// ── SearchModal Entity ──────────────────────────────────────────────

pub struct SearchModal {
    focus_handle: FocusHandle,
    query: String,
    results: Vec<FileMatch>,
    selected_index: usize,
    project_root: Option<PathBuf>,
}

impl EventEmitter<SearchEvent> for SearchModal {}

impl SearchModal {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            query: String::new(),
            results: Vec::new(),
            selected_index: 0,
            project_root: None,
        }
    }

    pub fn set_root(&mut self, root: PathBuf) {
        self.project_root = Some(root);
    }

    pub fn show(&mut self, window: &mut Window, _cx: &mut Context<Self>) {
        self.query.clear();
        self.results.clear();
        self.selected_index = 0;
        self.focus_handle.focus(window);
    }

    fn do_search(&mut self) {
        if let Some(ref root) = self.project_root {
            if self.query.is_empty() {
                self.results.clear();
            } else {
                self.results = find_files(root, &self.query, 50).unwrap_or_default();
            }
            self.selected_index = 0;
        }
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();

        if event.keystroke.modifiers.platform || event.keystroke.modifiers.control {
            return; // Let actions handle Cmd+key
        }

        match key {
            "escape" => {
                cx.emit(SearchEvent::Dismiss);
            }
            "enter" => {
                if let Some(result) = self.results.get(self.selected_index) {
                    if let Some(ref root) = self.project_root {
                        let abs = root.join(&result.relative_path);
                        cx.emit(SearchEvent::Open(abs.to_string_lossy().to_string()));
                    }
                }
            }
            "up" => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                cx.notify();
            }
            "down" => {
                if self.selected_index + 1 < self.results.len() {
                    self.selected_index += 1;
                }
                cx.notify();
            }
            "backspace" => {
                self.query.pop();
                self.do_search();
                cx.notify();
            }
            _ => {
                if let Some(ref ch) = event.keystroke.key_char {
                    self.query.push_str(ch);
                } else if key.len() == 1 {
                    self.query.push_str(key);
                } else {
                    return;
                }
                self.do_search();
                cx.notify();
            }
        }
    }
}

impl Render for SearchModal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let max_visible = 12;

        div()
            .id("search-modal-overlay")
            .size_full()
            .absolute()
            .top_0()
            .left_0()
            .bg(hsla(0.0, 0.0, 0.0, 0.4))
            .flex()
            .flex_col()
            .items_center()
            .pt(px(80.0))
            .child(
                div()
                    .id("search-modal")
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(Self::handle_key_down))
                    .w(px(500.0))
                    .max_h(px(400.0))
                    .bg(Colors::bg_surface())
                    .border_1()
                    .border_color(Colors::border())
                    .rounded(px(8.0))
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    // Search input
                    .child(
                        div()
                            .px(px(16.0))
                            .py(px(12.0))
                            .border_b_1()
                            .border_color(Colors::border())
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(Colors::text_muted())
                                    .child(">"),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .text_color(if self.query.is_empty() {
                                        Colors::text_muted()
                                    } else {
                                        Colors::text()
                                    })
                                    .child(if self.query.is_empty() {
                                        "Search files...".to_string()
                                    } else {
                                        self.query.clone()
                                    }),
                            ),
                    )
                    // Results
                    .child(
                        div()
                            .id("search-results")
                            .flex_1()
                            .overflow_y_scroll()
                            .children(
                                self.results
                                    .iter()
                                    .take(max_visible)
                                    .enumerate()
                                    .map(|(ix, result)| {
                                        let is_selected = ix == self.selected_index;
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
                                            .px(px(16.0))
                                            .py(px(6.0))
                                            .flex()
                                            .items_center()
                                            .gap(px(8.0))
                                            .when(is_selected, |d: Div| {
                                                d.bg(Colors::bg_overlay())
                                            })
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(Colors::text())
                                                    .child(filename),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(Colors::text_muted())
                                                    .child(dir_path),
                                            )
                                    }),
                            ),
                    )
                    // Footer hint
                    .child(
                        div()
                            .px(px(16.0))
                            .py(px(8.0))
                            .border_t_1()
                            .border_color(Colors::border())
                            .text_xs()
                            .text_color(Colors::text_muted())
                            .child(format!(
                                "{} files  ↑↓ navigate  ⏎ open  esc dismiss",
                                self.results.len()
                            )),
                    ),
            )
    }
}
