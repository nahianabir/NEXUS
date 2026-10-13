//! Tauri commands for the knowledge graph. Phase D.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use nexus_core::app_state::AppState;
use nexus_core::audit;
use nexus_core::error::{AppError, AppResult};
use nexus_core::kg;

#[tauri::command]
pub fn kg_entity_types() -> Vec<String> {
    kg::ENTITY_TYPES.iter().map(|s| s.to_string()).collect()
}

#[tauri::command]
pub fn kg_relationship_types() -> Vec<String> {
    kg::RELATIONSHIP_TYPES.iter().map(|s| s.to_string()).collect()
}

#[tauri::command]
pub fn kg_list_entities(
    state: State<'_, Arc<AppState>>,
    entity_type: Option<String>,
    workspace_id: Option<String>,
    search: Option<String>,
    limit: Option<i64>,
    include_merged: Option<bool>,
) -> AppResult<Vec<kg::Entity>> {
    let filter = kg::ListFilter { entity_type, workspace_id, search, limit, include_merged: include_merged.unwrap_or(false) };
    kg::graph::list(&state.pool, &filter)
}

#[tauri::command]
pub fn kg_search(state: State<'_, Arc<AppState>>, query: String, limit: Option<usize>) -> AppResult<Vec<kg::Entity>> {
    kg::queries::search(&state.pool, &query, limit.unwrap_or(50))
}

