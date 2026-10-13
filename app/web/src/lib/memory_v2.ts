// Typed IPC bindings for native long-term memory v2. Phase 5B.

import { invoke } from "@tauri-apps/api/core";

// ---------------- memories_v2 ----------------

export type MemoryCategory =
  | "preference"
  | "instruction"
  | "project_fact"
  | "decision"
  | "constraint"
  | "hypothesis"
  | "decision_record"
  | "experiment"
  | "problem"
  | "task_commit"
  | "milestone"
  | "lesson";

export type MemoryV2 = {
  id: string;
  workspace_id: string | null;
  project_id: string | null;
  conversation_id: string | null;
  category: string;
  content: string;
  content_hash: string;
  normalized_content: string;
  source_kind: string;
  source_id: string | null;
  source_label: string | null;
  source_excerpt: string | null;
  event_at: number | null;
  valid_from: number | null;
  valid_until: number | null;
  verified_at: number | null;
  superseded_by: string | null;
  deleted_at: number | null;
  approved: boolean;
  verification: string;
  pinned: boolean;
  created_at: number;
  updated_at: number;
};

export type CreateMemoryV2Request = {
  workspace_id?: string | null;
  project_id?: string | null;
  conversation_id?: string | null;
  category: MemoryCategory | string;
  content: string;
  source_kind: string;
  source_id?: string | null;
  source_label?: string | null;
  source_excerpt?: string | null;
  event_at?: number | null;
  valid_from?: number | null;
  valid_until?: number | null;
  pinned?: boolean | null;
};

export async function memoriesV2List(
  filter: {
    projectId?: string | null;
    workspaceId?: string | null;
    category?: string | null;
    approvedOnly?: boolean;
    includeSuperseded?: boolean;
    includeDeleted?: boolean;
    sourceKind?: string | null;
    limit?: number | null;
  } = {},
): Promise<MemoryV2[]> {
  return invoke("memories_v2_list", {
    projectId: filter.projectId ?? null,
    workspaceId: filter.workspaceId ?? null,
    category: filter.category ?? null,
    approvedOnly: filter.approvedOnly ?? true,
    includeSuperseded: filter.includeSuperseded ?? false,
    includeDeleted: filter.includeDeleted ?? false,
    sourceKind: filter.sourceKind ?? null,
    limit: filter.limit ?? null,
  });
}

export async function memoriesV2Get(id: string): Promise<MemoryV2> {
  return invoke("memories_v2_get", { id });
}

export async function memoriesV2Create(request: CreateMemoryV2Request): Promise<MemoryV2> {
  return invoke("memories_v2_create", { request });
}

export type UpdateMemoryV2Request = {
  id: string;
  content?: string | null;
  category?: string | null;
  pinned?: boolean | null;
  valid_from?: { value: number | null } | null;
  valid_until?: { value: number | null } | null;
  event_at?: { value: number | null } | null;
  verified_at?: number | null;
};

export async function memoriesV2Update(request: UpdateMemoryV2Request): Promise<MemoryV2> {
  return invoke("memories_v2_update", { request });
}

export async function memoriesV2Approve(id: string): Promise<MemoryV2> {
  return invoke("memories_v2_approve", { id });
}

export async function memoriesV2Supersede(oldId: string, newId: string): Promise<void> {
  return invoke("memories_v2_supersede", { oldId, newId });
}

export async function memoriesV2Delete(id: string, hard = false): Promise<void> {
  return invoke("memories_v2_delete", { id, hard });
}

export async function memoriesV2Search(
  query: string,
  filter: { category?: string | null; projectId?: string | null; limit?: number | null } = {},
): Promise<MemoryV2[]> {
  return invoke("memories_v2_search", {
    query,
    category: filter.category ?? null,
    projectId: filter.projectId ?? null,
    limit: filter.limit ?? null,
  });
}

export async function memoriesV2Categories(): Promise<string[]> {
  return invoke("memories_v2_categories");
}

export async function memoriesV2MigrateFromV1(): Promise<number> {
  return invoke("memories_v2_migrate_from_v1");
}

// ---------------- evidence -------------------

export type Evidence = {
  id: string;
  memory_id: string;
  source_kind: string;
  source_id: string | null;
  source_location: string | null;
  excerpt: string | null;
  verified: boolean;
  created_at: number;
};

export async function memoryEvidenceList(memoryId: string): Promise<Evidence[]> {
  return invoke("memory_evidence_list", { memoryId });
}
export async function memoryEvidenceAdd(
  memoryId: string,
  sourceKind: string,
  sourceId?: string | null,
  sourceLocation?: string | null,
  excerpt?: string | null,
): Promise<Evidence> {
  return invoke("memory_evidence_add", {
    memoryId,
    sourceKind,
    sourceId: sourceId ?? null,
    sourceLocation: sourceLocation ?? null,
    excerpt: excerpt ?? null,
  });
}
export async function memoryEvidenceVerify(id: string): Promise<void> {
  return invoke("memory_evidence_verify", { id });
}
export async function memoryEvidenceDelete(id: string): Promise<void> {
  return invoke("memory_evidence_delete", { id });
}

// ---------------- candidates -----------------

export type Candidate = {
  id: string;
  category: string;
  content: string;
  source_kind: string;
  source_id: string | null;
  source_excerpt: string | null;
  confidence: number;
  status: string;
  rejection_reason: string | null;
  project_id: string | null;
  conversation_id: string | null;
  created_at: number;
  decided_at: number | null;
};

export async function memoryCandidatesList(status = "pending", limit = 100): Promise<Candidate[]> {
  return invoke("memory_candidates_list", { status, limit });
}

export async function memoryCandidatesPropose(
  request: {
    category: MemoryCategory | string;
    content: string;
    source_kind: string;
    source_id?: string | null;
    source_excerpt?: string | null;
    confidence: number;
    project_id?: string | null;
    conversation_id?: string | null;
  },
): Promise<Candidate> {
  return invoke("memory_candidates_propose", { request });
}
export async function memoryCandidatesApprove(id: string): Promise<MemoryV2> {
  return invoke("memory_candidates_approve", { id });
}
export async function memoryCandidatesReject(id: string, reason?: string | null): Promise<void> {
  return invoke("memory_candidates_reject", { id, reason: reason ?? null });
}

// ---------------- sessions -------------------

export type Session = {
  id: string;
  workspace_id: string | null;
  project_id: string | null;
  conversation_id: string | null;
  title: string;
  summary: string;
  work_completed: string | null;
  decisions_made: string | null;
  files_changed: string | null;
  experiments_performed: string | null;
  errors_encountered: string | null;
  open_questions: string | null;
  next_actions: string | null;
  approved: boolean;
  started_at: number;
  ended_at: number;
  created_at: number;
};

export async function memorySessionsList(projectId: string, limit = 50): Promise<Session[]> {
  return invoke("memory_sessions_list", { projectId, limit });
}
export async function memorySessionsGet(id: string): Promise<Session> {
  return invoke("memory_sessions_get", { id });
}
export async function memorySessionsCreate(request: {
  workspace_id?: string | null;
  project_id?: string | null;
  conversation_id?: string | null;
  title: string;
  summary: string;
  work_completed?: string | null;
  decisions_made?: string | null;
  files_changed?: string | null;
  experiments_performed?: string | null;
  errors_encountered?: string | null;
  open_questions?: string | null;
  next_actions?: string | null;
  started_at: number;
  ended_at: number;
}): Promise<Session> {
  return invoke("memory_sessions_create", { request });
}
export async function memorySessionsDelete(id: string): Promise<void> {
  return invoke("memory_sessions_delete", { id });
}
