// Project Time Machine — Phase S.1 frontend bindings.
// Mirrors `nexus_core::timeline` exactly. Read-only against source tables.

import { invoke } from "@tauri-apps/api/core";

export type TimelineEvent = {
  id: string;
  project_id: string | null;
  event_type: string;
  source_kind: string;
  source_id: string | null;
  source_label: string | null;
  event_at: number;
  imported_at: number;
  summary: string;
  detail_json: unknown;
  ingested_by: string;
  ingest_run_id: string | null;
  created_at: number;
};

export type IngestReport = {
  run_id: string;
  examined: number;
  inserted: number;
  skipped_existing: number;
  sources: SourceReport[];
};

export type SourceReport = {
  source_kind: string;
  examined: number;
  inserted: number;
  skipped_existing: number;
};

export type ReconstructionItem = {
  id: string;
  label: string;
  at_ms: number;
  detail: unknown;
};

export type SnapshotSection = {
  label: string;
  /** "known" | "partial" */
  status: string;
  count: number;
  items: ReconstructionItem[];
  note: string | null;
};

export type Reconstruction = {
  project_id: string;
  at_ms: number;
  generated_at: number;
  sections: SnapshotSection[];
  events: TimelineEvent[];
  unknowns: string[];
};

export type Comparison = {
  project_id: string;
  from_ms: number;
  to_ms: number;
  added: TimelineEvent[];
  note: string;
};

export type SupersededItem = {
  id: string;
  label: string;
  at_ms: number;
  valid_until: number | null;
  superseded_by: string | null;
  detail: unknown;
};

export type Superseded = {
  memories: SupersededItem[];
  kg_relationships: SupersededItem[];
};

export type TimelineListRequest = {
  project_id?: string | null;
  source_kind?: string | null;
  event_type?: string | null;
  since_ms?: number | null;
  until_ms?: number | null;
  limit?: number | null;
};

export type InsertManualRequest = {
  project_id: string;
  summary: string;
  event_at?: number | null;
  detail: unknown;
};

export async function timelineIngestProject(projectId: string): Promise<IngestReport> {
  return invoke("timeline_ingest_project", { projectId });
}

export async function timelineList(request: TimelineListRequest): Promise<TimelineEvent[]> {
  return invoke("timeline_list", { request });
}

export async function timelineCount(projectId: string): Promise<number> {
  return invoke("timeline_count", { projectId });
}

export async function timelineReconstruct(projectId: string, atMs: number): Promise<Reconstruction> {
  return invoke("timeline_reconstruct", { projectId, atMs });
}

export async function timelineCompare(
  projectId: string,
  fromMs: number,
  toMs: number,
): Promise<Comparison> {
  return invoke("timeline_compare", { projectId, fromMs, toMs });
}

export async function timelineSuperseded(projectId: string): Promise<Superseded> {
  return invoke("timeline_superseded", { projectId });
}

export async function timelineDecisions(projectId: string): Promise<TimelineEvent[]> {
  return invoke("timeline_decisions", { projectId });
}

export async function timelineInsertManual(request: InsertManualRequest): Promise<TimelineEvent> {
  return invoke("timeline_insert_manual", { request });
}

export async function timelineDelete(id: string): Promise<void> {
  return invoke("timeline_delete", { id });
}
