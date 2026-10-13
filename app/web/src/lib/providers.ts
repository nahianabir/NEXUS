// Typed IPC bindings for the model manager commands added in Phase 2.
// Everything is `invoke`-based; streaming uses Tauri events.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ProviderKind = "lmstudio" | "llamacpp" | "ollama";

export type ProviderHealth = {
  reachable: boolean;
  base_url: string;
  latency_ms: number | null;
  error: string | null;
  version: string | null;
};

export type ModelInfo = {
  id: string;
  display_name: string;
  provider: string;
  context_len: number | null;
  supports_tools: boolean;
  gguf_path: string | null;
};

export type ProviderProbe = {
  kind: string;
  health: ProviderHealth;
  models: ModelInfo[];
};

export type ModelProfile = {
  id: string;
  role: string;
  display_name: string;
  provider: string;
  model_id: string;
  gguf_path: string | null;
  context_len: number;
  gpu_layers: number | null;
  temperature: number;
  top_p: number;
  max_tokens: number;
  enabled: boolean;
  created_at: number;
  updated_at: number;
  /** Embedding vector dimension. Set automatically by
   *  `modelsTestEmbedding` or by the user in the Model
   *  Manager. `null` for non-embedding profiles or legacy
   *  rows. */
  dimension: number | null;
  /** Stable identity of the embedding model, used to refuse
   *  silently mixing different models that happen to share a
   *  dimension. `null` for non-embedding profiles. */
  model_identity: string | null;
  /** True if the model advertises vision capability and
   *  can accept OpenAI-style `image_url` content parts.
   *  Defaults to `false` so the field is strictly opt-in;
   *  an existing row that pre-dates migration 0017 will
   *  not claim vision capability. The chat pipeline
   *  refuses image attachments when the active model
   *  does not advertise vision support so a text-only
   *  LLM is never asked to interpret an image. */
  supports_vision: boolean;
};

export type EmbeddingTestResult = {
  dimension: number;
  model: string;
  preview: number[];
};

export type TestPromptRequest = {
  model: string;
  prompt: string;
  system?: string | null;
  temperature?: number | null;
  max_tokens?: number | null;
};

export async function providersProbe(): Promise<ProviderProbe[]> {
  return invoke("providers_probe");
}

export async function providersSetActive(kind: ProviderKind): Promise<void> {
  return invoke("providers_set_active", { kind });
}

export async function providersStartLmstudio(): Promise<string> {
  return invoke("providers_start_lmstudio");
}

export async function providersStopLmstudio(): Promise<string> {
  return invoke("providers_stop_lmstudio");
}

export async function modelsList(): Promise<ModelProfile[]> {
  return invoke("models_list");
}

export async function modelsUpsert(profile: ModelProfile): Promise<ModelProfile> {
  return invoke("models_upsert", { profile });
}

export async function modelsDelete(id: string): Promise<void> {
  return invoke("models_delete", { id });
}

export async function modelsSetActive(id: string): Promise<void> {
  return invoke("models_set_active", { id });
}

export async function modelsTestPrompt(request: TestPromptRequest): Promise<string> {
  return invoke("models_test_prompt", { request });
}

export async function modelsTestEmbedding(
  model: string,
  probe?: string,
): Promise<EmbeddingTestResult> {
  return invoke("models_test_embedding", {
    request: { model, probe: probe ?? null },
  });
}

export async function cancelInflight(): Promise<number> {
  return invoke("cancel_inflight");
}

export async function onTestDelta(handler: (text: string) => void): Promise<UnlistenFn> {
  return listen<string>("nexus:test.delta", (e) => handler(e.payload));
}

export async function onTestReasoning(handler: (text: string) => void): Promise<UnlistenFn> {
  return listen<string>("nexus:test.reasoning", (e) => handler(e.payload));
}

export async function onTestError(handler: (text: string) => void): Promise<UnlistenFn> {
  return listen<string>("nexus:test.error", (e) => handler(e.payload));
}

export async function onTestDone(handler: (text: string) => void): Promise<UnlistenFn> {
  return listen<string>("nexus:test.done", (e) => handler(e.payload));
}