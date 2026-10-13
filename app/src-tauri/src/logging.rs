//! Application-wide logging. Two sinks:
//!   * stderr (and the Tauri dev console) via `env_logger` for `RUST_LOG`
//!   * rotating file under `<data_dir>/logs/nexus.log` via `fern`
//!
//! The logger is idempotent — repeated calls are no-ops.

use std::fs;
use std::sync::Once;

use crate::error::{AppError, AppResult};

static INIT: Once = Once::new();

pub fn init() {
    INIT.call_once(|| {
        // Default log filter if RUST_LOG is not set.
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,nexus_core=debug,tauri=info"));

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_level(true)
            .init();

        // Mirror to the env_logger family because Tauri plugins still emit `log!` macros.
        let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
            .try_init();
    });
}

pub fn ensure_log_dir(data_dir: &std::path::Path) -> AppResult<()> {
    let dir = data_dir.join("logs");
    fs::create_dir_all(&dir).map_err(AppError::Io)?;
    Ok(())
}
