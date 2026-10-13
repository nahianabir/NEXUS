//! Tauri commands for the audit log.

use std::sync::Arc;

use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::audit::{self, AuditEvent};
use nexus_core::error::AppResult;

#[tauri::command]
pub fn audit_recent(state: State<'_, Arc<AppState>>, limit: Option<usize>) -> AppResult<Vec<AuditEvent>> {
    audit::recent(&state.pool, limit.unwrap_or(200))
}

#[tauri::command]
pub fn audit_clear(state: State<'_, Arc<AppState>>) -> AppResult<usize> {
    let n = audit::clear(&state.pool)?;
    Ok(n)
}

#[tauri::command]
pub fn audit_prune(state: State<'_, Arc<AppState>>, before_ms: i64) -> AppResult<usize> {
    let n = audit::prune_before(&state.pool, before_ms)?;
    Ok(n)
}

#[tauri::command]
pub fn audit_prune_by_retention(state: State<'_, Arc<AppState>>, retention_days: u32) -> AppResult<usize> {
    let n = audit::prune_by_retention(&state.pool, retention_days)?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::audit;
    use nexus_core::db;

    fn temp_pool() -> nexus_core::db::SqlitePool {
        let base = std::env::temp_dir().join(format!("nexus-audit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&base).unwrap();
        let pool = db::open_pool(&db::default_db_path(&base)).expect("pool");
        let conn = pool.get().unwrap();
        let mut conn = conn;
        db::migrations::run(&mut conn).expect("migrate");
        pool
    }

    #[test]
    fn clear_removes_all_events() {
        let pool = temp_pool();
        for i in 0..3 {
            audit::log(&pool, "user", "test.event", Some(&format!("t{i}")), serde_json::json!({ "i": i })).unwrap();
        }
        assert_eq!(audit::recent(&pool, 10).unwrap().len(), 3);
        assert_eq!(audit::clear(&pool).unwrap(), 3);
        assert_eq!(audit::recent(&pool, 10).unwrap().len(), 0);
    }

    #[test]
    fn prune_before_removes_only_old() {
        let pool = temp_pool();
        // Insert one "ancient" and one "recent" event by hand.
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO audit_events (id, actor, action, target, detail, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["a1", "user", "old", "t", "{}", 1_i64],
        ).unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO audit_events (id, actor, action, target, detail, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params!["a2", "user", "new", "t", "{}", now],
        ).unwrap();
        let removed = audit::prune_before(&pool, now - 1000).unwrap();
        assert_eq!(removed, 1);
        let left = audit::recent(&pool, 10).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].action, "new");
    }

    #[test]
    fn prune_by_retention_zero_is_noop() {
        let pool = temp_pool();
        audit::log(&pool, "user", "x", None, serde_json::json!({})).unwrap();
        let n = audit::prune_by_retention(&pool, 0).unwrap();
        assert_eq!(n, 0);
        assert_eq!(audit::recent(&pool, 10).unwrap().len(), 1);
    }
}
