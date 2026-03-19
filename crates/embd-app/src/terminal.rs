use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gpui::*;
use gpui::prelude::FluentBuilder as _;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use crate::theme::Colors;

const MAX_OUTPUT_BYTES: usize = 512 * 1024; // 512KB scrollback limit

// ── Terminal session ────────────────────────────────────────────────

struct TermSession {
    id: u32,
    name: String,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    output: Arc<Mutex<String>>,
    alive: Arc<Mutex<bool>>,
}

// ── Terminal pane (GPUI entity) ─────────────────────────────────────

pub struct TerminalPane {
    focus_handle: FocusHandle,
    sessions: Vec<TermSession>,
    active: usize,
    next_id: u32,
    cwd: Option<PathBuf>,
    rx: flume::Receiver<(u32, String)>,
    tx: flume::Sender<(u32, String)>,
    exit_rx: flume::Receiver<u32>,
    exit_tx: flume::Sender<u32>,
}

impl TerminalPane {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (tx, rx) = flume::unbounded();
        let (exit_tx, exit_rx) = flume::unbounded();

        // Start a polling loop to drain output from background threads
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(32)).await;
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
        }
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
            rows: 24,
            cols: 80,
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

        let _child = match pair.slave.spawn_command(cmd) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to spawn shell: {e}");
                return;
            }
        };

        let writer: Box<dyn Write + Send> = pair.master.take_writer().unwrap();
        let reader = pair.master.try_clone_reader().unwrap();

        let output = Arc::new(Mutex::new(String::new()));
        let alive = Arc::new(Mutex::new(true));

        let session = TermSession {
            id,
            name: format!("Terminal {}", id),
            writer: Arc::new(Mutex::new(writer)),
            output: output.clone(),
            alive: alive.clone(),
        };

        // Reader thread
        let tx = self.tx.clone();
        let exit_tx = self.exit_tx.clone();
        let alive_clone = alive.clone();
        std::thread::spawn(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]).to_string();
                        // Strip common ANSI escape sequences for clean display
                        let cleaned = strip_ansi(&text);
                        let _ = tx.send((id, cleaned));
                    }
                    Err(_) => break,
                }
            }
            *alive_clone.lock().unwrap() = false;
            let _ = exit_tx.send(id);
        });

        self.sessions.push(session);
        self.active = self.sessions.len() - 1;
        cx.notify();
    }

    pub fn kill_active(&mut self, cx: &mut Context<Self>) {
        if self.sessions.is_empty() {
            return;
        }
        self.sessions.remove(self.active);
        if self.active >= self.sessions.len() && !self.sessions.is_empty() {
            self.active = self.sessions.len() - 1;
        }
        cx.notify();
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        let mut changed = false;

        // Drain output
        while let Ok((id, text)) = self.rx.try_recv() {
            if let Some(session) = self.sessions.iter().find(|s| s.id == id) {
                let mut out = session.output.lock().unwrap();
                out.push_str(&text);
                // Trim if too large
                if out.len() > MAX_OUTPUT_BYTES {
                    let drain = out.len() - MAX_OUTPUT_BYTES;
                    out.drain(..drain);
                }
                changed = true;
            }
        }

        // Drain exits
        while let Ok(id) = self.exit_rx.try_recv() {
            if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
                // Mark as dead but don't remove — user can see final output
                if let Some(session) = self.sessions.get(pos) {
                    *session.alive.lock().unwrap() = false;
                }
            }
            changed = true;
        }

        if changed {
            cx.notify();
        }
    }

    fn write_to_active(&mut self, data: &str) {
        if let Some(session) = self.sessions.get(self.active) {
            if let Ok(mut w) = session.writer.lock() {
                let _ = w.write_all(data.as_bytes());
                let _ = w.flush();
            }
        }
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
            // Ctrl+C, Ctrl+D, Ctrl+Z, etc.
            let key = ks.key.as_str();
            if key.len() == 1 {
                let ch = key.chars().next().unwrap();
                let ctrl_code = (ch as u8).wrapping_sub(b'a').wrapping_add(1);
                self.write_to_active(&String::from(ctrl_code as char));
                cx.notify();
                return;
            }
        }

        let key = ks.key.as_str();
        match key {
            "enter" => self.write_to_active("\r"),
            "backspace" => self.write_to_active("\x7f"),
            "tab" => self.write_to_active("\t"),
            "escape" => self.write_to_active("\x1b"),
            "up" => self.write_to_active("\x1b[A"),
            "down" => self.write_to_active("\x1b[B"),
            "right" => self.write_to_active("\x1b[C"),
            "left" => self.write_to_active("\x1b[D"),
            "home" => self.write_to_active("\x1b[H"),
            "end" => self.write_to_active("\x1b[F"),
            "delete" => self.write_to_active("\x1b[3~"),
            _ => {
                if let Some(ref ch) = ks.key_char {
                    self.write_to_active(ch);
                } else if key.len() == 1 {
                    self.write_to_active(key);
                } else {
                    return;
                }
            }
        }
        cx.notify();
    }
}

