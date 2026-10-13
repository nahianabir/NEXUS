// Typed IPC bindings for the Knowledge Graph. Phase D.

import { invoke } from "@tauri-apps/api/core";

export type KgEntity = {
  id: string;
  workspace_id: string | null;
  entity_type: string;
  canonical_name: string;
  description: string | null;
  metadata_json: string | null;
  merged_into: string | null;
  created_at: number;
  updated_at: number;
};

export type KgRelationship = {
  id: string;
  source_entity_id: string;
  relationship_type: string;
  target_entity_id: string;
  valid_from: number | null;
  valid_until: number | null;
  created_at: number;
  verification_status: string;
  weight: number;
  metadata_json: string | null;
};

export type KgEvidence = {
  id: string;
  relationship_id: string;
  source_kind: string;
  source_id: string | null;
  source_location: string | null;
  excerpt: string | null;
  created_at: number;
};

export type KgNeighborhood = {
  center: KgEntity;
  nodes: KgEntity[];
  edges: KgRelationship[];
  evidence_by_rel: KgEvidence[][];
};

export type KgStats = {
  entities: number;
  relationships: number;
  evidence: number;
  by_type: [string, number][];
};

export async function kgEntityTypes(): Promise<string[]> { return invoke("kg_entity_types"); }
export async function kgRelationshipTypes(): Promise<string[]> { return invoke("kg_relationship_types"); }
export async function kgListEntities(filter: {
  entity_type?: string | null;
  workspace_id?: string | null;
  search?: string | null;
  limit?: number | null;
  include_merged?: boolean;
} = {}): Promise<KgEntity[]> {
  return invoke("kg_list_entities", {
    entityType: filter.entity_type ?? null,
    workspaceId: filter.workspace_id ?? null,
    search: filter.search ?? null,
    limit: filter.limit ?? null,
    includeMerged: filter.include_merged ?? false,
  });
}
export async function kgSearch(query: string, limit?: number): Promise<KgEntity[]> {
  return invoke("kg_search", { query, limit: limit ?? null });
}
export async function kgGetEntity(id: string): Promise<KgEntity> { return invoke("kg_get_entity", { id }); }

export async function kgCreateEntity(req: {
  workspace_id?: string | null;
  entity_type: string;
  canonical_name: string;
  description?: string | null;
  metadata_json?: string | null;
  aliases?: string[] | null;
}): Promise<KgEntity> {
  return invoke("kg_create_entity", { request: req });
}
export async function kgAddAlias(entityId: string, alias: string): Promise<void> {
  return invoke("kg_add_alias", { entityId, alias });
}
export async function kgDeleteEntity(id: string): Promise<void> { return invoke("kg_delete_entity", { id }); }
export async function kgMergeEntities(winnerId: string, loserId: string, reason?: string): Promise<KgEntity> {
  return invoke("kg_merge_entities", { request: { winner_id: winnerId, loser_id: loserId, reason: reason ?? null } });
}

export async function kgCreateRelationship(req: {
  source_entity_id: string;
  relationship_type: string;
  target_entity_id: string;
  valid_from?: number | null;
  valid_until?: number | null;
  verification_status?: string | null;
  weight?: number | null;
  metadata_json?: string | null;
  evidence?: { source_kind: string; source_id?: string | null; source_location?: string | null; excerpt?: string | null }[] | null;
}): Promise<KgRelationship> {
  return invoke("kg_create_relationship", { request: req });
}
export async function kgSetRelationshipStatus(id: string, status: string): Promise<KgRelationship> {
  return invoke("kg_set_relationship_status", { id, status });
}
export async function kgListRelationshipsFor(entityId: string, direction: "in" | "out", limit?: number): Promise<KgRelationship[]> {
  return invoke("kg_list_relationships_for", { entityId, direction, limit: limit ?? null });
}
export async function kgDeleteRelationship(id: string): Promise<void> { return invoke("kg_delete_relationship", { id }); }

export async function kgEvidenceFor(relationshipId: string): Promise<KgEvidence[]> {
  return invoke("kg_evidence_for", { relationshipId });
}
export async function kgAddEvidence(req: {
  relationship_id: string;
  source_kind: string;
  source_id?: string | null;
  source_location?: string | null;
  excerpt?: string | null;
}): Promise<KgEvidence> { return invoke("kg_add_evidence", { request: req }); }
export async function kgDeleteEvidence(id: string): Promise<void> { return invoke("kg_delete_evidence", { id }); }

export async function kgNeighborhood(centerId: string, maxDepth?: number, maxNodes?: number): Promise<KgNeighborhood> {
  return invoke("kg_neighborhood", { centerId, maxDepth: maxDepth ?? null, maxNodes: maxNodes ?? null });
}
export async function kgShortestPath(a: string, b: string, maxDepth?: number): Promise<string[]> {
  return invoke("kg_shortest_path", { a, b, maxDepth: maxDepth ?? null });
}
export async function kgProjectsUsing(entityId: string, limit?: number): Promise<KgEntity[]> {
  return invoke("kg_projects_using", { entityId, limit: limit ?? null });
}
export async function kgStats(): Promise<KgStats> { return invoke("kg_stats"); }
export async function kgSeedProject(projectId: string): Promise<KgEntity> { return invoke("kg_seed_project", { projectId }); }
