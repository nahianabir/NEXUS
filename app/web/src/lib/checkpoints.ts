// One-Click Work Resume — Phase S.2 frontend bindings.
// Mirrors `nexus_core::checkpoints` exactly.

import { invoke } from "@tauri-apps/api/core";

export type ObservedFile = {
  path: string;
  label: string | null;
  sha256: string | null;
  size_bytes: number | null;
  mtime_ms: number | null;
};

export type ObservedTest = {
  name: string;
  outcome: string;
  note: string | null;
  at_ms: number | null;
};

export type CapturedDecision = {
  summary: string;
  rationale: string | null;
  memory_id: string | null;
};

export type PendingAction = {
  text: string;
  done: boolean;
};

export type CapturedError = {
  text: string;
  at_ms: number | null;
};

export type CapturedConstraint = {
  text: string;
};

export type EvidenceRef = {
  kind: string;
  id: string;
};

export type FileSnapshot = {
  path: string;
  size_bytes: number | null;
  mtime_ms: number | null;
  exists: boolean;
};

export type DriftCheck = {
  open_task_count: number | null;
  note_count: number | null;
  active_model: string | null;
  files: FileSnapshot[];
  captured_at: number | null;
};

export type DriftItem = {
  severity: "info" | "warn" | "conflict";
  kind: string;
  summary: string;
  expected: string | null;
  actual: string | null;
};

export type DriftReport = {
  items: DriftItem[];
  files_now: FileSnapshot[];
  open_task_count_now: number | null;
  note_count_now: number | null;
  active_model_now: string | null;
  generated_at: number;
};

export type Checkpoint = {
  id: string;
  project_id: string;
  task_id: string | null;
  title: string;
  status: string;
  goal: string | null;
  completed_work: string | null;
  current_state: string | null;
  next_action_text: string | null;
  observed_files: ObservedFile[];
  observed_tests: ObservedTest[];
  errors: CapturedError[];
  decisions: CapturedDecision[];
  constraints: CapturedConstraint[];
  pending_actions: PendingAction[];
  evidence_refs: EvidenceRef[];
  drift_check: DriftCheck;
  created_by: string;
  model_hint: string | null;
  created_at: number;
  updated_at: number;
  resumed_at: number | null;
};

export type ResumeRecord = {
  id: string;
  checkpoint_id: string;
  resumed_at: number;
  drift_report: DriftReport;
  note: string | null;
};

export type CreateCheckpointRequest = {
  project_id: string;
  task_id?: string | null;
  title: string;
  goal?: string | null;
  completed_work?: string | null;
  current_state?: string | null;
  next_action_text?: string | null;
  observed_files: ObservedFile[];
  observed_tests: ObservedTest[];
  errors: CapturedError[];
  decisions: CapturedDecision[];
  constraints: CapturedConstraint[];
  pending_actions: PendingAction[];
  evidence_refs: EvidenceRef[];
  created_by?: string | null;
  model_hint?: string | null;
};

export type UpdateCheckpointRequest = {
  title?: string | null;
  /** Tri-state: omitted = leave alone, `null` = clear, `string` = set. */
  goal?: string | null | undefined;
  completed_work?: string | null | undefined;
  current_state?: string | null | undefined;
  next_action_text?: string | null | undefined;
  observed_files?: ObservedFile[] | null;
  observed_tests?: ObservedTest[] | null;
  errors?: CapturedError[] | null;
  decisions?: CapturedDecision[] | null;
  constraints?: CapturedConstraint[] | null;
  pending_actions?: PendingAction[] | null;
  evidence_refs?: EvidenceRef[] | null;
};

export type ResumeResponse = {
  checkpoint: Checkpoint;
  drift_report: DriftReport;
  summary: string;
  resume_record_id: string;
};

export type ListCheckpointsRequest = {
  project_id?: string | null;
  task_id?: string | null;
  status?: string | null;
  limit?: number | null;
};

export async function checkpointsList(request: ListCheckpointsRequest): Promise<Checkpoint[]> {
  return invoke("checkpoints_list", { request });
}

export async function checkpointsGet(id: string): Promise<Checkpoint> {
  return invoke("checkpoints_get", { id });
}

export async function checkpointsCreate(request: CreateCheckpointRequest): Promise<Checkpoint> {
  return invoke("checkpoints_create", { request });
}

export async function checkpointsUpdate(id: string, request: UpdateCheckpointRequest): Promise<Checkpoint> {
  // The Rust side uses a tri-state Option<Option<String>> pattern.
  // JS can't express that directly, so we serialise: present
  // and not null => Some(Some(s)), present and null => Some(None),
  // omitted => None.
  return invoke("checkpoints_update", { id, request });
}

export async function checkpointsSetStatus(id: string, status: string): Promise<Checkpoint> {
  return invoke("checkpoints_set_status", { id, status });
}

export async function checkpointsDelete(id: string): Promise<void> {
  return invoke("checkpoints_delete", { id });
}

export async function checkpointsResume(id: string, note?: string | null): Promise<ResumeResponse> {
  return invoke("checkpoints_resume", { id, note: note ?? null });
}

export async function checkpointsListResumes(checkpointId: string): Promise<ResumeRecord[]> {
  return invoke("checkpoints_list_resumes", { checkpointId });
}
