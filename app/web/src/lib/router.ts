// Typed IPC bindings for the model router (Phase F).

import { invoke } from "@tauri-apps/api/core";

export type RouterCapabilities = {
  profile_id: string;
  supports_chat: boolean;
  supports_code: boolean;
  supports_structured_output: boolean;
  supports_tool_calling: boolean;
  supports_streaming: boolean;
  is_embedding: boolean;
  approx_ctx_tokens: number;
  approx_memory_mb: number;
  verified: boolean;
  notes: string | null;
  updated_at: number;
};

export type RouterLifecycleStatus = {
  profile_id: string;
  state: string;
  last_transition: number;
  last_error: string | null;
  loaded_at: number | null;
  last_used_at: number | null;
  inflight: number;
};

export type RouterDecision = {
  selected: string | null;
  rejected: { profile_id: string; reason: string }[];
  reason: string;
  fell_back: boolean;
};

export type RouterDecisionRecord = {
  id: string;
  task: string;
  selected: string | null;
  fell_back: boolean;
  reason: string;
  created_at: number;
};

export async function routerCapabilitiesList(): Promise<RouterCapabilities[]> {
  return invoke("router_capabilities_list");
}
export async function routerCapabilitiesGet(profileId: string): Promise<RouterCapabilities> {
  return invoke("router_capabilities_get", { profileId });
}
export async function routerCapabilitiesUpsert(cap: RouterCapabilities): Promise<void> {
  return invoke("router_capabilities_upsert", { cap });
}
export async function routerCapabilitiesInfer(profileId: string): Promise<RouterCapabilities> {
  return invoke("router_capabilities_infer", { profileId });
}
export async function routerLifecycleList(): Promise<RouterLifecycleStatus[]> {
  return invoke("router_lifecycle_list");
}
export async function routerLifecycleTransition(
  profileId: string,
  to: string,
  error?: string | null,
): Promise<RouterLifecycleStatus> {
  return invoke("router_lifecycle_transition", { profileId, to, error: error ?? null });
}
export async function routerRoute(request: {
  task: string;
  min_context_tokens?: number | null;
  prefer_profile_id?: string | null;
}): Promise<RouterDecision> {
  return invoke("router_route", { request });
}
export async function routerDecisionsRecent(limit?: number): Promise<RouterDecisionRecord[]> {
  return invoke("router_decisions_recent", { limit: limit ?? null });
}
