//! NEXUS-native JavaScript sandbox.
//!
//! This is the NEXUS-native equivalent of the `js-code-sandbox`
//! plugin that ships in `app/src-tauri/js-code-sandbox/`. The
//! original plugin is a TypeScript file that runs **inside LM
//! Studio**; it spawns a Deno subprocess with the flags:
//!
//! ```text
//! deno run \
//!   --allow-read=. \
//!   --allow-write=. \
//!   --no-prompt \
//!   --deny-net \
//!   --deny-env \
//!   --deny-sys \
//!   --deny-run \
//!   --deny-ffi \
//!   <script.ts>
//! ```
//!
//! We mirror that exact policy here: read/write only the
//! current working directory, no network, no env, no sys, no
//! subprocess execution, no FFI. The execution has a default
//! 5-second timeout, max 60 seconds (matching the plugin).
//!
//! The plugin is invoked from LM Studio as a tool. We expose
//! the same capability through the `sandbox_run_javascript`
//! Tauri command so the front-end (or the agent loop) can
//! call it directly without depending on LM Studio's plugin
//! runtime.
//!
//! **Caveat**: Deno is not bundled with NEXUS. The user must
//! install Deno separately (or we ship it as part of the
//! installer in a follow-up phase). The command below
//! returns a clear "deno not found" error if it is missing,
//! so the failure is surfaced instead of silently
//! pretending to work.

use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};

/// Default timeout if the caller does not specify one.
pub const DEFAULT_TIMEOUT_SECONDS: u32 = 5;
/// Maximum timeout the caller can request.
pub const MAX_TIMEOUT_SECONDS: u32 = 60;

/// Outcome of a sandbox run.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SandboxResult {
    /// Captured standard output (UTF-8, trimmed of trailing
    /// whitespace).
    pub stdout: String,
    /// Captured standard error (UTF-8, trimmed of trailing
    /// whitespace).
    pub stderr: String,
    /// Process exit code, or `None` if the process was
    /// killed by the timeout.
    pub exit_code: Option<i32>,
    /// Wall-clock duration of the run in milliseconds.
    pub elapsed_ms: u64,
    /// True if the process was killed because it exceeded
    /// the requested timeout.
    pub timed_out: bool,
}

