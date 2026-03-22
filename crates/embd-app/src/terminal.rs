use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gpui::*;
use gpui::prelude::FluentBuilder as _;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use crate::theme::Colors;

const TERM_ROWS: u16 = 24;
const TERM_COLS: u16 = 80;
const SCROLLBACK: usize = 1000;

// ── Terminal session ────────────────────────────────────────────────

struct TermSession {
    id: u32,
    name: String,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    parser: vt100::Parser,
    alive: Arc<Mutex<bool>>,
    /// Keep the child process alive for the session's lifetime.
    /// Dropping this early can orphan or kill the shell process.
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

// ── Terminal pane (GPUI entity) ─────────────────────────────────────

pub enum TerminalEvent {
    AllSessionsClosed,
}

impl EventEmitter<TerminalEvent> for TerminalPane {}

pub struct TerminalPane {
    focus_handle: FocusHandle,
    sessions: Vec<TermSession>,
    active: usize,
    next_id: u32,
    cwd: Option<PathBuf>,
    rx: flume::Receiver<(u32, Vec<u8>)>,
    tx: flume::Sender<(u32, Vec<u8>)>,
    exit_rx: flume::Receiver<u32>,
    exit_tx: flume::Sender<u32>,
    cursor_visible: bool,
}

impl TerminalPane {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (tx, rx) = flume::unbounded();
        let (exit_tx, exit_rx) = flume::unbounded();

        // Poll for PTY output every 16ms
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(16)).await;
                let ok = this
                    .update(cx, |this, cx| {
                        this.poll(cx);
                    })
                    .is_ok();
                if !ok {
                    break;
                }
            }
        })
        .detach();

        // Cursor blink every 500ms
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(500)).await;
                let ok = this
                    .update(cx, |this, cx| {
                        if !this.sessions.is_empty() {
                            this.cursor_visible = !this.cursor_visible;
                            cx.notify();
                        }
                    })
                    .is_ok();
                if !ok {
                    break;
                }
            }
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            sessions: Vec::new(),
            active: 0,
            next_id: 1,
            cwd: None,
            rx,
            tx,
            exit_rx,
            exit_tx,
            cursor_visible: true,
        }
    }

    pub fn focus(&self, window: &mut Window) {
        self.focus_handle.focus(window);
    }

    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = Some(cwd);
    }

    pub fn has_sessions(&self) -> bool {
        !self.sessions.is_empty()
    }

    pub fn spawn_session(&mut self, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;

        let pty_system = native_pty_system();
        let pair = match pty_system.openpty(PtySize {
            rows: TERM_ROWS,
            cols: TERM_COLS,
            pixel_width: 0,
            pixel_height: 0,
        }) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to open PTY: {e}");
                return;
            }
        };

        let mut cmd = CommandBuilder::new_default_prog();
        if let Some(ref cwd) = self.cwd {
            cmd.cwd(cwd);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to spawn shell: {e}");
                return;
            }
        };

        let writer: Box<dyn Write + Send> = match pair.master.take_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("Failed to get PTY writer: {e}");
                return;
            }
        };
        let reader = match pair.master.try_clone_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Failed to get PTY reader: {e}");
                return;
            }
        };

        let alive = Arc::new(Mutex::new(true));

        let session = TermSession {
            id,
            name: format!("Terminal {}", id),
            writer: Arc::new(Mutex::new(writer)),
            parser: vt100::Parser::new(TERM_ROWS, TERM_COLS, SCROLLBACK),
            alive: alive.clone(),
            _child: child,
        };

        // Reader thread — sends raw bytes
        let tx = self.tx.clone();
        let exit_tx = self.exit_tx.clone();
        let alive_clone = alive.clone();
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let _ = tx.send((id, buf[..n].to_vec()));
                    }
                    Err(_) => break,
                }
            }
            if let Ok(mut alive) = alive_clone.lock() {
                *alive = false;
            }
            let _ = exit_tx.send(id);
        });

        self.sessions.push(session);
        self.active = self.sessions.len() - 1;
        cx.notify();
    }

    fn kill_session(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.sessions.len() {
            return;
        }
        self.sessions.remove(index);
        if self.active >= self.sessions.len() && !self.sessions.is_empty() {
            self.active = self.sessions.len() - 1;
        }
        if self.sessions.is_empty() {
            cx.emit(TerminalEvent::AllSessionsClosed);
        }
        cx.notify();
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;

        // Drain raw output and feed into vt100 parser
        while let Ok((id, data)) = self.rx.try_recv() {
            if let Some(session) = self.sessions.iter_mut().find(|s| s.id == id) {
                session.parser.process(&data);
                changed = true;
            }
            // Reset cursor visible on new output
            self.cursor_visible = true;
        }

        // Drain exits — auto-remove dead sessions
        while let Ok(id) = self.exit_rx.try_recv() {
            self.sessions.retain(|s| s.id != id);
            if self.active >= self.sessions.len() && !self.sessions.is_empty() {
                self.active = self.sessions.len() - 1;
            }
            changed = true;
        }

        if changed {
            if self.sessions.is_empty() {
                cx.emit(TerminalEvent::AllSessionsClosed);
            }
            cx.notify();
        }
    }

    fn write_to_active(&mut self, data: &[u8]) {
        if let Some(session) = self.sessions.get(self.active) {
            if let Ok(mut w) = session.writer.lock() {
                let _ = w.write_all(data);
                let _ = w.flush();
            }
        }
        // Reset blink so cursor stays visible while typing
        self.cursor_visible = true;
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;

        // Let Cmd+key combos pass through to workspace actions
        if ks.modifiers.platform {
            return;
        }

        if ks.modifiers.control {
            let key = ks.key.as_str();
            if key.len() == 1 {
                let ch = key.as_bytes()[0];
                if ch.is_ascii_lowercase() {
                    let ctrl_code = ch - b'a' + 1;
                    self.write_to_active(&[ctrl_code]);
                    cx.notify();
                    return;
                }
            }
        }

        let key = ks.key.as_str();
        match key {
            "enter" => self.write_to_active(b"\r"),
            "backspace" => self.write_to_active(b"\x7f"),
            "tab" => self.write_to_active(b"\t"),
            "escape" => self.write_to_active(b"\x1b"),
            "up" => self.write_to_active(b"\x1b[A"),
            "down" => self.write_to_active(b"\x1b[B"),
            "right" => self.write_to_active(b"\x1b[C"),
            "left" => self.write_to_active(b"\x1b[D"),
            "home" => self.write_to_active(b"\x1b[H"),
            "end" => self.write_to_active(b"\x1b[F"),
            "delete" => self.write_to_active(b"\x1b[3~"),
            _ => {
                if let Some(ref ch) = ks.key_char {
                    self.write_to_active(ch.as_bytes());
                } else if key.len() == 1 {
                    self.write_to_active(key.as_bytes());
                } else {
                    return;
                }
            }
        }
        cx.notify();
    }
}

