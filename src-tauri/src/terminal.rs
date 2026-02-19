use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

pub struct PtyInstance {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
}

pub struct TerminalManager {
    sessions: HashMap<u32, PtyInstance>,
    next_id: u32,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            next_id: 1,
        }
    }
}

pub type TerminalState = Arc<Mutex<TerminalManager>>;

pub fn create_terminal_state() -> TerminalState {
    Arc::new(Mutex::new(TerminalManager::new()))
}

#[tauri::command]
pub fn spawn_terminal(
    state: tauri::State<'_, TerminalState>,
    app: AppHandle,
    cwd: Option<String>,
    rows: Option<u16>,
    cols: Option<u16>,
) -> Result<u32, String> {
    let pty_system = native_pty_system();

    let initial_rows = rows.unwrap_or(24);
    let initial_cols = cols.unwrap_or(80);

    let pair = pty_system
        .openpty(PtySize {
            rows: initial_rows,
            cols: initial_cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;

    let mut cmd = CommandBuilder::new_default_prog();
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");

    let _child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);

    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    let mut manager = state.lock().map_err(|e| e.to_string())?;
    let id = manager.next_id;
    manager.next_id += 1;
    manager.sessions.insert(id, PtyInstance { writer, master: pair.master });
    drop(manager);

    // Spawn reader thread — emits "terminal-output" events to the frontend
    let event_name = format!("terminal-output-{}", id);
    std::thread::spawn(move || {
        let mut buf = [0u8; 16384];
        let mut pending = Vec::new();
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    // Find the last valid UTF-8 boundary to avoid splitting multi-byte chars
                    let valid_len = {
                        let mut len = pending.len();
                        // Walk back up to 3 bytes to find a valid UTF-8 boundary
                        while len > 0 && std::str::from_utf8(&pending[..len]).is_err() {
                            len -= 1;
                        }
                        len
                    };
                    if valid_len > 0 {
                        let data = String::from_utf8_lossy(&pending[..valid_len]).to_string();
                        let _ = app.emit(&event_name, data);
                        pending.drain(..valid_len);
                    }
                }
                Err(_) => break,
            }
        }
        // Flush any remaining bytes
        if !pending.is_empty() {
            let data = String::from_utf8_lossy(&pending).to_string();
            let _ = app.emit(&event_name, data);
        }
    });

    Ok(id)
}

#[tauri::command]
pub fn write_terminal(
    state: tauri::State<'_, TerminalState>,
    id: u32,
    data: String,
) -> Result<(), String> {
    let mut manager = state.lock().map_err(|e| e.to_string())?;
    let session = manager
        .sessions
        .get_mut(&id)
        .ok_or("Terminal session not found")?;
    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| e.to_string())?;
    session.writer.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn kill_terminal(
    state: tauri::State<'_, TerminalState>,
    id: u32,
) -> Result<(), String> {
    let mut manager = state.lock().map_err(|e| e.to_string())?;
    manager.sessions.remove(&id);
    Ok(())
}

#[tauri::command]
pub fn resize_terminal(
    state: tauri::State<'_, TerminalState>,
    id: u32,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let mut manager = state.lock().map_err(|e| e.to_string())?;
    let session = manager
        .sessions
        .get_mut(&id)
        .ok_or("Terminal session not found")?;
    session
        .master
        .resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}
