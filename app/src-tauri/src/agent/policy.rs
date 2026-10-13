//! Permission policy. Decides whether a tool invocation may proceed.

use crate::agent::registry;
use crate::agent::Permission;
use crate::db::SqlitePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow,
    RequireApproval,
    Deny,
}

pub struct PolicyOutcome {
    pub decision: PolicyDecision,
    pub reason: String,
}

pub fn evaluate(pool: &SqlitePool, tool_name: &str) -> AppResult<PolicyOutcome> {
    let tool = registry::get(pool, tool_name)?;
    if !tool.enabled {
        return Ok(PolicyOutcome { decision: PolicyDecision::Deny, reason: "tool is disabled".into() });
    }
    let perm = Permission::parse(&tool.permission)?;
    match perm {
        Permission::Always => Ok(PolicyOutcome { decision: PolicyDecision::Allow, reason: "permission: always".into() }),
        Permission::Ask => Ok(PolicyOutcome { decision: PolicyDecision::RequireApproval, reason: "permission: ask".into() }),
        Permission::Restricted => Ok(PolicyOutcome { decision: PolicyDecision::RequireApproval, reason: "permission: restricted (user must enable in Settings)".into() }),
        Permission::Deny => Ok(PolicyOutcome { decision: PolicyDecision::Deny, reason: "permission: deny".into() }),
    }
}

pub fn check_path_allowed(path: &str, allowlist: &[String]) -> Result<(), AppError> {
    let p = std::path::Path::new(path);
    let canonical = p.canonicalize().map_err(|e| AppError::Invalid(format!("canonicalize: {e}")))?;
    let canonical_str = canonical.to_string_lossy().to_string();
    for allowed in allowlist {
        if let Ok(allowed_canon) = std::path::Path::new(allowed).canonicalize() {
            if canonical_str.starts_with(&allowed_canon.to_string_lossy().to_string()) {
                return Ok(());
            }
        } else if canonical_str.starts_with(allowed) {
            return Ok(());
        }
    }
    Err(AppError::Permission(format!("path {path} is not in the file allowlist")))
}

pub fn check_command_allowed(cmd: &str, allowlist: &[String]) -> Result<(), AppError> {
    if allowlist.iter().any(|a| a == cmd) {
        Ok(())
    } else {
        Err(AppError::Permission(format!("command {cmd} is not in the shell allowlist")))
    }
}
