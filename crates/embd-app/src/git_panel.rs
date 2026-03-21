use std::path::Path;

use gpui::*;
use gpui::prelude::FluentBuilder as _;

use embd_platform::git::GitRepo;

use crate::theme::Colors;

/// Data snapshot for git panel rendering.
pub struct GitPanelData {
    pub branch: Option<String>,
    pub staged: Vec<(String, String)>,   // (abs_path, status_code)
    pub changed: Vec<(String, String)>,  // (abs_path, status_code)
    pub ahead: u32,
    pub behind: u32,
}

impl GitPanelData {
    pub fn empty() -> Self {
        Self {
            branch: None,
            staged: Vec::new(),
            changed: Vec::new(),
            ahead: 0,
            behind: 0,
        }
    }
}

/// Events emitted by git panel buttons
#[allow(dead_code)]
pub enum GitAction {
    StageFile(String),
    UnstageFile(String),
    StageAll,
    UnstageAll,
    DiscardFile(String),
    Commit(String),
    Push,
    Pull,
    Fetch,
}

pub fn refresh_git(root: &Path) -> GitPanelData {
    let repo = GitRepo::new(root);
    let branch = repo.branch().ok().flatten();
    let (ahead, behind) = repo
        .ahead_behind()
        .map(|ab| (ab.ahead, ab.behind))
        .unwrap_or((0, 0));

    let status = repo.status().unwrap_or_default();

    let mut staged = Vec::new();
    let mut changed = Vec::new();

    for (abs_path, code) in &status {
        match code.as_str() {
            "S" => staged.push((abs_path.clone(), code.clone())),
            _ => changed.push((abs_path.clone(), code.clone())),
        }
    }

    staged.sort_by(|a, b| a.0.cmp(&b.0));
    changed.sort_by(|a, b| a.0.cmp(&b.0));

    GitPanelData {
        branch,
        staged,
        changed,
        ahead,
        behind,
    }
}

#[allow(dead_code)]
pub fn execute_action(root: &Path, action: &GitAction) -> Result<String, String> {
    let repo = GitRepo::new(root);
    match action {
        GitAction::StageFile(path) => {
            let rel = relative_to(path, root);
            repo.stage(&[rel.as_str()]).map(|_| "Staged".into()).map_err(|e| e.to_string())
        }
        GitAction::UnstageFile(path) => {
            let rel = relative_to(path, root);
            repo.unstage(&[rel.as_str()]).map(|_| "Unstaged".into()).map_err(|e| e.to_string())
        }
        GitAction::StageAll => {
            repo.stage(&["."]).map(|_| "All staged".into()).map_err(|e| e.to_string())
        }
        GitAction::UnstageAll => {
            repo.unstage(&["."]).map(|_| "All unstaged".into()).map_err(|e| e.to_string())
        }
        GitAction::DiscardFile(path) => {
            let rel = relative_to(path, root);
            repo.discard(&[rel.as_str()]).map(|_| "Discarded".into()).map_err(|e| e.to_string())
        }
        GitAction::Commit(msg) => {
            repo.commit(msg).map_err(|e| e.to_string())
        }
        GitAction::Push => {
            repo.push().map_err(|e| e.to_string())
        }
        GitAction::Pull => {
            repo.pull().map_err(|e| e.to_string())
        }
        GitAction::Fetch => {
            repo.fetch().map_err(|e| e.to_string())
        }
    }
}

#[allow(dead_code)]
fn relative_to(path: &str, root: &Path) -> String {
    let root_str = root.to_str().unwrap_or("");
    path.strip_prefix(root_str)
        .unwrap_or(path)
        .trim_start_matches('/')
        .to_string()
}

pub fn status_color(code: &str) -> Hsla {
    match code {
        "M" => Colors::warning(),
        "A" | "S" => Colors::success(),
        "D" => Colors::error(),
        "U" => Colors::text_faint(),
        "C" => Colors::error(),
        _ => Colors::text(),
    }
}

pub fn status_label(code: &str) -> &'static str {
    match code {
        "M" => "M",
        "A" => "A",
        "S" => "S",
        "D" => "D",
        "U" => "?",
        "C" => "!",
        _ => " ",
    }
}

