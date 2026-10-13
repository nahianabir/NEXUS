// Phase S.3 frontend bindings.
//
// Three additive features:
//   * Opportunity Radar    (read-only)
//   * Hypothesis Ledger    (CRUD + decide + evidence)
//   * Codebase Archaeologist (read-only index + head)

import { invoke } from "@tauri-apps/api/core";

// ---------------- Opportunity Radar ----------------

export type OpportunityKind =
  | "overdue_urgent_task"
  | "overdue_high_task"
  | "stale_open_task"
  | "resumed_checkpoint_with_pending"
  | "high_importance_recent_memory"
  | "open_hypothesis";

export type OpportunityRef = {
  kind: string;
  id: string;
};

export type Opportunity = {
  kind: OpportunityKind;
  title: string;
  summary: string;
  score: number;
  refs: OpportunityRef[];
  suggested_action: string;
  metric: number | null;
  key: string;
};

export type RadarCounts = {
  open_tasks: number;
  overdue_tasks: number;
  resumed_checkpoints: number;
  open_hypotheses: number;
  high_importance_memories: number;
};

export type RadarReport = {
  project_id: string;
  generated_at: number;
  items: Opportunity[];
  counts: RadarCounts;
};

export type RadarRequest = {
  project_id: string;
  stale_days?: number | null;
  memory_recent_days?: number | null;
  high_importance_threshold?: number | null;
  per_kind_limit?: number | null;
  weights?: Weights | null;
};

export type Weights = {
  overdue_urgent_task_base: number;
  overdue_high_task_base: number;
  overdue_day_factor: number;
  stale_open_task_base: number;
  stale_day_factor: number;
  resumed_checkpoint_base: number;
  resumed_pending_factor: number;
  high_importance_memory_base: number;
  high_importance_factor: number;
  open_hypothesis_base: number;
  open_hypothesis_confidence_factor: number;
};

export async function radarReport(request: RadarRequest): Promise<RadarReport> {
  return invoke("radar_report", { request });
}

export async function radarCounts(projectId: string): Promise<RadarCounts> {
  return invoke("radar_counts", { projectId });
}

// ---------------- Hypothesis Ledger ----------------

export type HypothesisStatus = "open" | "confirmed" | "refuted" | "abandoned";
export type Stance = "for" | "against";

export type EvidenceEntry = {
  kind: string;
  id: string;
  summary: string;
  weight: number;
};

export type Hypothesis = {
  id: string;
  project_id: string;
  task_id: string | null;
  title: string;
  statement: string;
  prediction: string | null;
  status: HypothesisStatus;
  confidence: number;
  evidence_for: EvidenceEntry[];
  evidence_against: EvidenceEntry[];
  decided_at: number | null;
  decision_note: string | null;
  memory_id: string | null;
  created_at: number;
  updated_at: number;
};

export type HypothesisEvidence = {
  id: string;
  hypothesis_id: string;
  kind: string;
  summary: string;
  weight: number;
  source_kind: string | null;
  source_id: string | null;
  stance: Stance;
  created_at: number;
};

export type ListHypothesesRequest = {
  project_id?: string | null;
  task_id?: string | null;
  status?: string | null;
  limit?: number | null;
};

export type CreateHypothesisRequest = {
  project_id: string;
  task_id?: string | null;
  title: string;
  statement: string;
  prediction?: string | null;
  confidence: number;
  evidence_for: EvidenceEntry[];
  evidence_against: EvidenceEntry[];
  memory_id?: string | null;
};

export type UpdateHypothesisRequest = {
  title?: string | null;
  statement?: string | null;
  prediction?: string | null | undefined;
  confidence?: number | null;
  evidence_for?: EvidenceEntry[] | null;
  evidence_against?: EvidenceEntry[] | null;
  memory_id?: string | null | undefined;
};

export type AddEvidenceRequest = {
  hypothesis_id: string;
  kind: string;
  summary: string;
  weight: number;
  source_kind?: string | null;
  source_id?: string | null;
  stance: Stance;
};

