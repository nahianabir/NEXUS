//! Helpers to seed the graph from existing records without requiring
//! the model. We do not invent relationships; we only materialise
//! well-known ones (a project is an entity; a memory's `project_id`
//! creates a `BELONGS_TO` edge from the memory-as-entity to the
//! project). Phase 6 adds model-assisted extraction.

use crate::db::SqlitePool;
use crate::error::AppResult;
use super::evidence::CreateEvidence;
use super::graph::{self, CreateEntity};
use super::relationship::{self, CreateRelationship};
use super::types::Entity;

pub fn seed_project(pool: &SqlitePool, project_id: &str) -> AppResult<Entity> {
    let project = crate::projects::projects::get(pool, project_id)?;
    let e = graph::create(pool, &CreateEntity {
        workspace_id: project.workspace_id,
        entity_type: "project".into(),
        canonical_name: project.name.clone(),
        description: project.description.clone(),
        metadata_json: Some(serde_json::json!({ "project_id": project.id }).to_string()),
        aliases: vec![],
    })?;
    Ok(e)
}

/// Add a `BELONGS_TO` edge from `child` to `parent`, attaching the
/// given evidence. If either side is missing, the call returns the
/// canonical id of the existing or newly-created child.
pub fn add_belongs_to(
    pool: &SqlitePool,
    child_type: &str,
    child_name: &str,
    parent_id: &str,
    evidence: CreateEvidence,
) -> AppResult<Entity> {
    let child = graph::create(pool, &CreateEntity {
        workspace_id: None,
        entity_type: child_type.into(),
        canonical_name: child_name.into(),
        description: None,
        metadata_json: None,
        aliases: vec![],
    })?;
    relationship::create(pool, &CreateRelationship {
        source_entity_id: child.id.clone(),
        relationship_type: "BELONGS_TO".into(),
        target_entity_id: parent_id.into(),
        verification_status: Some("inferred".into()),
        evidence: vec![evidence],
        ..Default::default()
    })?;
    Ok(child)
}
