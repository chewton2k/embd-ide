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

// ── Styled text run (batched adjacent cells with same style) ────────

struct StyledRun {
    text: String,
    fg: Hsla,
    bg: Hsla,
    bold: bool,
    italic: bool,
    underline: bool,
    cursor: CursorStyle,
}

#[derive(Clone, Copy, PartialEq)]
enum CursorStyle {
    None,
    Block,       // focused + visible phase
    Underline,   // focused + blink-off phase
    Hollow,      // unfocused
}

impl StyledRun {
    fn can_merge(&self, fg: Hsla, bg: Hsla, bold: bool, italic: bool, underline: bool) -> bool {
        self.cursor == CursorStyle::None
            && self.fg == fg
            && self.bg == bg
            && self.bold == bold
            && self.italic == italic
            && self.underline == underline
    }
}

// ── vt100 callbacks for title tracking ──────────────────────────────

#[derive(Clone)]
struct TermCallbacks {
    title: Arc<Mutex<Option<String>>>,
}

impl vt100::Callbacks for TermCallbacks {
    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        if let Ok(s) = std::str::from_utf8(title) {
            *self.title.lock().unwrap() = Some(s.to_string());
        }
    }

    fn set_window_icon_name(&mut self, _: &mut vt100::Screen, name: &[u8]) {
        if let Ok(s) = std::str::from_utf8(name) {
            *self.title.lock().unwrap() = Some(s.to_string());
        }
    }
}

// ── Terminal session ────────────────────────────────────────────────

struct TermSession {
    id: u32,
    default_name: String,
    title: Arc<Mutex<Option<String>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    parser: vt100::Parser<TermCallbacks>,
    alive: Arc<Mutex<bool>>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl TermSession {
    fn display_name(&self) -> String {
        if let Some(ref title) = *self.title.lock().unwrap() {
            title.clone()
        } else {
            self.default_name.clone()
        }
    }
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
    blink_epoch: usize,
}

impl TerminalPane {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (tx, rx) = flume::unbounded();
        let (exit_tx, exit_rx) = flume::unbounded();

        // Poll for PTY output every 8ms (~120fps) — only notify on actual changes
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(8)).await;
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

