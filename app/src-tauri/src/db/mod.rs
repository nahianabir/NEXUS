//! SQLite pool, migration runner, and small query helpers.
//!
//! The pool is wrapped in `r2d2` for ergonomic sharing across the Tokio runtime;
//! we still open connections synchronously because `rusqlite` is blocking by
//! design. Long-running queries go through `tokio::task::spawn_blocking`.
//!
//! We auto-register the `sqlite-vec` extension so the `vec0` virtual table
//! works on every connection (we only need the *auto* registration once,
//! at the start of the process).

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Duration;

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::OpenFlags;

use crate::error::{AppError, AppResult};

pub mod migrations;
pub mod settings;

pub type SqlitePool = Pool<SqliteConnectionManager>;

static VEC_INIT: Once = Once::new();

/// Register the sqlite-vec auto extension. Idempotent.
///
/// IMPORTANT: the C signature is
///     int sqlite3_vec_init(sqlite3 *db, char **pzErrMsg, const sqlite3_api_routines *pApi)
/// but the `sqlite-vec` crate's Rust binding declares it as `pub fn sqlite3_vec_init()`
/// (no args, void return). Calling it directly is a type-mismatch undefined-behaviour
/// land mine — the function reads `pApi` from stack garbage and segfaults.
///
/// We must NOT call it. The `#[link(name = "sqlite_vec0")]` on the extern block
/// is what pulls the static lib into the binary; we just need a reference to
/// the symbol. We achieve that by going through the auto-extension API, which
/// takes the function pointer, casts it to the right C signature, and SQLite
/// itself calls it with the right args. That is the only safe entry point.
pub fn ensure_sqlite_vec() {
    VEC_INIT.call_once(|| unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    });
}


pub fn open_pool(db_path: &Path) -> AppResult<SqlitePool> {
    fn trace(msg: &str) {
        let dir = std::env::var("APPDATA")
            .map(|p| std::path::PathBuf::from(p).join("ai.nexus.workbench"))
            .unwrap_or_else(|_| std::env::temp_dir().join("ai.nexus.workbench"));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("boot.log");
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{} db: {}", chrono::Utc::now().to_rfc3339(), msg);
        }
    }
    trace("open_pool: enter");
    ensure_sqlite_vec();
    trace("open_pool: vec ensured");
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    trace("open_pool: parent created");
    let manager = SqliteConnectionManager::file(db_path)
        .with_flags(OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_URI)
        .with_init(|c| {
            c.busy_timeout(Duration::from_millis(5_000))?;
            c.pragma_update(None, "journal_mode", "WAL")?;
            c.pragma_update(None, "foreign_keys", "ON")?;
            c.pragma_update(None, "synchronous", "NORMAL")?;
            c.pragma_update(None, "temp_store", "MEMORY")?;
            Ok(())
        });
    trace("open_pool: manager built");
    let pool = Pool::builder()
        .max_size(8)
        .min_idle(Some(1))
        .build(manager)
        .map_err(|e| AppError::Internal(format!("sqlite pool: {e}")))?;
    trace("open_pool: pool built");
    Ok(pool)
}

pub fn default_db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("nexus.sqlite")
}