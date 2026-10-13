// Typed IPC bindings for Experiment Tracking. Phase H.

import { invoke } from "@tauri-apps/api/core";

export type Experiment = {
  id: string;
  project_id: string | null;
  name: string;
  description: string | null;
  status: string;
  tags: string[];
  created_at: number;
  updated_at: number;
};

export type ExperimentRun = {
  id: string;
  experiment_id: string;
  name: string;
  status: string;
  hyperparams: unknown;
  metrics: unknown;
  code_revision: string | null;
  code_path: string | null;
  hardware: string | null;
  notes: string | null;
  started_at: number | null;
  ended_at: number | null;
  created_at: number;
  updated_at: number;
};

export type MetricPoint = {
  run_id: string;
  run_name: string;
  metric: string;
  value: number;
};

export async function experimentCreate(exp: Experiment): Promise<Experiment> {
  return invoke("experiment_create", { exp });
}
export async function experimentGet(id: string): Promise<Experiment> {
  return invoke("experiment_get", { id });
}
export async function experimentList(project_id: string | null = null, status: string | null = null, limit: number | null = null): Promise<Experiment[]> {
  return invoke("experiment_list", { projectId: project_id, status, limit });
}
export async function experimentUpdate(exp: Experiment): Promise<Experiment> {
  return invoke("experiment_update", { exp });
}
export async function experimentDelete(id: string): Promise<void> {
  return invoke("experiment_delete", { id });
}

export async function experimentRunCreate(run: ExperimentRun): Promise<ExperimentRun> {
  return invoke("experiment_run_create", { run });
}
export async function experimentRunGet(id: string): Promise<ExperimentRun> {
  return invoke("experiment_run_get", { id });
}
export async function experimentRunList(experiment_id: string): Promise<ExperimentRun[]> {
  return invoke("experiment_run_list", { experimentId: experiment_id });
}
export async function experimentRunUpdate(run: ExperimentRun): Promise<ExperimentRun> {
  return invoke("experiment_run_update", { run });
}
export async function experimentRunDelete(id: string): Promise<void> {
  return invoke("experiment_run_delete", { id });
}
export async function experimentMetricHistory(experiment_id: string): Promise<MetricPoint[]> {
  return invoke("experiment_metric_history", { experimentId: experiment_id });
}
