//! Tauri command wrappers around the NEXUS-native JavaScript
//! sandbox. The actual sandbox implementation lives in
//! `crate::sandbox` and is library-level so it can be
//! exercised from the test suite without spinning up Tauri.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use nexus_core::data_dir;
use nexus_core::error::AppResult;
use nexus_core::sandbox::{self, SandboxResult};

/// Request body for `sandbox_run_javascript`.
#[derive(Debug, Deserialize)]
pub struct SandboxRunRequest {
    /// JavaScript / TypeScript source to execute.
    pub code: String,
    /// Optional timeout in seconds, default 5, max 60.
    /// Mirrors the LM Studio `js-code-sandbox` plugin.
    #[serde(default)]
    pub timeout_seconds: Option<u32>,
    /// Optional override for the working directory. The
    /// script is allowed to read and write only inside this
    /// directory. Defaults to a per-app scratch directory.
    #[serde(default)]
    pub working_directory: Option<String>,
}

/// Tauri command: run a JavaScript snippet in the Deno
/// sandbox. Returns captured stdout / stderr / exit code.
///
/// The command is intentionally **synchronous** from the
/// Tauri runtime's perspective (we use `tauri::command`
/// without `async`). The internal sandbox already runs the
/// subprocess with a hard timeout, so a slow script cannot
/// pin a Tauri command worker.
#[tauri::command]
pub fn sandbox_run_javascript(req: SandboxRunRequest) -> AppResult<SandboxResult> {
    let cwd = match req.working_directory.as_deref() {
        Some(p) => PathBuf::from(p),
        None => default_sandbox_working_dir()?,
    };
    sandbox::run_javascript(&req.code, req.timeout_seconds, &cwd)
}

/// Tauri command: return a small JSON object describing the
/// sandbox's current state. The frontend uses this to show
/// "Deno not found" in the UI before the user tries to run
/// a snippet.
#[derive(Debug, Serialize)]
pub struct SandboxStatus {
    pub deno_path: Option<String>,
    pub deno_version: Option<String>,
    pub default_working_directory: String,
}

#[tauri::command]
pub fn sandbox_status() -> AppResult<SandboxStatus> {
    let cwd = default_sandbox_working_dir()?;
    let deno = nexus_core::sandbox::find_deno().ok();
    let version = deno.as_ref().and_then(|p| {
        std::process::Command::new(p)
            .arg("--version")
            .output()
            .ok()
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.trim().to_string())
    });
    Ok(SandboxStatus {
        deno_path: deno.map(|p| p.to_string_lossy().to_string()),
        deno_version: version,
        default_working_directory: cwd.to_string_lossy().to_string(),
    })
}

fn default_sandbox_working_dir() -> AppResult<PathBuf> {
    // Use a stable scratch directory under the app data dir.
    // We pick the first data-dir candidate that is writable
    // (NEXUS_DATA_DIR, portable mode, or Tauri's default).
    // We do not auto-create the directory here because the
    // sandbox is opt-in: only the user (or the chat
    // orchestrator) that wants to run code should bring it
    // into existence.
    let data_dir = data_dir::candidates()
        .into_iter()
        .find(|c| c.path.exists() && data_dir::is_writable(&c.path))
        .map(|c| c.path)
        .or_else(|| dirs::data_dir().map(|d| d.join("ai.nexus.workbench")))
        .ok_or_else(|| nexus_core::error::AppError::Internal("no usable data directory for sandbox".into()))?;
    let scratch = data_dir.join("sandbox_scratch");
    if !scratch.exists() {
        std::fs::create_dir_all(&scratch).map_err(nexus_core::error::AppError::Io)?;
    }
    Ok(scratch)
}
