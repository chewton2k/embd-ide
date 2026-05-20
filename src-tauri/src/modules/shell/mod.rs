use crate::modules::fs::ProjectRootState;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

const MAX_SESSIONS: usize = 10;

#[derive(Serialize)]
pub struct SpawnResult {
    pub id: u32,
    pub pid: Option<u32>,
}

pub struct PtyInstance {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

pub struct TerminalManager {
    sessions: HashMap<u32, PtyInstance>,
}

impl TerminalManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn kill_all(&mut self) {
        for (_, mut session) in self.sessions.drain() {
            let _ = session.child.kill();
        }
    }
}

/// Global atomic counter for terminal session IDs. Ensures IDs are unique
/// across all windows, preventing event collision when two windows both
/// listen for `terminal-output-{id}`.
static NEXT_TERMINAL_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

pub type TerminalState = Arc<Mutex<HashMap<String, TerminalManager>>>;

pub fn create_terminal_state() -> TerminalState {
    Arc::new(Mutex::new(HashMap::new()))
}

#[tauri::command]
pub fn spawn_terminal(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, TerminalState>,
    project_root: tauri::State<'_, ProjectRootState>,
    app: AppHandle,
    cwd: Option<String>,
    rows: Option<u16>,
    cols: Option<u16>,
) -> Result<SpawnResult, String> {
    let label = window.label().to_string();

    // Limit concurrent sessions per window
    {
        let managers = state.lock().map_err(|e| e.to_string())?;
        if let Some(manager) = managers.get(&label) {
            if manager.sessions.len() >= MAX_SESSIONS {
                return Err("Maximum number of terminal sessions reached".to_string());
            }
        }
    }

    // Validate cwd against project root
    let cwd = {
        let map = project_root.blocking_read();
        let root_path = map
            .get(&label)
            .and_then(|opt| opt.as_ref())
            .ok_or_else(|| "No project is open. Open a folder first.".to_string())?;
        let cwd_path = cwd.as_ref().map(std::path::PathBuf::from);
        let dir_to_check = cwd_path.as_deref().unwrap_or(root_path.as_path());
        let canonical =
            std::fs::canonicalize(dir_to_check).map_err(|e| format!("Invalid cwd: {}", e))?;
        if !canonical.starts_with(root_path) {
            return Err("Access denied: terminal cwd is outside the project directory".to_string());
        }
        Some(canonical)
    };

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

    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    let pid = child.process_id();
    drop(pair.slave);

    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;

    let mut managers = state.lock().map_err(|e| e.to_string())?;
    let manager = managers.entry(label).or_insert_with(TerminalManager::new);
    let id = NEXT_TERMINAL_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    manager.sessions.insert(
        id,
        PtyInstance {
            writer,
            master: pair.master,
            child,
        },
    );
    drop(managers);

    // Spawn reader thread — emits "terminal-output" events to the spawning window only
    let event_name = format!("terminal-output-{}", id);
    let exit_event_name = format!("terminal-exit-{}", id);
    let window_label = window.label().to_string();
    std::thread::spawn(move || {
        let mut buf = [0u8; 16384];
        let mut pending = Vec::new();
        let target = tauri::EventTarget::WebviewWindow { label: window_label.clone() };
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
                        let _ = app.emit_to(target.clone(), &event_name, data);
                        pending.drain(..valid_len);
                    }
                }
                Err(_) => break,
            }
        }
        // Flush any remaining bytes
        if !pending.is_empty() {
            let data = String::from_utf8_lossy(&pending).to_string();
            let _ = app.emit_to(target.clone(), &event_name, data);
        }
        // Notify frontend that this terminal session has exited
        let _ = app.emit_to(target, &exit_event_name, ());
    });

    Ok(SpawnResult { id, pid })
}

#[tauri::command]
pub fn write_terminal(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, TerminalState>,
    id: u32,
    data: String,
) -> Result<(), String> {
    let mut managers = state.lock().map_err(|e| e.to_string())?;
    let manager = managers
        .get_mut(window.label())
        .ok_or("No terminal manager for this window")?;
    let session = manager
        .sessions
        .get_mut(&id)
        .ok_or("Terminal session not found")?;
    session
        .writer
        .write_all(data.as_bytes())
        .map_err(|e| e.to_string())?;
    // No flush needed: PTY master fd is unbuffered — the kernel delivers
    // bytes to the slave process immediately after write(). Flushing added
    // one syscall per keystroke with zero benefit.
    Ok(())
}

#[tauri::command]
pub fn kill_terminal(window: tauri::WebviewWindow, state: tauri::State<'_, TerminalState>, id: u32) -> Result<(), String> {
    let mut managers = state.lock().map_err(|e| e.to_string())?;
    if let Some(manager) = managers.get_mut(window.label()) {
        if let Some(mut session) = manager.sessions.remove(&id) {
            let _ = session.child.kill();
            std::thread::spawn(move || {
                let _ = session.child.wait();
            });
        }
    }
    Ok(())
}

#[tauri::command]
pub fn resize_terminal(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, TerminalState>,
    id: u32,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let mut managers = state.lock().map_err(|e| e.to_string())?;
    let manager = managers
        .get_mut(window.label())
        .ok_or("No terminal manager for this window")?;
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

// ── Command capture (for agent tool-calling and self-verify) ──

#[derive(Serialize)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Run a shell command and capture its output (stdout, stderr, exit code).
/// Used by the agent's run_command tool and self-verify loop.
/// Enforces a timeout to prevent runaway processes.
#[tauri::command]
pub async fn run_command_capture(
    window: tauri::WebviewWindow,
    command: String,
    cwd: String,
    timeout_ms: u64,
    state: tauri::State<'_, ProjectRootState>,
) -> Result<CommandOutput, String> {
    // Validate cwd is within project root (async context — must use .read().await)
    let label = window.label().to_string();
    let root = {
        let map = state.read().await;
        map.get(&label)
            .and_then(|opt| opt.as_ref())
            .ok_or("No project is open")?
            .clone()
    };
    let cwd_path = std::fs::canonicalize(&cwd).map_err(|e| format!("Invalid cwd: {}", e))?;
    if !cwd_path.starts_with(&root) {
        return Err("Access denied: cwd is outside the project directory".into());
    }

    // Spawn the command via sh -c (cross-platform shell execution)
    let child = tokio::process::Command::new("sh")
        .args(["-c", &command])
        .current_dir(&cwd_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn command: {}", e))?;

    // Wait with timeout
    let timeout = std::time::Duration::from_millis(timeout_ms.max(1000).min(120_000));
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| format!("Command timed out after {}ms", timeout_ms))?
        .map_err(|e| format!("Command failed: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(CommandOutput { stdout, stderr, exit_code })
}
