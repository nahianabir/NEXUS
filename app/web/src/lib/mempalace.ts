// Typed IPC bindings for the MemPalace adapter. Phase C.

import { invoke } from "@tauri-apps/api/core";

export type BackendKind = "native" | "mempalace" | "disabled";

export type BackendStatus = {
  active: string;
  native_healthy: boolean;
  mempalace_healthy: boolean;
  mempalace_version: string | null;
  mempalace_message: string;
};

export type AdapterMemory = {
  id: string;
  backend: string;
  project_id: string | null;
  workspace_id: string | null;
  category: string;
  content: string;
  source_kind: string;
  source_id: string | null;
  source_excerpt: string | null;
  event_at: number | null;
  valid_from: number | null;
  valid_until: number | null;
  created_at: number;
  updated_at: number;
};

export type MemPalaceConfig = {
  binary: string;
  cwd?: string | null;
  timeout_ms: number;
  extra_args?: string[];
};

export async function memoryBackendStatus(): Promise<BackendStatus> {
  return invoke("memory_backend_status");
}
export async function memoryBackendActivateNative(): Promise<void> {
  return invoke("memory_backend_activate_native");
}
export async function memoryBackendDisable(): Promise<void> {
  return invoke("memory_backend_disable");
}
export async function memoryBackendActivateMemPalace(
  config: MemPalaceConfig,
): Promise<BackendStatus> {
  return invoke("memory_backend_activate_mempalace", { config });
}
export async function memoryBackendSearch(
  query: string,
  projectId?: string | null,
  limit?: number,
): Promise<AdapterMemory[]> {
  return invoke("memory_backend_search", {
    query,
    projectId: projectId ?? null,
    limit: limit ?? null,
  });
}
export async function memoryBackendStore(request: {
  project_id?: string | null;
  workspace_id?: string | null;
  category: string;
  content: string;
  source_kind: string;
  source_id?: string | null;
  source_excerpt?: string | null;
  event_at?: number | null;
  valid_from?: number | null;
  valid_until?: number | null;
}): Promise<string> {
  return invoke("memory_backend_store", { request });
}
export async function memoryBackendDelete(id: string): Promise<void> {
  return invoke("memory_backend_delete", { id });
}
