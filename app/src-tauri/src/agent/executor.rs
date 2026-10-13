//! Tool executor. The actual implementations live in a trait
//! implementation so they can be unit-tested without Tauri.
//!
//! For Phase I we ship implementations for the read-only /
//! inspection tools (vault.search, kg.search, memory.recall,
//! research.report.build, project.note.add, project.task.add,
//! file.list, git.status, settings.get). "ask" tools return
//! pending; the UI must approve them before they execute.

use serde::{Deserialize, Serialize};

use crate::agent::registry;
use crate::agent::invocations;
use crate::agent::policy;
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};
use crate::ingestion;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub ok: bool,
    pub summary: String,
    pub data: serde_json::Value,
}

pub fn run_tool(
    pool: &SqlitePool,
    invocation_id: &str,
    tool_name: &str,
    input: &serde_json::Value,
    file_allowlist: &[String],
    shell_allowlist: &[String],
) -> AppResult<serde_json::Value> {
    let tool = registry::get(pool, tool_name)?;
    if !tool.enabled {
        return Err(AppError::Permission(format!("tool {tool_name} is disabled")));
    }
    let decision = policy::evaluate(pool, tool_name)?;
    if matches!(decision.decision, policy::PolicyDecision::Deny) {
        return Err(AppError::Permission(decision.reason));
    }
    if matches!(decision.decision, policy::PolicyDecision::RequireApproval) {
        // Stay pending; the UI will set status=approved and call run_tool again.
        invocations::update_status(pool, invocation_id, "pending", None, None, false)?;
        return Ok(serde_json::json!({ "status": "pending_approval", "reason": decision.reason }));
    }
    // Permission::Always — execute.
    let result = dispatch(pool, tool_name, input, file_allowlist, shell_allowlist)?;
    invocations::update_status(pool, invocation_id, "executed", Some(&serde_json::to_value(&result).unwrap_or(serde_json::json!({}))), None, true)?;
    Ok(serde_json::to_value(&result).unwrap_or(serde_json::json!({})))
}

pub fn approve(pool: &SqlitePool, invocation_id: &str) -> AppResult<()> {
    let inv = invocations::get(pool, invocation_id)?;
    if inv.status != "pending" && inv.status != "approved" {
        return Err(AppError::Invalid(format!("cannot approve invocation in status {}", inv.status)));
    }
    invocations::update_status(pool, invocation_id, "approved", None, None, false)?;
    Ok(())
}

pub fn deny(pool: &SqlitePool, invocation_id: &str) -> AppResult<()> {
    invocations::update_status(pool, invocation_id, "denied", None, Some("user denied"), true)?;
    Ok(())
}