// ── Color conversion ─────────────────────────────────────────────────

fn vt100_color_to_hsla(color: vt100::Color, is_fg: bool) -> Hsla {
    match color {
        vt100::Color::Default => {
            if is_fg { Colors::text() } else { hsla(0.0, 0.0, 0.0, 0.0) }
        }
        vt100::Color::Idx(idx) => ansi_index_to_hsla(idx),
        vt100::Color::Rgb(r, g, b) => {
            let color: Hsla = rgb(
                ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
            ).into();
            color
        }
    }
}

fn ansi_index_to_hsla(idx: u8) -> Hsla {
    // Standard 16 ANSI colors (Catppuccin-inspired)
    match idx {
        0 => rgb(0x45475a).into(),   // black
        1 => rgb(0xf38ba8).into(),   // red
        2 => rgb(0xa6e3a1).into(),   // green
        3 => rgb(0xf9e2af).into(),   // yellow
        4 => rgb(0x89b4fa).into(),   // blue
        5 => rgb(0xf5c2e7).into(),   // magenta
        6 => rgb(0x94e2d5).into(),   // cyan
        7 => rgb(0xbac2de).into(),   // white
        8 => rgb(0x585b70).into(),   // bright black
        9 => rgb(0xf38ba8).into(),   // bright red
        10 => rgb(0xa6e3a1).into(),  // bright green
        11 => rgb(0xf9e2af).into(),  // bright yellow
        12 => rgb(0x89b4fa).into(),  // bright blue
        13 => rgb(0xf5c2e7).into(),  // bright magenta
        14 => rgb(0x94e2d5).into(),  // bright cyan
        15 => rgb(0xa6adc8).into(),  // bright white
        // 216 color cube (indices 16-231)
        16..=231 => {
            let idx = idx - 16;
            let r = (idx / 36) * 51;
            let g = ((idx % 36) / 6) * 51;
            let b = (idx % 6) * 51;
            let color: Hsla = rgb(
                ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
            ).into();
            color
        }
        // Grayscale (indices 232-255)
        _ => {
            let v = 8 + (idx - 232) * 10;
            let color: Hsla = rgb(
                ((v as u32) << 16) | ((v as u32) << 8) | (v as u32),
            ).into();
            color
        }
    }
}

// ── Render ───────────────────────────────────────────────────────────

