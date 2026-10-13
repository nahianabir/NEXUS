# Phase 3 — Summary

## What landed

* **`nexus_core::conversations`** — CRUD for conversations and messages (migrations `0003_conversations.sql`, tables `conversations`, `messages`, `message_sources`).
* **`commands::chat`** — Tauri commands for the AI Assistant:
  * `chat_list_conversations(archived)` — list visible or archived conversations.
  * `chat_create_conversation(title, system_prompt, model_profile_id)`.
  * `chat_get_conversation(id)`.
  * `chat_rename_conversation(id, title)`.
  * `chat_archive_conversation(id, archived)`.
  * `chat_delete_conversation(id)`.
  * `chat_list_messages(conversation_id, limit)`.
  * `chat_set_system_prompt(id, system_prompt)`.
  * `chat_send(request)` — resolves the active profile, builds a `ChatRequest` from the conversation history + system prompt + the new user turn, opens the SSE stream against the active provider, persists the assistant message incrementally, and emits `nexus:chat.delta` (token) and `nexus:chat.reasoning` events.
* Streaming events: the same `StreamEvent` model from Phase 2 is forwarded as Tauri events. The web side listens and updates the in-memory message list as tokens arrive.
* Cancellation: `cancel_inflight` cancels all in-flight `CancellationToken`s, which propagates into the SSE future and closes the HTTP response.
* Profile resolution order: explicit override → conversation's `model_profile_id` → app's active model → first enabled `general` profile. If none is available, the command returns `NotFound` and the UI surfaces "no model profile available; create one in Model Manager".

## Frontend (`Assistant.tsx`)

* **Conversation list** (left): rename / archive / delete per row, with a "New" button.
* **Message thread** (centre): Markdown rendering via the new `MarkdownView` component, with code-block copy buttons, streaming placeholders, status badges (`streaming`, `cancelled`, `error`, `complete`), and a latency display.
* **Composer** (bottom): textarea with `Enter` to send, `Shift+Enter` for newline, a `Stop` button that replaces `Send` while a stream is in progress.
* **Activity timeline** (right): emits `Sending`, `Reasoning`, `Done · finish_reason · tokens`, `Cancelled`, `Error` events with timestamps. Each entry uses a coloured dot to distinguish the variant.
* **Reasoning** (collapsed under the message): the most recent 2 KB of model reasoning, shown on demand so it does not pollute the conversation but remains auditable.
* **Active model selector** (sidebar): picks which profile to use for the next send. If unset, the conversation's default or the app's active model is used.
* **System prompt editor** (per conversation): opens a `prompt()` dialog (Phase 5 will replace this with an in-app editor).

## Markdown rendering

* `components/MarkdownView.tsx` — a 100-line Markdown parser/renderer that covers paragraphs, H1-H3, bold, italic, inline code, fenced code blocks (with `Copy`), unordered lists, and blockquotes. No external dependency.
* `useStableMarkdown` — debounces the rendered text during streaming so the UI stays responsive.

## Tests that ran

* `tests/phase3.rs` (3 new tests):
  * `conversation_crud_round_trip` — create, add messages, rename, archive, delete.
  * `update_message_status_and_content` — incremental updates during streaming.
  * `profile_resolution_picks_general_first` — profile resolution picks the seeded `general` profile.
* `tests/live_chat.rs` — end-to-end through the live LM Studio with Qwen3.5 9B, 10.09 s, 658 chars of reasoning, `done` event received, summary returned.
* `tests/live_lmstudio.rs` (already in Phase 2) — still passes.

Total: **14 unit/integration tests + 2 live tests, all passing.**

## Smoke

* `nexus.exe` boots, working set ~48 MB.
* `tsc -b` and `vite build` succeed.

## Limitations explicit in the UI

* "Phase 4 adds grounded citations and the Knowledge Vault" is shown in the right inspector.
* Source-citation strip is not yet rendered; `message_sources` rows will be created in Phase 4.
* No retry/regenerate yet (Phase 8 will add it as an agent action).
