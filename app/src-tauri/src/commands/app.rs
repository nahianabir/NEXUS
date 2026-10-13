//! Top-level Tauri commands the frontend calls on launch.

use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::error::AppResult;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub data_dir: String,
    pub build_target: &'static str,
    pub schema_version: u32,
}

#[tauri::command]
pub fn app_info(state: State<'_, Arc<AppState>>) -> AppInfo {
    let conn = state
        .pool
        .get()
        .map_err(|e| nexus_core::error::AppError::Internal(format!("pool: {e}")))
        .ok();
    let schema_version = conn
        .and_then(|c| {
            c.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |r| r.get::<_, i64>(0),
            )
            .ok()
        })
        .unwrap_or(0) as u32;
    AppInfo {
        name: "NEXUS",
        version: env!("CARGO_PKG_VERSION"),
        data_dir: state.data_dir.display().to_string(),
        build_target: std::env::consts::OS,
        schema_version,
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct SidebarItem {
    pub id: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    pub group: &'static str,
    pub route: &'static str,
}

const SIDEBAR: &[SidebarItem] = &[
    SidebarItem { id: "command-center", label: "Command Center", icon: "compass",     group: "Overview",   route: "/" },
    SidebarItem { id: "assistant",      label: "AI Assistant",   icon: "spark",       group: "Work",       route: "/assistant" },
    SidebarItem { id: "knowledge",      label: "Knowledge Vault",icon: "vault",       group: "Work",       route: "/knowledge" },
    SidebarItem { id: "research",       label: "Research Studio",icon: "flask",       group: "Work",       route: "/research" },
    SidebarItem { id: "documents",      label: "Documents",      icon: "doc",         group: "Work",       route: "/documents" },
    SidebarItem { id: "projects",       label: "Projects",       icon: "grid",        group: "Work",       route: "/projects" },
    SidebarItem { id: "timeline",       label: "Time Machine",   icon: "history",     group: "Work",       route: "/timeline" },
    SidebarItem { id: "checkpoints",    label: "Work Resume",    icon: "resume",      group: "Work",       route: "/checkpoints" },
    SidebarItem { id: "radar",          label: "Opportunity Radar", icon: "radar",    group: "Work",       route: "/radar" },
    SidebarItem { id: "memory-center",  label: "Memory Center",  icon: "brain",       group: "Memory",     route: "/memory" },
    SidebarItem { id: "knowledge-graph",label: "Knowledge Graph",icon: "graph",       group: "Memory",     route: "/graph" },
    SidebarItem { id: "agent",          label: "Agent & Tools",  icon: "robot",       group: "Control",    route: "/agent" },
    SidebarItem { id: "experiments",    label: "Experiments",    icon: "chart",       group: "Research",   route: "/experiments" },
    SidebarItem { id: "tasks",          label: "Tasks & Calendar",icon:"calendar",    group: "Work",       route: "/tasks" },
    SidebarItem { id: "models",         label: "Model Manager",  icon: "chip",        group: "Control",    route: "/models" },
    SidebarItem { id: "orchestrator",   label: "Model Orchestrator", icon: "router", group: "Control",    route: "/orchestrator" },
    SidebarItem { id: "monitor",        label: "System Monitor", icon: "pulse",       group: "Control",    route: "/monitor" },
    SidebarItem { id: "settings",       label: "Settings",       icon: "gear",        group: "System",     route: "/settings" },
];

#[tauri::command]
pub fn list_sidebar() -> Vec<SidebarItem> {
    SIDEBAR.to_vec()
}

#[derive(Debug, Serialize)]
pub struct DashboardSummary {
    pub app: AppInfo,
    pub active_model: Option<String>,
    pub active_project: Option<nexus_core::projects::projects::Project>,
    pub queue_depth: i64,
    pub documents: i64,
    pub notes: i64,
    pub conversations: i64,
    pub tasks_open: i64,
    pub projects_total: i64,
    pub sample: nexus_core::monitor::SystemSample,
    pub sidebar: Vec<SidebarItem>,
}

#[tauri::command]
pub fn dashboard_summary(state: State<'_, Arc<AppState>>) -> AppResult<DashboardSummary> {
    // Reuse the long-lived monitor from `AppState` so CPU-time
    // deltas remain stable across requests. A fresh monitor would
    // reset the prime counter and the CPU value would be wrong on
    // every call.
    let sample = state.monitor.lock().sample();

    let conn = state
        .pool
        .get()
        .map_err(|e| nexus_core::error::AppError::Internal(format!("pool: {e}")))?;
    let count = |sql: &str| -> i64 {
        conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0)
    };
    let documents = count("SELECT COUNT(*) FROM documents");
    let notes = count("SELECT COUNT(*) FROM notes");
    let conversations = count("SELECT COUNT(*) FROM conversations");
    let tasks_open = count("SELECT COUNT(*) FROM tasks WHERE status IN ('open','in_progress')");
    let queue_depth = count("SELECT COUNT(*) FROM jobs WHERE status IN ('queued','running')");
    let projects_total = count("SELECT COUNT(*) FROM projects WHERE archived = 0");
    let active_model = state.active_model.read().clone();

    let active_project = match nexus_core::projects::ui_state::get::<String>(&state.pool, nexus_core::projects::ui_state::ACTIVE_PROJECT) {
        Ok(Some(id)) if !id.is_empty() => nexus_core::projects::projects::get(&state.pool, &id).ok(),
        _ => None,
    };

    Ok(DashboardSummary {
        app: app_info(state.clone()),
        active_model,
        active_project,
        queue_depth,
        documents,
        notes,
        conversations,
        tasks_open,
        projects_total,
        sample,
        sidebar: SIDEBAR.to_vec(),
    })
}

