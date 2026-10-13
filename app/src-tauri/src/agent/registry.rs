//! Tool registry CRUD.

use rusqlite::params;

use super::{now_ms, Tool};
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn row_to_tool(r: &rusqlite::Row<'_>) -> rusqlite::Result<Tool> {
    let schema_json: String = r.get(6)?;
    let schema: serde_json::Value = serde_json::from_str(&schema_json).unwrap_or(serde_json::json!({}));
    Ok(Tool {
        id: r.get(0)?,
        name: r.get(1)?,
        description: r.get(2)?,
        category: r.get(3)?,
        permission: r.get(4)?,
        enabled: r.get::<_, i64>(5)? != 0,
        schema,
        created_at: r.get(7)?,
    })
}

pub fn list(pool: &SqlitePool, category: Option<&str>, include_disabled: bool) -> AppResult<Vec<Tool>> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let mut sql = String::from("SELECT id, name, description, category, permission, enabled, schema_json, created_at FROM agent_tools WHERE 1=1");
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(c) = category { sql.push_str(" AND category = ?"); args.push(Box::new(c.to_string())); }
    if !include_disabled { sql.push_str(" AND enabled = 1"); }
    sql.push_str(" ORDER BY category, name");
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref() as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(rusqlite::params_from_iter(refs), row_to_tool)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn get(pool: &SqlitePool, name: &str) -> AppResult<Tool> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.query_row(
        "SELECT id, name, description, category, permission, enabled, schema_json, created_at FROM agent_tools WHERE name = ?1",
        params![name],
        row_to_tool,
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::NotFound(format!("tool {name}")),
        other => AppError::Db(other),
    })
}

pub fn set_enabled(pool: &SqlitePool, name: &str, enabled: bool) -> AppResult<Tool> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    conn.execute(
        "UPDATE agent_tools SET enabled = ?1 WHERE name = ?2",
        params![enabled as i64, name],
    )?;
    get(pool, name)
}

pub fn upsert(pool: &SqlitePool, t: &Tool) -> AppResult<Tool> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let schema_str = serde_json::to_string(&t.schema).unwrap_or_else(|_| "{}".into());
    let now = now_ms();
    let id = if t.id.is_empty() { t.name.clone() } else { t.id.clone() };
    conn.execute(
        "INSERT INTO agent_tools (id, name, description, category, permission, enabled, schema_json, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(name) DO UPDATE SET
           description=excluded.description,
           category=excluded.category,
           permission=excluded.permission,
           enabled=excluded.enabled,
           schema_json=excluded.schema_json",
        params![id, t.name, t.description, t.category, t.permission, t.enabled as i64, schema_str, now],
    )?;
    get(pool, &t.name)
}
