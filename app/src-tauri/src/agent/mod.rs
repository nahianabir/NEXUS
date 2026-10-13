//! Controlled Agent. Phase I.
//!
//! The agent is a small planner/executor with a permission-gated
//! tool registry. Every invocation is recorded; nothing happens
//! silently. The user reviews the run in the Agent page; "ask"
//! tools require explicit approval before they execute.
//!
//! Tool categories:
//!   * knowledge — vault.search, kg.search, memory.recall
//!   * research  — research.claim.list, research.report.build
//!   * projects  — project.note.add, project.task.add
//!   * files     — file.read (allowlisted paths), file.list
//!   * dev       — shell.run (allowlisted commands), git.status
//!   * system    — settings.get, settings.set
//!
//! Permission levels:
//!   * always      — runs without prompting (read-only, safe)
//!   * ask         — runs after the user clicks "approve" in the UI
//!   * restricted  — disabled unless the user has explicitly enabled
//!                    the underlying feature in Settings
//!   * deny        — never runs; the registry entry exists for
//!                    documentation only

pub mod registry;
pub mod runs;
pub mod invocations;
pub mod policy;
pub mod executor;

use rusqlite::params;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

pub fn new_id() -> String { Ulid::new().to_string() }

pub fn now_ms() -> i64 { chrono::Utc::now().timestamp_millis() }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission { Always, Ask, Restricted, Deny }

impl Permission {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Ask => "ask",
            Self::Restricted => "restricted",
            Self::Deny => "deny",
        }
    }
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "always" => Ok(Self::Always),
            "ask" => Ok(Self::Ask),
            "restricted" => Ok(Self::Restricted),
            "deny" => Ok(Self::Deny),
            other => Err(AppError::Invalid(format!("permission: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub permission: String,
    pub enabled: bool,
    pub schema: serde_json::Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: String,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub title: String,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invocation {
    pub id: String,
    pub run_id: String,
    pub tool_name: String,
    pub permission: String,
    pub input: serde_json::Value,
    pub output: Option<serde_json::Value>,
    pub status: String,
    pub error: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

pub fn ensure_seed_tools(pool: &SqlitePool) -> AppResult<()> {
    let conn = pool.get().map_err(|e| AppError::Internal(format!("pool: {e}")))?;
    let now = now_ms();
    let seeds: &[(&str, &str, &str, &str, &str)] = &[
        ("knowledge", "vault.search",          "Search the local knowledge vault",                                          "always",     r#"{"type":"object","required":["query"],"properties":{"query":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":50}}}"#),
        ("knowledge", "kg.search",             "Search the knowledge graph for entities",                                  "always",     r#"{"type":"object","required":["query"],"properties":{"query":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":50}}}"#),
        ("knowledge", "memory.recall",         "Recall long-term memories matching a query",                               "always",     r#"{"type":"object","required":["query"],"properties":{"query":{"type":"string"},"limit":{"type":"integer","minimum":1,"maximum":50}}}"#),
        ("research",  "research.report.build", "Build a Markdown snapshot of a research project",                          "ask",        r#"{"type":"object","required":["project_id"],"properties":{"project_id":{"type":"string"}}}"#),
        ("projects",  "project.note.add",      "Append a note to the active project",                                       "ask",        r#"{"type":"object","required":["title","body"],"properties":{"title":{"type":"string"},"body":{"type":"string"},"project_id":{"type":"string"}}}"#),
        ("projects",  "project.task.add",      "Create a task in the active project",                                      "ask",        r#"{"type":"object","required":["title"],"properties":{"title":{"type":"string"},"due_at":{"type":"integer"},"project_id":{"type":"string"}}}"#),
        ("files",     "file.read",             "Read a file from an allowlisted directory",                                "ask",        r#"{"type":"object","required":["path"],"properties":{"path":{"type":"string"},"max_bytes":{"type":"integer","minimum":1,"maximum":1048576}}}"#),
        ("files",     "file.list",             "List files under an allowlisted directory",                                "always",     r#"{"type":"object","required":["path"],"properties":{"path":{"type":"string"},"max_entries":{"type":"integer","minimum":1,"maximum":1000}}}"#),
        ("dev",       "shell.run",             "Run a shell command from the allowlist (with timeout and output cap)",     "restricted", r#"{"type":"object","required":["command"],"properties":{"command":{"type":"string"},"args":{"type":"array","items":{"type":"string"}},"timeout_ms":{"type":"integer","minimum":100,"maximum":120000}}}"#),
        ("dev",       "git.status",            "Run `git status --porcelain` inside the working directory",               "always",     r#"{"type":"object","properties":{"cwd":{"type":"string"}}}"#),
        ("system",    "settings.get",          "Read a single app setting",                                                "always",     r#"{"type":"object","required":["key"],"properties":{"key":{"type":"string"}}}"#),
        ("system",    "settings.set",          "Write a single app setting",                                               "ask",        r#"{"type":"object","required":["key","value"],"properties":{"key":{"type":"string"},"value":{}}}"#),
    ];
    for (cat, name, desc, perm, schema) in seeds {
        let schema_v: serde_json::Value = serde_json::from_str(schema).unwrap_or(serde_json::json!({}));
        let schema_str = serde_json::to_string(&schema_v).unwrap_or_else(|_| "{}".into());
        let id = format!("seed-{name}");
        conn.execute(
            "INSERT OR IGNORE INTO agent_tools (id, name, description, category, permission, enabled, schema_json, created_at) VALUES (?1,?2,?3,?4,?5,1,?6,?7)",
            params![id, name, desc, cat, perm, schema_str, now],
        )?;
    }
    Ok(())
}
