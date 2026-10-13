// Typed IPC binding for the context inspector (Phase E).

import { invoke } from "@tauri-apps/api/core";

export type ContextSource =
  | "project_context"
  | "native_memory"
  | "mempalace_memory"
  | "vault_document"
  | "graph_relationship"
  | "graph_entity"
  | "manual_instruction";

export type ContextItem = {
  source: ContextSource;
  source_id: string;
  source_label: string | null;
  title: string | null;
  excerpt: string;
  approx_tokens: number;
  provenance: string | null;
  verification: string | null;
  score: number;
};

export type ContextBundle = {
  items: ContextItem[];
  approx_tokens: number;
  truncated: boolean;
  system_block: string;
  sources_searched: ContextSource[];
  hits_by_source: Record<string, number>;
};

export async function contextPreview(request: {
  project_id?: string | null;
  conversation_id?: string | null;
  user_message: string;
  max_tokens?: number | null;
}): Promise<ContextBundle> {
  return invoke("context_preview", {
    projectId: request.project_id ?? null,
    conversationId: request.conversation_id ?? null,
    userMessage: request.user_message,
    maxTokens: request.max_tokens ?? null,
  });
}