export async function hypothesesList(request: ListHypothesesRequest): Promise<Hypothesis[]> {
  return invoke("hypotheses_list", { request });
}
export async function hypothesesGet(id: string): Promise<Hypothesis> {
  return invoke("hypotheses_get", { id });
}
export async function hypothesesCreate(request: CreateHypothesisRequest): Promise<Hypothesis> {
  return invoke("hypotheses_create", { request });
}
export async function hypothesesUpdate(id: string, request: UpdateHypothesisRequest): Promise<Hypothesis> {
  return invoke("hypotheses_update", { id, request });
}
export async function hypothesesDecide(id: string, decision: HypothesisStatus, note?: string | null): Promise<Hypothesis> {
  return invoke("hypotheses_decide", { id, decision, note: note ?? null });
}
export async function hypothesesDelete(id: string): Promise<void> {
  return invoke("hypotheses_delete", { id });
}
export async function hypothesesAddEvidence(request: AddEvidenceRequest): Promise<HypothesisEvidence> {
  return invoke("hypotheses_add_evidence", { request });
}
export async function hypothesesListEvidence(hypothesisId: string): Promise<HypothesisEvidence[]> {
  return invoke("hypotheses_list_evidence", { hypothesisId });
}
export async function hypothesesDeleteEvidence(id: string): Promise<void> {
  return invoke("hypotheses_delete_evidence", { id });
}

// ---------------- Codebase Archaeologist ----------------

export type FileKind =
  | "rust" | "typescript" | "javascript" | "python" | "markdown"
  | "toml" | "yaml" | "json" | "sql" | "shell" | "text" | "binary" | "other";

export type FileEntry = {
  path: string;
  size_bytes: number;
  mtime_ms: number;
  kind: FileKind;
  sha256: string | null;
};

export type IndexSummary = {
  root: string;
  total_files: number;
  total_bytes: number;
  by_kind: Record<string, number>;
  files: FileEntry[];
  skipped: string[];
  indexed_at: number;
};

export type HeadResult = {
  path: string;
  size_bytes: number;
  total_lines: number;
  head: string;
  truncated: boolean;
  sha256: string | null;
};

export type IndexRequest = {
  root: string;
  hash?: boolean | null;
  max_file_bytes?: number | null;
  max_files?: number | null;
  max_depth?: number | null;
  exclusions?: string[] | null;
};

export async function archaeologistIndex(request: IndexRequest): Promise<IndexSummary> {
  return invoke("archaeologist_index", { request });
}

export async function archaeologistHead(root: string, relPath: string, n?: number): Promise<HeadResult> {
  return invoke("archaeologist_head", { root, relPath, n: n ?? null });
}

// ---------------- Serendipity (read-only) ----------------

export type DormantKind =
  | "dormant_memory" | "dormant_note" | "dormant_experiment"
  | "forgotten_hypothesis" | "cold_checkpoint";

export type DormantItem = {
  kind: DormantKind;
  id: string;
  project_id: string;
  title: string;
  summary: string;
  days_dormant: number;
  score: number;
  ref_kind: string;
};

export type CrossProjectHit = {
  left: DormantItem;
  right: DormantItem;
  shared_count: number;
};

export type SerendipityReport = {
  project_id: string;
  generated_at: number;
  items: DormantItem[];
  cross_project: CrossProjectHit[];
};

export type SerendipityRequest = {
  project_id: string;
  dormant_days?: number | null;
  per_kind_limit?: number | null;
  cross_project_limit?: number | null;
  weights?: SerendipityWeights | null;
};

export type SerendipityWeights = {
  dormant_memory_factor: number;
  dormant_note_factor: number;
  dormant_experiment_factor: number;
  forgotten_hypothesis_base: number;
  forgotten_hypothesis_confidence_factor: number;
  cold_checkpoint_factor: number;
};

export async function serendipityReport(request: SerendipityRequest): Promise<SerendipityReport> {
  return invoke("serendipity_report", { request });
}

