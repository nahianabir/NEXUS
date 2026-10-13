//! Public types for the context builder.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextSource {
    ProjectContext,
    NativeMemory,
    MemPalaceMemory,
    VaultDocument,
    GraphRelationship,
    GraphEntity,
    ManualInstruction,
}

impl ContextSource {
    pub fn label(&self) -> &'static str {
        match self {
            Self::ProjectContext => "project",
            Self::NativeMemory => "memory",
            Self::MemPalaceMemory => "mempalace",
            Self::VaultDocument => "vault",
            Self::GraphRelationship => "graph-edge",
            Self::GraphEntity => "graph-node",
            Self::ManualInstruction => "instruction",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    pub source: ContextSource,
    pub source_id: String,
    pub source_label: Option<String>,
    pub title: Option<String>,
    pub excerpt: String,
    /// Approximate token count. Used to enforce the budget.
    pub approx_tokens: i64,
    /// Free-form provenance: page number, memory id, relationship type, etc.
    pub provenance: Option<String>,
    /// Verification state where relevant (memory verification, edge status).
    pub verification: Option<String>,
    /// Higher = more relevant. 0..=100.
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBundle {
    pub items: Vec<ContextItem>,
    pub approx_tokens: i64,
    pub truncated: bool,
    pub system_block: String,
    /// The sources that were searched, regardless of whether they
    /// produced hits. Used by the activity inspector.
    pub sources_searched: Vec<ContextSource>,
    /// Per-source hit counts.
    pub hits_by_source: std::collections::BTreeMap<String, i64>,
}