#[tauri::command]
pub fn kg_get_entity(state: State<'_, Arc<AppState>>, id: String) -> AppResult<kg::Entity> {
    kg::graph::get(&state.pool, &id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEntityRequest {
    pub workspace_id: Option<String>,
    pub entity_type: String,
    pub canonical_name: String,
    pub description: Option<String>,
    pub metadata_json: Option<String>,
    pub aliases: Option<Vec<String>>,
}

#[tauri::command]
pub fn kg_create_entity(state: State<'_, Arc<AppState>>, request: CreateEntityRequest) -> AppResult<kg::Entity> {
    let req = kg::CreateEntity {
        workspace_id: request.workspace_id,
        entity_type: request.entity_type,
        canonical_name: request.canonical_name,
        description: request.description,
        metadata_json: request.metadata_json,
        aliases: request.aliases.unwrap_or_default(),
    };
    let e = kg::graph::create(&state.pool, &req)?;
    audit::log(&state.pool, "user", "kg.entity.create", Some(&e.id), serde_json::json!({"type": e.entity_type}))?;
    Ok(e)
}

#[tauri::command]
pub fn kg_add_alias(state: State<'_, Arc<AppState>>, entity_id: String, alias: String) -> AppResult<()> {
    kg::graph::add_alias(&state.pool, &entity_id, &alias)
}

#[tauri::command]
pub fn kg_delete_entity(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    kg::graph::delete(&state.pool, &id)?;
    audit::log(&state.pool, "user", "kg.entity.delete", Some(&id), serde_json::json!({}))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    pub winner_id: String,
    pub loser_id: String,
    pub reason: Option<String>,
}

#[tauri::command]
pub fn kg_merge_entities(state: State<'_, Arc<AppState>>, request: MergeRequest) -> AppResult<kg::Entity> {
    let req = kg::MergeRequest { winner_id: request.winner_id, loser_id: request.loser_id, reason: request.reason };
    let winner = kg::graph::merge(&state.pool, &req)?;
    audit::log(&state.pool, "user", "kg.entity.merge", Some(&winner.id), serde_json::json!({"loser": req.loser_id}))?;
    Ok(winner)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRelationshipRequest {
    pub source_entity_id: String,
    pub relationship_type: String,
    pub target_entity_id: String,
    pub valid_from: Option<i64>,
    pub valid_until: Option<i64>,
    pub verification_status: Option<String>,
    pub weight: Option<f64>,
    pub metadata_json: Option<String>,
    pub evidence: Option<Vec<kg::CreateEvidence>>,
}

#[tauri::command]
pub fn kg_create_relationship(state: State<'_, Arc<AppState>>, request: CreateRelationshipRequest) -> AppResult<kg::Relationship> {
    let req = kg::CreateRelationship {
        source_entity_id: request.source_entity_id,
        relationship_type: request.relationship_type,
        target_entity_id: request.target_entity_id,
        valid_from: request.valid_from,
        valid_until: request.valid_until,
        verification_status: request.verification_status,
        weight: request.weight,
        metadata_json: request.metadata_json,
        evidence: request.evidence.unwrap_or_default(),
    };
    let r = kg::relationship::create(&state.pool, &req)?;
    audit::log(&state.pool, "user", "kg.rel.create", Some(&r.id), serde_json::json!({"type": r.relationship_type}))?;
    Ok(r)
}

#[tauri::command]
pub fn kg_set_relationship_status(state: State<'_, Arc<AppState>>, id: String, status: String) -> AppResult<kg::Relationship> {
    kg::relationship::set_status(&state.pool, &id, &status)
}

#[tauri::command]
pub fn kg_list_relationships_for(
    state: State<'_, Arc<AppState>>,
    entity_id: String,
    direction: String,
    limit: Option<usize>,
) -> AppResult<Vec<kg::Relationship>> {
    kg::relationship::list_for(&state.pool, &entity_id, &direction, limit.unwrap_or(200))
}

#[tauri::command]
pub fn kg_delete_relationship(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    kg::relationship::delete(&state.pool, &id)?;
    audit::log(&state.pool, "user", "kg.rel.delete", Some(&id), serde_json::json!({}))
}

#[tauri::command]
pub fn kg_evidence_for(state: State<'_, Arc<AppState>>, relationship_id: String) -> AppResult<Vec<kg::Evidence>> {
    kg::evidence::list_for(&state.pool, &relationship_id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddEvidenceRequest {
    pub relationship_id: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_location: Option<String>,
    pub excerpt: Option<String>,
}

#[tauri::command]
pub fn kg_add_evidence(state: State<'_, Arc<AppState>>, request: AddEvidenceRequest) -> AppResult<kg::Evidence> {
    let req = kg::CreateEvidence {
        source_kind: request.source_kind,
        source_id: request.source_id,
        source_location: request.source_location,
        excerpt: request.excerpt,
    };
    kg::evidence::add(&state.pool, &request.relationship_id, &req)
}

#[tauri::command]
pub fn kg_delete_evidence(state: State<'_, Arc<AppState>>, id: String) -> AppResult<()> {
    kg::evidence::delete(&state.pool, &id)
}

#[tauri::command]
pub fn kg_neighborhood(
    state: State<'_, Arc<AppState>>,
    center_id: String,
    max_depth: Option<usize>,
    max_nodes: Option<usize>,
) -> AppResult<kg::Neighborhood> {
    let b = kg::queries::Bounded { max_depth: max_depth.unwrap_or(2), max_nodes: max_nodes.unwrap_or(200) };
    kg::queries::neighborhood(&state.pool, &center_id, b)
}

#[tauri::command]
pub fn kg_shortest_path(state: State<'_, Arc<AppState>>, a: String, b: String, max_depth: Option<usize>) -> AppResult<Vec<String>> {
    kg::queries::shortest_path(&state.pool, &a, &b, max_depth.unwrap_or(4))
}

#[tauri::command]
pub fn kg_projects_using(state: State<'_, Arc<AppState>>, entity_id: String, limit: Option<usize>) -> AppResult<Vec<kg::Entity>> {
    kg::queries::projects_using_entity(&state.pool, &entity_id, limit.unwrap_or(50))
}

#[tauri::command]
pub fn kg_stats(state: State<'_, Arc<AppState>>) -> AppResult<kg::GraphStats> {
    kg::queries::stats(&state.pool)
}

#[tauri::command]
pub fn kg_seed_project(state: State<'_, Arc<AppState>>, project_id: String) -> AppResult<kg::Entity> {
    // Verify the project exists so we don't create orphan graph nodes.
    let _ = nexus_core::projects::projects::get(&state.pool, &project_id).map_err(|e| match e {
        AppError::NotFound(_) => AppError::Invalid(format!("project {project_id} not found")),
        other => other,
    })?;
    let e = kg::extract::seed_project(&state.pool, &project_id)?;
    audit::log(&state.pool, "user", "kg.seed.project", Some(&e.id), serde_json::json!({"project_id": project_id}))?;
    Ok(e)
}