// ---------------- Impact Simulator (read-only) ----------------

export type ImpactAction =
  | "delete_note" | "delete_memory" | "delete_task" | "delete_checkpoint"
  | "delete_hypothesis" | "delete_project" | "complete_task";

export type ImpactCount = { table: string; count: number };
export type AffectedRow = { table: string; id: string; title: string };

export type ImpactReport = {
  action: ImpactAction;
  target_id: string;
  generated_at: number;
  counts: ImpactCount[];
  affected: AffectedRow[];
  warnings: string[];
  destructive: boolean;
};

export type ImpactCommandRequest = {
  action: ImpactAction;
  target_id: string;
};

export async function impactSimulate(request: ImpactCommandRequest): Promise<ImpactReport> {
  return invoke("impact_simulate", { request });
}

// ---------------- Phase F Radar Feedback ----------------

export type RadarFeedbackKind =
  | "dismiss"
  | "snooze"
  | "pin"
  | "done"
  | "false";

export type RadarFeedback = {
  id: string;
  project_id: string;
  key: string;
  kind: RadarFeedbackKind;
  title: string | null;
  note: string | null;
  until_ms: number | null;
  created_at: number;
  updated_at: number;
};

export type RadarFeedbackRequest = {
  project_id: string;
  key: string;
  kind: RadarFeedbackKind;
  title?: string | null;
  note?: string | null;
  until_ms?: number | null;
};

export async function radarFeedbackUpsert(
  request: RadarFeedbackRequest,
): Promise<RadarFeedback> {
  return invoke("radar_feedback_upsert", { request });
}

export async function radarFeedbackDelete(id: string): Promise<void> {
  return invoke("radar_feedback_delete", { id });
}

export async function radarFeedbackList(
  request: ListRadarFeedbackRequest = {},
): Promise<RadarFeedback[]> {
  return invoke("radar_feedback_list", { request });
}

export type ListRadarFeedbackRequest = {
  project_id?: string;
  kind?: string;
  limit?: number;
};

// ---------------- Phase G Evidence Tribunal ----------------

export type TribunalStance =
  | "supports"
  | "contradicts"
  | "partial"
  | "unjudged";

export type EvidenceCard = {
  source_kind: string;
  source_id: string;
  project_id: string;
  snippet: string;
  stance: TribunalStance;
  confidence: number;
  reasons: string[];
  href: string | null;
};

export type TribunalCounts = {
  supports: number;
  contradicts: number;
  partial: number;
  unjudged: number;
};

export type TribunalReport = {
  question: string;
  generated_at: number;
  cards: EvidenceCard[];
  counts: TribunalCounts;
  notes: string[];
};

export type TribunalRequest = {
  question: string;
  project_id?: string | null;
  repo_root?: string | null;
  limit_per_source?: number | null;
};

export async function evidenceTribunalRun(
  request: TribunalRequest,
): Promise<TribunalReport> {
  return invoke("evidence_tribunal_run", { request });
}

// ---------------- Phase J Knowledge Vault Export ----------------

export type VaultExportKind = string;

export type VaultExportRequest = {
  project_id?: string | null;
  kinds?: VaultExportKind[] | null;
};

export type VaultExportManifest = {
  format: string;
  schema_version: number;
  app_version: string;
  exported_at_ms: number;
  project_id: string | null;
  kinds: string[];
  record_counts: Record<string, number>;
  records_sha256: string;
};

export type VaultExportArchive = {
  manifest: VaultExportManifest;
  records: Record<string, unknown>;
};

export type VaultExportResponse = {
  json: string;
  archive: VaultExportArchive;
  path: string;
};

export async function vaultExportRun(
  request: VaultExportRequest,
  dest: string,
): Promise<VaultExportResponse> {
  return invoke("vault_export_run", { request, dest });
}

export async function vaultExportInspect(
  request: VaultExportRequest = {},
): Promise<VaultExportArchive> {
  return invoke("vault_export_inspect", { request });
}