#[tauri::command]
pub fn open_main_window(app: tauri::AppHandle) -> AppResult<()> {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
        audit::log(
            &app.state::<Arc<AppState>>().pool,
            "user",
            "window.shown",
            Some("main"),
            serde_json::json!({}),
        )?;
    }
    let _ = app.emit("nexus:focus", serde_json::json!({"at": chrono::Utc::now().timestamp_millis()}));
    Ok(())
}

// ----------------------------- data dir ------------------------------
//
// The user can change the NEXUS data directory from the
// Settings page. The change is written to `config.toml`
// immediately; the actual move of files and the database
// happens during `app_data_dir_migrate`. The application must
// be restarted after a successful migration for the new
// directory to take effect (Tauri 2 cannot move a live
// database pool).

#[derive(Debug, Serialize)]
pub struct DataDirInfo {
    /// The directory the running process is using.
    pub current: String,
    /// Whether the running process is using the default
    /// per-user roaming location. When true, the user can
    /// change the directory from the Settings page.
    pub is_default: bool,
    /// The portable-mode hint. True if the running process
    /// is using `<exe_dir>/data`.
    pub is_portable: bool,
    /// Free space at the current directory, in bytes.
    /// Best-effort: Windows-specific, may return 0 on
    /// unsupported platforms.
    pub free_bytes: u64,
    /// Total space at the current directory, in bytes.
    pub total_bytes: u64,
}

#[tauri::command]
pub fn app_data_dir_info(state: State<'_, Arc<AppState>>) -> DataDirInfo {
    let current = state.data_dir.to_string_lossy().to_string();
    let is_portable = state
        .data_dir
        .file_name()
        .map(|n| n == "data")
        .unwrap_or(false);
    let is_default = !is_portable && std::env::var("NEXUS_DATA_DIR").is_err();
    let (free, total) = disk_space(&state.data_dir);
    DataDirInfo { current, is_default, is_portable, free_bytes: free, total_bytes: total }
}

#[derive(Debug, Serialize)]
pub struct DataDirValidation {
    pub ok: bool,
    pub reason: String,
}

#[tauri::command]
pub fn app_data_dir_validate(path: String) -> DataDirValidation {
    let p = std::path::PathBuf::from(&path);
    match nexus_core::data_dir::validate(&p) {
        Ok(()) => DataDirValidation { ok: true, reason: "OK".into() },
        Err(e) => DataDirValidation { ok: false, reason: e.to_string() },
    }
}

#[derive(Debug, Serialize)]
pub struct DataDirMigrationReport {
    pub files: u64,
    pub bytes: u64,
    pub source: String,
    pub destination: String,
}

#[tauri::command]
pub fn app_data_dir_migrate(
    state: State<'_, Arc<AppState>>,
    destination: String,
) -> AppResult<DataDirMigrationReport> {
    let src = state.data_dir.clone();
    let dst = std::path::PathBuf::from(&destination);
    if src == dst {
        return Err(nexus_core::error::AppError::Invalid(
            "destination is the same as the current data dir".into(),
        ));
    }
    nexus_core::data_dir::validate(&dst)?;
    // The source may be a SQLite WAL database that is open
    // and being written to. The migration does its own copy
    // without coordinating with the live pool; the caller
    // (frontend) is expected to:
    //   1. Show a confirmation dialog.
    //   2. Stop the application.
    //   3. Restart the application with the new
    //      `NEXUS_DATA_DIR` env var set.
    // The migration itself is best-effort. The user can
    // re-run it if the source files are still present.
    let (files, bytes) = nexus_core::data_dir::migrate(&src, &dst)?;
    // Update config to point at the new location.
    let mut cfg = state.config.write();
    cfg.general.data_dir = dst.clone();
    cfg.save(&dst)?;
    Ok(DataDirMigrationReport {
        files,
        bytes,
        source: src.to_string_lossy().to_string(),
        destination: dst.to_string_lossy().to_string(),
    })
}

#[cfg(target_os = "windows")]
fn disk_space(path: &std::path::Path) -> (u64, u64) {
    // Use sysinfo (already in our dependency tree) to look
    // up the disk that hosts `path` and return its free and
    // total bytes. sysinfo handles UNC paths, removable
    // drives, and missing directories.
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut best: Option<(u64, u64)> = None;
    for d in &disks {
        let mount = d.mount_point();
        if canon.starts_with(mount) {
            best = Some((d.available_space(), d.total_space()));
            break;
        }
    }
    best.unwrap_or((0, 0))
}

#[cfg(not(target_os = "windows"))]
fn disk_space(_path: &std::path::Path) -> (u64, u64) {
    (0, 0)
}

#[tauri::command]
pub fn app_data_dir_open(state: State<'_, Arc<AppState>>) -> AppResult<()> {
    let path = &state.data_dir;
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
    Ok(())
}