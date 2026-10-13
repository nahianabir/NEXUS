//! Helpers for building the markdown blocks that get prepended to the
//! system prompt at chat time. Kept in the rlib (not the Tauri command
//! module) so unit tests can exercise them.

use crate::db::SqlitePool;
use crate::error::AppResult;
use crate::memory;
use crate::projects::{notes, projects};

/// Build the markdown block that prepends a project's instructions, goals,
/// and the three most recently-updated notes to the system prompt. The
/// model is told to honour project instructions as if they were explicit
/// user preferences. Returns an empty string if the project has no content.
pub fn build_project_context_block(pool: &SqlitePool, project_id: &str) -> AppResult<String> {
    let p = match projects::get(pool, project_id) {
        Ok(p) => p,
        Err(crate::error::AppError::NotFound(_)) => return Ok(String::new()),
        Err(e) => return Err(e),
    };
    let mut s = String::new();
    s.push_str(&format!("\n\n## Project: {}\n", p.name));
    if let Some(d) = p.description.as_deref().filter(|x| !x.trim().is_empty()) {
        s.push_str(&format!("{}\n", d.trim()));
    }
    if let Some(ins) = p.instructions.as_deref().filter(|x| !x.trim().is_empty()) {
        s.push_str(&format!("\n### Instructions\nTreat the following as firm project rules. The user has asked you to honour them for the duration of this project.\n\n{}\n", ins.trim()));
    }
    if !p.goals.is_empty() {
        s.push_str("\n### Goals\n");
        for g in &p.goals {
            s.push_str(&format!("- {}\n", g));
        }
    }
    let recent_notes = notes::list(pool, Some(project_id))?;
    if !recent_notes.is_empty() {
        let take = recent_notes.iter().take(3);
        s.push_str("\n### Recent notes (top 3 by recency)\n");
        for n in take {
            let body = if n.body.len() > 600 {
                format!("{}…", &n.body[..600])
            } else {
                n.body.clone()
            };
            s.push_str(&format!("- **{}** — {}\n", n.title, body.replace('\n', " ")));
        }
    }
    Ok(s)
}

/// Build the markdown block that surfaces the approved memories the model
/// should hold for this conversation (user-level, plus the conversation's
/// project, plus this conversation's ephemeral entries).
pub fn build_memory_context_block(
    pool: &SqlitePool,
    project_id: Option<&str>,
    conversation_id: Option<&str>,
) -> AppResult<String> {
    let mems = memory::list_for_context(pool, project_id, conversation_id)?;
    if mems.is_empty() { return Ok(String::new()); }
    let mut s = String::from("\n\n## Persistent memory\n");
    s.push_str("The following are user-approved facts. Honour them unless the user explicitly overrides them.\n");
    for m in &mems {
        let scope = match m.scope.as_str() {
            "user" => "user",
            "project" => "project",
            "conv" => "this conversation",
            _ => "memory",
        };
        s.push_str(&format!("- ({}): {}\n", scope, m.content));
    }
    Ok(s)
}