fn dispatch(
    pool: &SqlitePool,
    tool_name: &str,
    input: &serde_json::Value,
    file_allowlist: &[String],
    shell_allowlist: &[String],
) -> AppResult<ToolOutput> {
    match tool_name {
        "vault.search" => {
            let query = input.get("query").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("query required".into()))?;
            let limit: usize = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let res = ingestion::search::search_fts(pool, query, limit).unwrap_or_default();
            Ok(ToolOutput { ok: true, summary: format!("{} chunks", res.len()), data: serde_json::to_value(&res).unwrap_or(serde_json::json!([])) })
        }
        "kg.search" => {
            let query = input.get("query").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("query required".into()))?;
            let limit: usize = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let res = crate::kg::queries::search(pool, query, limit).unwrap_or_default();
            Ok(ToolOutput { ok: true, summary: format!("{} entities", res.len()), data: serde_json::to_value(&res).unwrap_or(serde_json::json!([])) })
        }
        "memory.recall" => {
            let query = input.get("query").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("query required".into()))?;
            let limit: usize = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
            let res = crate::memory_v2::search::search(pool, query, None, None, limit).unwrap_or_default();
            Ok(ToolOutput { ok: true, summary: format!("{} memories", res.len()), data: serde_json::to_value(&res).unwrap_or(serde_json::json!([])) })
        }
        "research.report.build" => {
            let project_id = input.get("project_id").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("project_id required".into()))?;
            let body = crate::research::markdown::build_project_report(pool, project_id)?;
            Ok(ToolOutput { ok: true, summary: format!("report: {} chars", body.len()), data: serde_json::json!({ "body_md": body }) })
        }
        "project.note.add" => {
            let title = input.get("title").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("title required".into()))?;
            let body = input.get("body").and_then(|v| v.as_str()).unwrap_or("");
            let project_id = input.get("project_id").and_then(|v| v.as_str());
            let n = crate::projects::notes::create(pool, project_id, title, body)?;
            Ok(ToolOutput { ok: true, summary: format!("note: {}", title), data: serde_json::json!({ "id": n.id }) })
        }
        "project.task.add" => {
            let title = input.get("title").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("title required".into()))?;
            let due_at = input.get("due_at").and_then(|v| v.as_i64());
            let project_id = input.get("project_id").and_then(|v| v.as_str());
            let t = crate::projects::tasks::create(
                pool,
                project_id,
                title,
                None,
                crate::projects::tasks::TaskPriority::Normal,
                due_at,
                &[],
            )?;
            Ok(ToolOutput { ok: true, summary: format!("task: {}", title), data: serde_json::json!({ "id": t.id }) })
        }
        "file.list" => {
            let path = input.get("path").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("path required".into()))?;
            let max_entries = input.get("max_entries").and_then(|v| v.as_i64()).unwrap_or(500) as usize;
            policy::check_path_allowed(path, file_allowlist)?;
            let mut entries: Vec<serde_json::Value> = Vec::new();
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let meta = entry.metadata().ok();
                entries.push(serde_json::json!({
                    "name": entry.file_name().to_string_lossy(),
                    "is_dir": meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                    "is_file": meta.as_ref().map(|m| m.is_file()).unwrap_or(false),
                    "size": meta.as_ref().map(|m| m.len()).unwrap_or(0),
                }));
                if entries.len() >= max_entries { break; }
            }
            Ok(ToolOutput { ok: true, summary: format!("{} entries", entries.len()), data: serde_json::json!({ "entries": entries }) })
        }
        "file.read" => {
            let path = input.get("path").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("path required".into()))?;
            let max_bytes = input.get("max_bytes").and_then(|v| v.as_u64()).unwrap_or(65536);
            policy::check_path_allowed(path, file_allowlist)?;
            let bytes = std::fs::read(path)?;
            let truncated = bytes.len() as u64 > max_bytes;
            let text = String::from_utf8_lossy(&bytes[..bytes.len().min(max_bytes as usize)]).to_string();
            Ok(ToolOutput { ok: true, summary: format!("{} bytes{}", text.len(), if truncated { "(truncated)" } else { "" }), data: serde_json::json!({ "text": text, "truncated": truncated }) })
        }
        "shell.run" => {
            let cmd = input.get("command").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("command required".into()))?;
            let args: Vec<String> = input.get("args").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|s| s.as_str().map(|s| s.to_string())).collect()).unwrap_or_default();
            let _timeout_ms = input.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(15000);
            policy::check_command_allowed(cmd, shell_allowlist)?;
            let output = std::process::Command::new(cmd).args(&args).output()?;
            let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if stdout.len() > 65536 { stdout.truncate(65536); stdout.push_str("\n... (output truncated)"); }
            if stderr.len() > 65536 { stderr.truncate(65536); stderr.push_str("\n... (output truncated)"); }
            // The `_timeout_ms` is best-effort on a synchronous process. We do
            // not block; we trust the user to allowlist only fast commands.
            Ok(ToolOutput { ok: output.status.success(), summary: format!("exit {:?}", output.status.code()), data: serde_json::json!({ "stdout": stdout, "stderr": stderr, "exit": output.status.code() }) })
        }
        "git.status" => {
            let cwd = input.get("cwd").and_then(|v| v.as_str());
            let mut cmd = std::process::Command::new("git");
            cmd.arg("status").arg("--porcelain");
            if let Some(c) = cwd { cmd.current_dir(c); }
            let output = cmd.output()?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Ok(ToolOutput { ok: output.status.success(), summary: format!("{} lines", stdout.lines().count()), data: serde_json::json!({ "stdout": stdout, "stderr": stderr, "exit": output.status.code() }) })
        }
        "settings.get" => {
            let key = input.get("key").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("key required".into()))?;
            let v: Option<String> = crate::db::settings::get(pool, key).unwrap_or(None);
            Ok(ToolOutput { ok: true, summary: format!("{}: {}", key, v.clone().unwrap_or_else(|| "(unset)".into())), data: serde_json::json!({ "key": key, "value": v }) })
        }
        "settings.set" => {
            let key = input.get("key").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("key required".into()))?;
            let value = input.get("value").and_then(|v| v.as_str()).ok_or_else(|| AppError::Invalid("value required".into()))?;
            crate::db::settings::set(pool, key, &value.to_string())?;
            Ok(ToolOutput { ok: true, summary: format!("{} set", key), data: serde_json::json!({ "key": key }) })
        }
        other => Err(AppError::Invalid(format!("unknown tool: {other}"))),
    }
}