        // Cursor blink every 500ms — only notify when cursor actually toggles
        cx.spawn(async move |this, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(500)).await;
                let ok = this
                    .update(cx, |this, cx| {
                        if !this.sessions.is_empty() {
                            let new_val = !this.cursor_visible;
                            if new_val != this.cursor_visible {
                                this.cursor_visible = new_val;
                                this.blink_epoch += 1;
                                cx.notify();
                            }
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
            blink_epoch: 0,
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

        let shell_name = std::env::var("SHELL")
            .ok()
            .and_then(|s| s.rsplit('/').next().map(String::from))
            .unwrap_or_else(|| "shell".into());
        let project_name = self
            .cwd
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string());
        let name = match project_name {
            Some(proj) => format!("{} — {}", proj, shell_name),
            None => shell_name,
        };

        let title: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let callbacks = TermCallbacks {
            title: title.clone(),
        };

        let session = TermSession {
            id,
            default_name: name,
            title,
            writer: Arc::new(Mutex::new(writer)),
            parser: vt100::Parser::new_with_callbacks(
                TERM_ROWS,
                TERM_COLS,
                SCROLLBACK,
                callbacks,
            ),
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
            // Reset cursor visible on new output so it stays solid while active
            self.cursor_visible = true;
        }

        // Drain exits
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
        self.cursor_visible = true;
    }

    /// Get application cursor mode from the active session's vt100 screen.
    fn app_cursor_mode(&self) -> bool {
        self.sessions
            .get(self.active)
            .map(|s| s.parser.screen().application_cursor())
            .unwrap_or(false)
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ks = &event.keystroke;

        // Let Cmd+key combos pass through to workspace
        if ks.modifiers.platform {
            return;
        }

        let app_cursor = self.app_cursor_mode();
        if let Some(bytes) = keystroke_to_bytes(ks, app_cursor) {
            self.write_to_active(&bytes);
            cx.notify();
        }
    }
}

// ── Keyboard mapping (xterm-compatible) ─────────────────────────────

fn keystroke_to_bytes(ks: &Keystroke, app_cursor: bool) -> Option<Vec<u8>> {
    let key = ks.key.as_str();
    let ctrl = ks.modifiers.control;
    let alt = ks.modifiers.alt;
    let shift = ks.modifiers.shift;

    // xterm modifier encoding: 1 + (shift?1:0) + (alt?2:0) + (ctrl?4:0)
    let mod_code = 1
        + if shift { 1 } else { 0 }
        + if alt { 2 } else { 0 }
        + if ctrl { 4 } else { 0 };
    let has_mods = mod_code > 1;

    match key {
        "enter" => Some(if alt { b"\x1b\r".to_vec() } else { b"\r".to_vec() }),
        "backspace" => Some(if ctrl {
            b"\x08".to_vec()
        } else if alt {
            b"\x1b\x7f".to_vec()
        } else {
            b"\x7f".to_vec()
        }),
        "tab" => Some(if shift {
            b"\x1b[Z".to_vec()
        } else {
            b"\t".to_vec()
        }),
        "escape" => Some(b"\x1b".to_vec()),
        "space" => Some(if ctrl {
            b"\x00".to_vec()
        } else if alt {
            b"\x1b ".to_vec()
        } else {
            b" ".to_vec()
        }),

        // Arrow keys — respect application cursor mode
        "up" => Some(csi_or_ss3(b'A', app_cursor, has_mods, mod_code)),
        "down" => Some(csi_or_ss3(b'B', app_cursor, has_mods, mod_code)),
        "right" => Some(csi_or_ss3(b'C', app_cursor, has_mods, mod_code)),
        "left" => Some(csi_or_ss3(b'D', app_cursor, has_mods, mod_code)),

        // Navigation
        "home" => Some(if has_mods {
            format!("\x1b[1;{}H", mod_code).into_bytes()
        } else if app_cursor {
            b"\x1bOH".to_vec()
        } else {
            b"\x1b[H".to_vec()
        }),
        "end" => Some(if has_mods {
            format!("\x1b[1;{}F", mod_code).into_bytes()
        } else if app_cursor {
            b"\x1bOF".to_vec()
        } else {
            b"\x1b[F".to_vec()
        }),
        "pageup" => Some(tilde_key(5, has_mods, mod_code)),
        "pagedown" => Some(tilde_key(6, has_mods, mod_code)),
        "insert" => Some(tilde_key(2, has_mods, mod_code)),
        "delete" => Some(tilde_key(3, has_mods, mod_code)),

        // Function keys
        "f1" => Some(ss3_func(b'P', has_mods, mod_code)),
        "f2" => Some(ss3_func(b'Q', has_mods, mod_code)),
        "f3" => Some(ss3_func(b'R', has_mods, mod_code)),
        "f4" => Some(ss3_func(b'S', has_mods, mod_code)),
        "f5" => Some(tilde_key(15, has_mods, mod_code)),
        "f6" => Some(tilde_key(17, has_mods, mod_code)),
        "f7" => Some(tilde_key(18, has_mods, mod_code)),
        "f8" => Some(tilde_key(19, has_mods, mod_code)),
        "f9" => Some(tilde_key(20, has_mods, mod_code)),
        "f10" => Some(tilde_key(21, has_mods, mod_code)),
        "f11" => Some(tilde_key(23, has_mods, mod_code)),
        "f12" => Some(tilde_key(24, has_mods, mod_code)),

        _ => {
            // Ctrl+letter → control character (optionally with alt meta prefix)
            if ctrl && !shift {
                if let Some(code) = ctrl_code_for(key) {
                    return if alt {
                        Some(vec![0x1b, code])
                    } else {
                        Some(vec![code])
                    };
                }
            }

            // Alt/Option as meta prefix — \x1b + char
            if alt {
                if let Some(ref ch) = ks.key_char {
                    let mut bytes = vec![0x1b];
                    bytes.extend_from_slice(ch.as_bytes());
                    return Some(bytes);
                } else if key.len() == 1 {
                    return Some(vec![0x1b, key.as_bytes()[0]]);
                }
            }

            // Regular character input
            if let Some(ref ch) = ks.key_char {
                Some(ch.as_bytes().to_vec())
            } else if key.len() == 1 {
                Some(key.as_bytes().to_vec())
            } else {
                None
            }
        }
    }
}

/// Arrow/cursor keys: SS3 in app mode, CSI otherwise, with modifier support.
fn csi_or_ss3(ch: u8, app_cursor: bool, has_mods: bool, mod_code: u32) -> Vec<u8> {
    if has_mods {
        format!("\x1b[1;{}{}", mod_code, ch as char).into_bytes()
    } else if app_cursor {
        vec![0x1b, b'O', ch]
    } else {
        vec![0x1b, b'[', ch]
    }
}

/// Tilde-style keys: \x1b[N~ or \x1b[N;mod~
fn tilde_key(n: u32, has_mods: bool, mod_code: u32) -> Vec<u8> {
    if has_mods {
        format!("\x1b[{};{}~", n, mod_code).into_bytes()
    } else {
        format!("\x1b[{}~", n).into_bytes()
    }
}

/// F1-F4 use SS3 without mods, CSI with mods.
fn ss3_func(ch: u8, has_mods: bool, mod_code: u32) -> Vec<u8> {
    if has_mods {
        format!("\x1b[1;{}{}", mod_code, ch as char).into_bytes()
    } else {
        vec![0x1b, b'O', ch]
    }
}

/// Map a single-char key to its Ctrl code (0x01-0x1f).
fn ctrl_code_for(key: &str) -> Option<u8> {
    if key.len() != 1 {
        return None;
    }
    let ch = key.as_bytes()[0];
    match ch {
        b'a'..=b'z' => Some(ch - b'a' + 1),
        b'@' => Some(0x00),
        b'[' => Some(0x1b),
        b'\\' => Some(0x1c),
        b']' => Some(0x1d),
        b'^' => Some(0x1e),
        b'_' => Some(0x1f),
        _ => None,
    }
}

// ── Color conversion ─────────────────────────────────────────────────

fn vt100_color_to_hsla(color: vt100::Color, is_fg: bool) -> Hsla {
    match color {
        vt100::Color::Default => {
            if is_fg {
                Colors::text()
            } else {
                hsla(0.0, 0.0, 0.0, 0.0)
            }
        }
        vt100::Color::Idx(idx) => ansi_index_to_hsla(idx),
        vt100::Color::Rgb(r, g, b) => {
            rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)).into()
        }
    }
}