/// Render the full git panel. Returns element + optional action from button clicks.
pub fn render_git_panel(
    data: &GitPanelData,
    commit_msg: &str,
    root: &Path,
    diff_text: &str,
) -> impl IntoElement {
    let branch_text = data.branch.as_deref().unwrap_or("detached").to_string();
    let root_str = root.to_str().unwrap_or("").to_string();

    let mut ahead_behind = String::new();
    if data.ahead > 0 || data.behind > 0 {
        ahead_behind = format!("↑{} ↓{}", data.ahead, data.behind);
    }

    div()
        .id("git-panel")
        .size_full()
        .bg(Colors::bg_surface())
        .border_l_1()
        .border_color(Colors::border())
        .flex()
        .flex_col()
        .overflow_hidden()
        // Header — branch + ahead/behind
        .child(
            div()
                .h(px(32.0))
                .px(px(14.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .border_b_1()
                .border_color(Colors::border_subtle())
                .child(
                    div()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Colors::text_faint())
                        .child("SOURCE CONTROL"),
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
                        .text_color(Colors::accent_dim())
                        .child(branch_text),
                )
                .when(!ahead_behind.is_empty(), |d| {
                    d.child(
                        div()
                            .text_xs()
                            .text_color(Colors::text_faint())
                            .child(ahead_behind.clone()),
                    )
                }),
        )
        // Action buttons
        .child(
            div()
                .px(px(10.0))
                .py(px(6.0))
                .flex()
                .gap(px(4.0))
                .child(action_btn("Fetch"))
                .child(action_btn("Pull"))
                .child(action_btn("Push")),
        )
        // Staged section
        .child(render_file_section("Staged", &data.staged, true, &root_str))
        // Changes section
        .child(render_file_section("Changes", &data.changed, false, &root_str))
        // Diff preview
        .child(if !diff_text.is_empty() {
            div()
                .id("diff-preview")
                .max_h(px(180.0))
                .overflow_y_scroll()
                .mx(px(8.0))
                .mb(px(6.0))
                .bg(Colors::bg_base())
                .rounded(px(3.0))
                .p(px(8.0))
                .text_xs()
                .font_family("monospace")
                .children(diff_text.lines().map(|line| {
                    let color = if line.starts_with('+') && !line.starts_with("+++") {
                        Colors::success()
                    } else if line.starts_with('-') && !line.starts_with("---") {
                        Colors::error()
                    } else if line.starts_with("@@") {
                        Colors::accent_dim()
                    } else {
                        Colors::text_faint()
                    };
                    div().text_color(color).child(line.to_string())
                }))
                .into_any_element()
        } else {
            div().into_any_element()
        })
        // Commit area
        .child(
            div()
                .mt_auto()
                .px(px(10.0))
                .py(px(10.0))
                .flex()
                .flex_col()
                .gap(px(6.0))
                .border_t_1()
                .border_color(Colors::border_subtle())
                .child(
                    div()
                        .h(px(52.0))
                        .w_full()
                        .bg(Colors::bg_base())
                        .border_1()
                        .border_color(Colors::border())
                        .rounded(px(3.0))
                        .p(px(8.0))
                        .text_xs()
                        .text_color(if commit_msg.is_empty() {
                            Colors::text_faint()
                        } else {
                            Colors::text()
                        })
                        .child(if commit_msg.is_empty() {
                            "Commit message...".to_string()
                        } else {
                            commit_msg.to_string()
                        }),
                )
                .child(
                    div()
                        .id("commit-btn")
                        .h(px(26.0))
                        .w_full()
                        .bg(Colors::accent_dim())
                        .rounded(px(3.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(Colors::text())
                        .cursor_pointer()
                        .hover(|s| s.bg(Colors::accent()))
                        .child("Commit"),
                ),
        )
}

fn action_btn(label: &str) -> impl IntoElement {
    div()
        .px(px(8.0))
        .py(px(3.0))
        .bg(Colors::bg_base())
        .rounded(px(3.0))
        .text_xs()
        .text_color(Colors::text_faint())
        .cursor_pointer()
        .hover(|s| s.text_color(Colors::text_muted()).bg(Colors::surface_hover()))
        .child(label.to_string())
}

fn render_file_section(
    title: &str,
    files: &[(String, String)],
    is_staged: bool,
    root_str: &str,
) -> impl IntoElement {
    let title = title.to_string();
    let count = files.len();

    let file_items: Vec<AnyElement> = files
        .iter()
        .map(|(abs_path, code)| {
            let rel = abs_path
                .strip_prefix(root_str)
                .unwrap_or(abs_path)
                .trim_start_matches('/')
                .to_string();
            let filename = rel.rsplit('/').next().unwrap_or(&rel).to_string();
            let dir = if rel.contains('/') {
                rel[..rel.len() - filename.len()]
                    .trim_end_matches('/')
                    .to_string()
            } else {
                String::new()
            };

            div()
                .px(px(10.0))
                .py(px(2.0))
                .mx(px(4.0))
                .rounded(px(3.0))
                .flex()
                .items_center()
                .gap(px(4.0))
                .text_xs()
                .hover(|s| s.bg(Colors::surface_hover()))
                .cursor_pointer()
                .child(
                    div()
                        .w(px(12.0))
                        .text_xs()
                        .text_color(status_color(code))
                        .child(status_label(code).to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .overflow_hidden()
                        .child(div().text_color(Colors::text_muted()).child(filename))
                        .when(!dir.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(Colors::text_faint())
                                    .child(dir),
                            )
                        }),
                )
                // Action buttons
                .child(
                    div()
                        .flex()
                        .gap(px(2.0))
                        .child(if is_staged {
                            div()
                                .text_xs()
                                .text_color(Colors::text_faint())
                                .cursor_pointer()
                                .hover(|s| s.text_color(Colors::error()))
                                .child("−")
                                .into_any_element()
                        } else {
                            div()
                                .flex()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Colors::text_faint())
                                        .cursor_pointer()
                                        .hover(|s| s.text_color(Colors::success()))
                                        .child("+"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(Colors::text_faint())
                                        .cursor_pointer()
                                        .hover(|s| s.text_color(Colors::error()))
                                        .child("✕"),
                                )
                                .into_any_element()
                        }),
                )
                .into_any_element()
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .child(
            div()
                .px(px(14.0))
                .py(px(5.0))
                .flex()
                .items_center()
                .justify_between()
                .text_xs()
                .text_color(Colors::text_faint())
                .child(format!("{} {}", title, count))
                .child(if is_staged {
                    div()
                        .text_xs()
                        .text_color(Colors::text_faint())
                        .cursor_pointer()
                        .hover(|s| s.text_color(Colors::error()))
                        .child("−")
                        .into_any_element()
                } else {
                    div()
                        .text_xs()
                        .text_color(Colors::text_faint())
                        .cursor_pointer()
                        .hover(|s| s.text_color(Colors::success()))
                        .child("+")
                        .into_any_element()
                }),
        )
        .children(file_items)
}