impl Render for TerminalPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_focused = self.focus_handle.is_focused(window)
            || self.focus_handle.contains_focused(window, cx);
        let cursor_visible = self.cursor_visible;
        if self.sessions.is_empty() {
            return div()
                .id("terminal-empty")
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .text_color(Colors::text_faint())
                .child("No sessions — press + to start")
                .into_any_element();
        }

        let active_alive = self
            .sessions
            .get(self.active)
            .and_then(|s| s.alive.lock().ok().map(|g| *g))
            .unwrap_or(false);

        // Read terminal screen from vt100 parser
        let screen_lines: Vec<Vec<(String, Hsla, Hsla, bool, bool)>> = self
            .sessions
            .get(self.active)
            .map(|s| {
                let screen = s.parser.screen();
                let rows = screen.size().0;
                let cols = screen.size().1;
                (0..rows)
                    .map(|row| {
                        let mut line: Vec<(String, Hsla, Hsla, bool, bool)> = Vec::new();
                        let mut col = 0u16;
                        while col < cols {
                            let cell = screen.cell(row, col);
                            let (contents, fg, bg, bold, inverse) = match cell {
                                Some(cell) => {
                                    let contents = cell.contents();
                                    let fg = vt100_color_to_hsla(cell.fgcolor(), true);
                                    let bg = vt100_color_to_hsla(cell.bgcolor(), false);
                                    let bold = cell.bold();
                                    let inverse = cell.inverse();
                                    // Skip wide char trailing cells
                                    if contents.is_empty() {
                                        col += 1;
                                        continue;
                                    }
                                    (contents, fg, bg, bold, inverse)
                                }
                                None => {
                                    col += 1;
                                    continue;
                                }
                            };
                            line.push((contents.to_string(), fg, bg, bold, inverse));
                            col += 1;
                        }
                        line
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Cursor position
        let cursor_pos = self
            .sessions
            .get(self.active)
            .map(|s| {
                let screen = s.parser.screen();
                let pos = screen.cursor_position();
                (pos.0 as usize, pos.1 as usize)
            })
            .unwrap_or((0, 0));

        // Tab bar
        let tab_items: Vec<AnyElement> = self.sessions.iter().enumerate().map(|(i, s)| {
            let is_active = i == self.active;
            let alive = s.alive.lock().ok().map_or(false, |g| *g);
            let label = if alive {
                s.name.clone()
            } else {
                format!("{} (exited)", s.name)
            };

            div()
                .id(SharedString::from(format!("term-tab-{}", i)))
                .px(px(10.0))
                .py(px(4.0))
                .text_xs()
                .cursor_pointer()
                .flex()
                .items_center()
                .gap(px(6.0))
                .text_color(if is_active {
                    Colors::text_muted()
                } else {
                    Colors::text_faint()
                })
                .when(is_active, |d| d.bg(Colors::bg_base()).rounded_t(px(3.0)))
                .hover(|s| s.text_color(Colors::text_muted()))
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.active = i;
                    cx.notify();
                }))
                .child(label)
                .child(
                    div()
                        .id(SharedString::from(format!("term-close-{}", i)))
                        .cursor_pointer()
                        .text_color(Colors::text_faint())
                        .hover(|s| s.text_color(Colors::text_muted()))
                        .rounded(px(2.0))
                        .child("×")
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.kill_session(i, cx);
                        })),
                )
                .into_any_element()
        }).collect();

        let tab_bar = div()
            .h(px(28.0))
            .w_full()
            .flex()
            .items_end()
            .gap(px(1.0))
            .px(px(6.0))
            .bg(Colors::bg_surface())
            .border_b_1()
            .border_color(Colors::border_subtle())
            .children(tab_items)
            .child(
                div()
                    .id("term-new-tab")
                    .px(px(6.0))
                    .py(px(4.0))
                    .text_xs()
                    .text_color(Colors::text_faint())
                    .cursor_pointer()
                    .hover(|s| s.text_color(Colors::text_muted()))
                    .child("+")
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.spawn_session(cx);
                    })),
            );

        // Render screen lines
        let rendered_lines: Vec<AnyElement> = screen_lines
            .iter()
            .enumerate()
            .map(|(row_idx, line)| {
                let is_cursor_row = row_idx == cursor_pos.0;

                // Build spans for this row
                let spans: Vec<AnyElement> = line
                    .iter()
                    .enumerate()
                    .map(|(col_idx, (text, fg, bg, bold, inverse))| {
                        let is_cursor = is_cursor_row && col_idx == cursor_pos.1 && active_alive;
                        let (fg, bg) = if *inverse {
                            (*bg, *fg)
                        } else {
                            (*fg, *bg)
                        };

                        let mut el = div().text_color(fg);
                        if bg.a > 0.0 {
                            el = el.bg(bg);
                        }
                        if *bold {
                            el = el.font_weight(FontWeight::BOLD);
                        }
                        if is_cursor && is_focused && cursor_visible {
                            // Solid block cursor when focused and visible
                            el = el.bg(Colors::text()).text_color(Colors::bg_base());
                        } else if is_cursor && is_focused {
                            // Blink off phase — show a subtle underline
                            el = el.border_b_2().border_color(Colors::text());
                        } else if is_cursor {
                            // Unfocused — dim hollow cursor
                            el = el.border_1().border_color(Colors::text_muted());
                        }
                        el.child(text.clone()).into_any_element()
                    })
                    .collect();

                // If line is empty, show a space to maintain line height
                if spans.is_empty() {
                    div().child(" ").into_any_element()
                } else {
                    div().flex().children(spans).into_any_element()
                }
            })
            .collect();

        div()
            .id("terminal-pane")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::handle_key_down))
            .size_full()
            .flex()
            .flex_col()
            .bg(Colors::bg_base())
            .child(tab_bar)
            .child(
                div()
                    .id("terminal-output")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(10.0))
                    .py(px(6.0))
                    .text_xs()
                    .font_family("monospace")
                    .children(rendered_lines),
            )
            .into_any_element()
    }
}