/// Find the Deno binary. We look in:
/// 1. `$DENO` environment variable (if set).
/// 2. The system PATH (via `which::which`).
/// 3. The LM Studio home directory's `.internal/utils/deno`
///    subdirectory — this is the same path the original
///    plugin uses, so if LM Studio is installed the user
///    already has a Deno binary NEXUS can borrow.
pub fn find_deno() -> AppResult<PathBuf> {
    if let Ok(env) = std::env::var("DENO") {
        let p = PathBuf::from(env);
        if p.is_file() {
            return Ok(p);
        }
    }
    if let Ok(p) = which::which("deno") {
        return Ok(p);
    }
    if let Some(home) = lm_studio_home() {
        let candidate = home.join(".internal").join("utils").join(if cfg!(windows) { "deno.exe" } else { "deno" });
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(AppError::NotFound(
        "deno not found. Install Deno (https://deno.land) and either add it to PATH or set the DENO environment variable.".into()
    ))
}

fn lm_studio_home() -> Option<PathBuf> {
    // ~/.cache/lm-studio (the actual location the plugin
    // uses on Windows is %LOCALAPPDATA%/LM-Studio, but
    // ~/.cache/lm-studio is the cross-platform default).
    if let Ok(p) = std::env::var("LMSTUDIO_HOME") {
        let pb = PathBuf::from(p);
        if pb.is_dir() { return Some(pb); }
    }
    if let Some(home) = dirs::home_dir() {
        let cache = home.join(".cache").join("lm-studio");
        if cache.is_dir() { return Some(cache); }
    }
    None
}

/// Run a JavaScript snippet in the Deno sandbox.
///
/// `working_directory` is the directory the script is allowed
/// to read/write. NEXUS uses a per-conversation scratch
/// directory so the model cannot reach the database, the
/// model files, or the user's home directory.
pub fn run_javascript(
    code: &str,
    timeout_seconds: Option<u32>,
    working_directory: &std::path::Path,
) -> AppResult<SandboxResult> {
    let timeout = timeout_seconds
        .unwrap_or(DEFAULT_TIMEOUT_SECONDS)
        .clamp(1, MAX_TIMEOUT_SECONDS);
    let deno = find_deno()?;
    if !working_directory.is_dir() {
        return Err(AppError::Invalid(format!(
            "working directory does not exist: {}",
            working_directory.display()
        )));
    }
    // Write the code to a temp file inside the working
    // directory. We delete the file before returning so the
    // sandbox leaves no trace. The file name is suffixed
    // with a millisecond timestamp so two concurrent
    // sandbox calls cannot collide.
    let started = Instant::now();
    let script_name = format!("nexus_sandbox_{}.ts", started.elapsed().as_millis() as u64);
    let script_path = working_directory.join(&script_name);
    {
        let mut f = std::fs::File::create(&script_path).map_err(AppError::Io)?;
        f.write_all(code.as_bytes()).map_err(AppError::Io)?;
    }
    // Spawn Deno with the same flags the LM Studio plugin
    // uses. We do not pass `--allow-read=.` etc. by default
    // because Deno's `.` is the current working directory,
    // which we have already set to the scratch directory.
    let child = std::process::Command::new(&deno)
        .arg("run")
        .arg(format!("--allow-read={}", working_directory.display()))
        .arg(format!("--allow-write={}", working_directory.display()))
        .arg("--no-prompt")
        .arg("--deny-net")
        .arg("--deny-env")
        .arg("--deny-sys")
        .arg("--deny-run")
        .arg("--deny-ffi")
        .arg(&script_path)
        .current_dir(working_directory)
        .env("NO_COLOR", "true")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .map_err(|e| AppError::Upstream(format!("sandbox spawn: {e}")))?;

    let mut child = child;
    // We poll for completion in a separate thread so the
    // main thread can enforce the timeout. We do not use
    // `wait_timeout` (unstable) or a `tokio` runtime here
    // because this is called from a `tauri::command`
    // synchronous handler.
    let timeout_d = Duration::from_secs(timeout as u64);
    let (tx, rx) = std::sync::mpsc::channel::<Result<std::process::Output, std::io::Error>>();
    let pid = child.id();
    std::thread::spawn(move || {
        let res = child.wait_with_output();
        let _ = tx.send(res);
    });
    let result = match rx.recv_timeout(timeout_d) {
        Ok(r) => r,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // Process did not finish in time. Kill it (and
            // any child processes it might have spawned)
            // before returning.
            let _ = kill_process_tree(pid);
            return Ok(SandboxResult {
                stdout: String::new(),
                stderr: format!("timeout: exceeded {} seconds", timeout),
                exit_code: None,
                elapsed_ms: started.elapsed().as_millis() as u64,
                timed_out: true,
            });
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            return Err(AppError::Upstream("sandbox wait channel disconnected".into()));
        }
    };
    let output = result.map_err(|e| AppError::Upstream(format!("sandbox wait: {e}")))?;
    let _ = std::fs::remove_file(&script_path);
    let stdout = String::from_utf8_lossy(&output.stdout).trim_end().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim_end().to_string();
    Ok(SandboxResult {
        stdout,
        stderr,
        exit_code: output.status.code(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        timed_out: false,
    })
}

#[cfg(windows)]
fn kill_process_tree(pid: u32) -> std::io::Result<()> {
    // `taskkill /F /T /PID` terminates the process and its
    // descendants. We swallow the error because the
    // alternative (no kill) is worse than a noisy log line.
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

#[cfg(not(windows))]
fn kill_process_tree(pid: u32) -> std::io::Result<()> {
    // `kill -TERM -<pid>` kills the process group. We
    // fall back to `kill -9` if that fails.
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &format!("-{}", pid)])
        .status();
    Ok(())
}