fn ansi_index_to_hsla(idx: u8) -> Hsla {
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
            let i = idx - 16;
            let r = (i / 36) * 51;
            let g = ((i % 36) / 6) * 51;
            let b = (i % 6) * 51;
            rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)).into()
        }
        // Grayscale (indices 232-255)
        _ => {
            let v = 8 + (idx - 232) * 10;
            rgb(((v as u32) << 16) | ((v as u32) << 8) | (v as u32)).into()
        }
    }
}

// ── Build styled runs from a screen row ─────────────────────────────

fn build_row_runs(
    screen: &vt100::Screen,
    row: u16,
    cursor_row: u16,
    cursor_col: u16,
    cursor_style: CursorStyle,
) -> Vec<StyledRun> {
    let cols = screen.size().1;
    let is_cursor_row = row == cursor_row;
    let mut runs: Vec<StyledRun> = Vec::with_capacity(16);

    let mut col = 0u16;
    while col < cols {
        let cell = match screen.cell(row, col) {
            Some(c) => c,
            None => {
                col += 1;
                continue;
            }
        };

        let contents = cell.contents();
        // Wide char trailing cell — skip
        if contents.is_empty() {
            col += 1;
            continue;
        }

        let mut fg = vt100_color_to_hsla(cell.fgcolor(), true);
        let mut bg = vt100_color_to_hsla(cell.bgcolor(), false);
        let bold = cell.bold();
        let italic = cell.italic();
        let underline = cell.underline();

        if cell.inverse() {
            std::mem::swap(&mut fg, &mut bg);
            // If both were default, make the swap visible
            if bg.a == 0.0 {
                bg = Colors::text();
                fg = Colors::bg_base();
            }
        }

        let cell_cursor = if is_cursor_row && col == cursor_col {
            cursor_style
        } else {
            CursorStyle::None
        };

        // Try to merge into the previous run
        if cell_cursor == CursorStyle::None {
            if let Some(last) = runs.last_mut() {
                if last.can_merge(fg, bg, bold, italic, underline) {
                    last.text.push_str(contents);
                    col += 1;
                    continue;
                }
            }
        }

        runs.push(StyledRun {
            text: contents.to_string(),
            fg,
            bg,
            bold,
            italic,
            underline,
            cursor: cell_cursor,
        });

        col += 1;
    }

    // Ensure cursor is visible even on an empty/trailing position
    if is_cursor_row && cursor_style != CursorStyle::None {
        let cursor_col_usize = cursor_col as usize;
        // Check if cursor was already placed on a cell
        let cursor_placed = runs.iter().any(|r| r.cursor != CursorStyle::None);
        if !cursor_placed {
            // Cursor is past the last content — append a cursor block
            // Pad with spaces if needed
            let current_len: usize = runs.iter().map(|r| r.text.chars().count()).sum();
            if cursor_col_usize > current_len {
                runs.push(StyledRun {
                    text: " ".repeat(cursor_col_usize - current_len),
                    fg: Colors::text(),
                    bg: hsla(0.0, 0.0, 0.0, 0.0),
                    bold: false,
                    italic: false,
                    underline: false,
                    cursor: CursorStyle::None,
                });
            }
            runs.push(StyledRun {
                text: " ".to_string(),
                fg: Colors::text(),
                bg: hsla(0.0, 0.0, 0.0, 0.0),
                bold: false,
                italic: false,
                underline: false,
                cursor: cursor_style,
            });
        }
    }

    // Guarantee at least one run per line to maintain height
    if runs.is_empty() {
        runs.push(StyledRun {
            text: " ".to_string(),
            fg: Colors::text(),
            bg: hsla(0.0, 0.0, 0.0, 0.0),
            bold: false,
            italic: false,
            underline: false,
            cursor: CursorStyle::None,
        });
    }

    runs
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

        // Determine cursor style
        let cursor_style = if !active_alive {
            CursorStyle::None
        } else if is_focused && cursor_visible {
            CursorStyle::Block
        } else if is_focused {
            CursorStyle::Underline
        } else {
            CursorStyle::Hollow
        };

        // Build all row runs from the vt100 screen
        let row_runs: Vec<Vec<StyledRun>> = self
            .sessions
            .get(self.active)
            .map(|s| {
                let screen = s.parser.screen();
                let rows = screen.size().0;
                let cursor = screen.cursor_position();
                (0..rows)
                    .map(|row| {
                        build_row_runs(screen, row, cursor.0, cursor.1, cursor_style)
                    })
                    .collect()
            })
            .unwrap_or_default();

        // ── Tab bar ─────────────────────────────────────────────────
        let tab_items: Vec<AnyElement> = self
            .sessions
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let is_active = i == self.active;
                let alive = s.alive.lock().ok().map_or(false, |g| *g);
                let display = s.display_name();
                let label = if alive {
                    display
                } else {
                    format!("{} (exited)", display)
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
            })
            .collect();

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

        // ── Render rows from batched runs ───────────────────────────
        let rendered_lines: Vec<AnyElement> = row_runs
            .into_iter()
            .map(|runs| {
                let spans: Vec<AnyElement> = runs
                    .into_iter()
                    .map(|run| {
                        let mut el = div().text_color(run.fg);

                        if run.bg.a > 0.0 {
                            el = el.bg(run.bg);
                        }
                        if run.bold {
                            el = el.font_weight(FontWeight::BOLD);
                        }
                        if run.italic {
                            el = el.italic();
                        }
                        if run.underline {
                            el = el.underline().text_decoration_color(run.fg);
                        }

                        // Cursor styling
                        match run.cursor {
                            CursorStyle::Block => {
                                el = el.bg(Colors::text()).text_color(Colors::bg_base());
                            }
                            CursorStyle::Underline => {
                                el = el.border_b_2().border_color(Colors::text());
                            }
                            CursorStyle::Hollow => {
                                el = el.border_1().border_color(Colors::text_muted());
                            }
                            CursorStyle::None => {}
                        }

                        el.child(run.text).into_any_element()
                    })
                    .collect();

                div().flex().children(spans).into_any_element()
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
                    .text_size(px(13.0))
                    .line_height(px(18.0))
                    .font_family("monospace")
                    .children(rendered_lines),
            )
            .into_any_element()
    }
}
