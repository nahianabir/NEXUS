//! Resolving the NEXUS data directory.
//!
//! The location of the user's data is determined in this
//! order, lowest-precedence first:
//!
//!   1. **Default**: Tauri 2's `app.path().app_data_dir()`,
//!      which on Windows resolves to
//!      `C:\Users\<user>\AppData\Roaming\ai.nexus.workbench`.
//!   2. **Per-user config**: `<data_dir>/config.toml`'s
//!      `[general] data_dir = "..."`. The first launch writes
//!      the default into the config; the user can change it
//!      from the Settings page. Changing this value requires
//!      a restart.
//!   3. **Environment variable**: `NEXUS_DATA_DIR`. The
//!      installer or a launcher script can set this to point
//!      at a different drive without touching the config.
//!      Useful for portable installations.
//!
//! The first launch always lands in the default location. The
//! `navigate` function lets the user move the data directory
//! later; the source directory is preserved until the
//! destination is verified.
//!
//! Portable mode: if the executable's adjacent `data/`
//! directory is writable, NEXUS uses it instead of the
//! per-user roaming path. This makes the application behave
//! like a portable install when the user copies the whole
//! `NEXUS/` directory onto a thumb drive.

use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

/// Returns the *primary* candidate for the data directory,
/// in order of decreasing precedence. The caller picks the
/// first one that exists and is writable.
///
/// Order:
/// 1. `NEXUS_DATA_DIR` env var (if set and non-empty).
/// 2. `<exe_dir>/data` if writable (portable mode).
/// 3. Tauri's `app_data_dir()` (the default).
pub fn candidates() -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Ok(s) = std::env::var("NEXUS_DATA_DIR") {
        if !s.trim().is_empty() {
            out.push(Candidate { path: PathBuf::from(s.trim()), origin: Origin::Env });
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let p = parent.join("data");
            if p.exists() && is_writable(&p) {
                out.push(Candidate { path: p, origin: Origin::Portable });
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
pub enum Origin {
    Env,
    Portable,
    Default,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub path: PathBuf,
    pub origin: Origin,
}

/// Returns true if `path` is writable by the current user.
/// Creates a tiny test file in the directory.
pub fn is_writable(path: &Path) -> bool {
    let probe = path.join(".nexus_write_probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Validate that `path` is a usable data directory. Used
/// after the user picks a new location in Settings.
pub fn validate(path: &Path) -> AppResult<()> {
    if path.as_os_str().is_empty() {
        return Err(AppError::Invalid("data directory path is empty".into()));
    }
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            return Err(AppError::Invalid(format!(
                "parent directory does not exist: {}",
                parent.display()
            )));
        }
    }
    std::fs::create_dir_all(path).map_err(|e| {
        AppError::Invalid(format!("cannot create data dir {}: {e}", path.display()))
    })?;
    if !is_writable(path) {
        return Err(AppError::Invalid(format!(
            "data dir is not writable: {}",
            path.display()
        )));
    }
    Ok(())
}

/// Migrate data from one directory to another. The source
/// directory is *not* deleted — it is preserved until the
/// caller confirms the destination is healthy.
///
/// Returns the number of files copied and the total bytes.
pub fn migrate(src: &Path, dst: &Path) -> AppResult<(u64, u64)> {
    if !src.exists() {
        return Err(AppError::Invalid(format!(
            "source data dir does not exist: {}",
            src.display()
        )));
    }
    validate(dst)?;
    let mut files: u64 = 0;
    let mut bytes: u64 = 0;
    for entry in walk_dir(src)? {
        let rel = entry.strip_prefix(src).unwrap_or(&entry);
        let target = dst.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // SQLite WAL files may be in use; the caller is
            // expected to stop the database before invoking
            // this. We copy with a best-effort strategy and
            // surface failures so the caller can retry.
            std::fs::copy(&entry, &target).map_err(|e| {
                AppError::Internal(format!("copy {} -> {}: {e}", entry.display(), target.display()))
            })?;
            files += 1;
            bytes += std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok((files, bytes))
}

fn walk_dir(root: &Path) -> AppResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        let entries = std::fs::read_dir(&p)?;
        for e in entries {
            let e = e?;
            let path = e.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.push(root.to_path_buf());
    Ok(out)
}
