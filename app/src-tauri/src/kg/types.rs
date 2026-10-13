//! Graph types.

use serde::{Deserialize, Serialize};

/// The supported entity types. The list is intentionally small and
/// open; new types are accepted as long as the call site spells them
/// out. Documentation in `docs/KNOWLEDGE_GRAPH.md` covers the
/// canonical set.
pub const ENTITY_TYPES: &[&str] = &[
    "person", "project", "paper", "dataset", "model", "experiment",
    "metric", "repository", "file", "document", "note", "conversation",
    "decision", "idea", "task", "organization", "technology", "concept",
    "memory",
];

pub const RELATIONSHIP_TYPES: &[&str] = &[
    "BELONGS_TO", "REFERENCES", "AUTHORED_BY", "USES_DATASET", "USES_MODEL",
    "EVALUATED_WITH", "PRODUCED_RESULT", "DECIDED_IN", "SUPERSEDES",
    "DEPENDS_ON", "RELATED_TO", "CONTRADICTS", "SUPPORTS", "IMPLEMENTED_IN",
    "DERIVED_FROM", "MENTIONED_IN", "CONTINUES", "BLOCKED_BY",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub workspace_id: Option<String>,
    pub entity_type: String,
    pub canonical_name: String,
    pub description: Option<String>,
    pub metadata_json: Option<String>,
    pub merged_into: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub source_entity_id: String,
    pub relationship_type: String,
    pub target_entity_id: String,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
    pub created_at: i64,
    pub verification_status: String,
    pub weight: f64,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub id: String,
    pub relationship_id: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_location: Option<String>,
    pub excerpt: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Neighborhood {
    pub center: Entity,
    pub nodes: Vec<Entity>,
    pub edges: Vec<Relationship>,
    pub evidence_by_rel: Vec<Vec<Evidence>>,
}