impl Render for TerminalPane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.sessions.is_empty() {
            return div()
                .id("terminal-empty")
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(Colors::text_muted())
                .child("No terminal sessions. Click + to create one.")
                .into_any_element();
        }

        let active_output = self
            .sessions
            .get(self.active)
            .map(|s| s.output.lock().unwrap().clone())
            .unwrap_or_default();

        let active_alive = self
            .sessions
            .get(self.active)
            .map(|s| *s.alive.lock().unwrap())
            .unwrap_or(false);

        // Terminal tab bar
        let tab_bar = div()
            .h(px(28.0))
            .w_full()
            .flex()
            .items_center()
            .gap(px(2.0))
            .px(px(8.0))
            .bg(Colors::bg_surface())
            .border_b_1()
            .border_color(Colors::border())
            .children(self.sessions.iter().enumerate().map(|(i, s)| {
                let is_active = i == self.active;
                let alive = *s.alive.lock().unwrap();
                div()
                    .px(px(8.0))
                    .py(px(4.0))
                    .text_xs()
                    .cursor_pointer()
                    .text_color(if is_active {
                        Colors::text()
                    } else {
                        Colors::text_muted()
                    })
                    .when(is_active, |d| d.bg(Colors::bg_base()))
                    .rounded_t(px(4.0))
                    .child(if alive {
                        s.name.clone()
                    } else {
                        format!("{} (exited)", s.name)
                    })
            }))
            .child(
                div()
                    .id("term-new-tab")
                    .px(px(6.0))
                    .py(px(2.0))
                    .text_xs()
                    .text_color(Colors::text_muted())
                    .cursor_pointer()
                    .hover(|s| s.text_color(Colors::text()))
                    .child("+")
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.spawn_session(cx);
                    })),
            );

        // Terminal output
        let lines: Vec<&str> = active_output.lines().collect();
        let visible_start = lines.len().saturating_sub(100); // Show last 100 lines
        let visible_lines: Vec<AnyElement> = lines[visible_start..]
            .iter()
            .map(|line| {
                div()
                    .text_color(Colors::text())
                    .child(if line.is_empty() {
                        " ".to_string()
                    } else {
                        line.to_string()
                    })
                    .into_any_element()
            })
            .collect();

        // Cursor indicator
        let cursor = if active_alive {
            div()
                .w(px(8.0))
                .h(px(14.0))
                .bg(Colors::text())
                .into_any_element()
        } else {
            div().into_any_element()
        };

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
                    .p(px(8.0))
                    .text_sm()
                    .font_family("monospace")
                    .children(visible_lines)
                    .child(cursor),
            )
            .into_any_element()
    }
}

// ── ANSI stripping ──────────────────────────────────────────────────

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // ESC sequence
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Read until a letter (the terminator)
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c.is_ascii_alphabetic() || c == 'H' || c == 'J' || c == 'K' {
                        break;
                    }
                }
            } else if chars.peek() == Some(&']') {
                // OSC sequence — read until BEL or ST
                chars.next();
                while let Some(&c) = chars.peek() {
                    chars.next();
                    if c == '\x07' {
                        break;
                    }
                    if c == '\x1b' {
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            } else if chars.peek() == Some(&'(') || chars.peek() == Some(&')') {
                chars.next();
                chars.next(); // skip charset designation
            }
            // Skip other single-char escape sequences
        } else if ch == '\r' {
            // Carriage return — overwrite current line
            // Find the last newline in output
            if let Some(nl_pos) = out.rfind('\n') {
                out.truncate(nl_pos + 1);
            } else {
                out.clear();
            }
        } else if ch == '\x08' {
            // Backspace
            out.pop();
        } else if ch >= ' ' || ch == '\n' || ch == '\t' {
            out.push(ch);
        }
        // Skip other control chars
    }
    out
}
