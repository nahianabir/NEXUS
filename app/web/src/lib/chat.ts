// AI Assistant IPC. The web side mirrors the `nexus_core::conversations`
// types and adds the streaming event types.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Conversation = {
  id: string;
  project_id: string | null;
  title: string;
  model_profile_id: string | null;
  system_prompt: string | null;
  archived: boolean;
  created_at: number;
  updated_at: number;
};

export type Message = {
  id: string;
  conversation_id: string;
  role: string;
  content: string;
  status: string;
  token_count: number | null;
  latency_ms: number | null;
  model_profile_id: string | null;
  created_at: number;
};

export type ChatDeltaEvent = {
  conversation_id: string;
  message_id: string;
  text: string;
  kind: "token" | "reasoning" | "error" | "done" | "cancelled";
  finish_reason: string | null;
  usage: { prompt_tokens: number | null; completion_tokens: number | null; total_tokens: number | null } | null;
  /** Wall-clock millis of the moment the request was dispatched. */
  dispatched_at_ms?: number | null;
  /** Wall-clock millis of the moment the first token arrived. */
  first_token_at_ms?: number | null;
  /**
   * Stable request id (ULID) for the in-flight chat
   * request. Phase P4 — Bug L. The frontend uses this to
   * ignore late events from a stale request, to recover
   * the in-flight state after a navigation, and to
   * target the Stop control at a specific request.
   */
  request_id?: string | null;
};

export type ChatCitation = {
  index: number;
  chunk_id: string;
  document_id: string;
  document_title: string;
  document_path: string;
  score: number;
  source: string;
  text: string;
};

export type ChatCitationsEvent = {
  conversation_id: string;
  message_id_user: string;
  citations: ChatCitation[];
  used_vector: boolean;
};

export async function listConversations(archived = false): Promise<Conversation[]> {
  return invoke("chat_list_conversations", { archived });
}
export async function createConversation(
  title?: string,
  system_prompt?: string,
  model_profile_id?: string,
  project_id?: string | null,
): Promise<Conversation> {
  return invoke("chat_create_conversation", {
    title,
    systemPrompt: system_prompt,
    modelProfileId: model_profile_id,
    projectId: project_id ?? null,
  });
}
export async function getConversation(id: string): Promise<Conversation> {
  return invoke("chat_get_conversation", { id });
}
export async function renameConversation(id: string, title: string): Promise<void> {
  return invoke("chat_rename_conversation", { id, title });
}
export async function archiveConversation(id: string, archived: boolean): Promise<void> {
  return invoke("chat_archive_conversation", { id, archived });
}
export async function deleteConversation(id: string): Promise<void> {
  return invoke("chat_delete_conversation", { id });
}
export async function listMessages(conversationId: string, limit?: number): Promise<Message[]> {
  return invoke("chat_list_messages", { conversationId, limit });
}
export async function setSystemPrompt(id: string, systemPrompt: string | null): Promise<void> {
  return invoke("chat_set_system_prompt", { id, systemPrompt });
}
export type ChatSendOptions = {
  useVault?: boolean;
  vaultLimit?: number;
};

export type ChatSendAttachment = {
  filename: string;
  kind: string;
  mime_type: string;
  text: string;
  truncated: boolean;
};

export type ChatSendInput = {
  text: string;
  modelProfileId?: string;
  options?: ChatSendOptions;
  attachments?: ChatSendAttachment[];
};

export async function chatSend(
  conversationId: string,
  textOrInput: string | ChatSendInput,
  modelProfileId?: string,
  options?: ChatSendOptions,
): Promise<string> {
  // New shape: pass a single object with optional attachments.
  if (typeof textOrInput === "object") {
    return invoke("chat_send", {
      request: {
        conversation_id: conversationId,
        text: textOrInput.text,
        model_profile_id: textOrInput.modelProfileId,
        use_vault: textOrInput.options?.useVault ?? false,
        vault_limit: textOrInput.options?.vaultLimit ?? null,
        attachments: textOrInput.attachments ?? [],
      },
    });
  }
  // Legacy shape: positional args.
  return invoke("chat_send", {
    request: {
      conversation_id: conversationId,
      text: textOrInput,
      model_profile_id: modelProfileId,
      use_vault: options?.useVault ?? false,
      vault_limit: options?.vaultLimit ?? null,
    },
  });
}

export async function onChatDelta(handler: (e: ChatDeltaEvent) => void): Promise<UnlistenFn> {
  return listen<ChatDeltaEvent>("nexus:chat.delta", (e) => handler(e.payload));
}
export async function onChatReasoning(handler: (e: ChatDeltaEvent) => void): Promise<UnlistenFn> {
  return listen<ChatDeltaEvent>("nexus:chat.reasoning", (e) => handler(e.payload));
}
export async function onChatCitations(handler: (e: ChatCitationsEvent) => void): Promise<UnlistenFn> {
  return listen<ChatCitationsEvent>("nexus:chat.citations", (e) => handler(e.payload));
}
export type ChatTitleUpdatedEvent = {
  conversation_id: string;
  title: string;
  from_model: boolean;
};
export async function onChatTitleUpdated(handler: (e: ChatTitleUpdatedEvent) => void): Promise<UnlistenFn> {
  return listen<ChatTitleUpdatedEvent>("nexus:chat.title_updated", (e) => handler(e.payload));
}

// ----------------------------- Phase P4 -----------------------------
//
// Bug E + L. The chat request registry is the source of
// truth for "is anything in flight right now?". The UI
// queries it on mount (and on every nav return) so it
// can rehydrate the running state.

export type ActiveRequestSnapshot = {
  request_id: string;
  kind: string;
  conversation_id: string | null;
  message_id: string | null;
};

export async function chatRequestListActive(): Promise<ActiveRequestSnapshot[]> {
  return invoke("chat_request_list_active");
}

export async function chatRequestCancel(requestId: string): Promise<boolean> {
  return invoke("chat_request_cancel", { requestId });
}

// ----------------------------- Memory save -----------------------------
//
// Bug F. The chat pipeline intercepts explicit
// "save to your memory that X" requests and returns a
// `SaveResult` synchronously without calling the model.
// The frontend uses this for the in-chat outcome chip
// and to refresh the Memory Center page.

export type MemorySaveResult = {
  outcome: "saved" | "updated" | "rejected" | "failed";
  memory_id: string | null;
  category: string | null;
  content: string | null;
  reason: string | null;
};

export type MemorySaveEvent = MemorySaveResult & {
  conversation_id: string;
};

export async function memorySaveExplicit(
  text: string,
  conversationId: string | null,
  projectId: string | null,
): Promise<MemorySaveResult> {
  return invoke("memory_save_explicit", {
    request: {
      text,
      conversation_id: conversationId,
      project_id: projectId,
    },
  });
}

export async function onMemorySaved(handler: (e: MemorySaveEvent) => void): Promise<UnlistenFn> {
  return listen<MemorySaveEvent>("nexus:memory.saved", (e) => handler(e.payload));
}