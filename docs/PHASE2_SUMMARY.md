# Phase 2 — Summary

## What landed

* **Model provider abstraction** (`nexus_core::providers`) with three backends:
  * `LmStudioProvider` — OpenAI-compatible SSE on `http://127.0.0.1:1234/v1`.
  * `LlamaCppProvider` — same dialect on `http://127.0.0.1:8081/v1`.
  * `OllamaProvider` — Ollama's NDJSON dialect on `http://127.0.0.1:11434`.
* A unified `StreamEvent` channel: `token` (user-visible text), `reasoning` (chain-of-thought from models like Qwen3.5), `tool_call`, `error`, `done`, `cancelled`.
* `ProviderSet` holds the three providers and tracks the active one.
* `model_profiles` CRUD against SQLite (`migrations/0002_models.sql`):
  * `general`, `coding`, `embedding`, `reranker` roles.
  * fields: provider, model_id, gguf_path, context_len, gpu_layers, temperature, top_p, max_tokens, enabled.
  * defaults seeded only when the user explicitly creates a profile, so the boot path is never blocked by a missing backend.
* Frontend (`Models.tsx`):
  * Three live backend reachability probes (LM Studio, Ollama, llama.cpp).
  * "Start LM Studio" / "Stop" buttons that shell out to `lms` (`which::which` lookup).
  * Profile CRUD UI with three columns (General / Coding / Embedding) and a "New profile" form.
  * Streaming "Test prompt" panel that consumes `nexus:test.delta`, `nexus:test.reasoning`, `nexus:test.error`, `nexus:test.done` Tauri events.
  * Inline Stop button that calls `cancel_inflight`, which cancels the in-flight `CancellationToken` and propagates into the SSE future.
* `commands/models.rs`:
  * `providers_probe` — async, returns health + model list per backend.
  * `providers_set_active` — switches the active provider and writes an `audit_events` row.
  * `providers_start_lmstudio` / `providers_stop_lmstudio` — manage the local server lifecycle.
  * `models_list` / `models_upsert` / `models_delete` / `models_set_active`.
  * `models_test_prompt` — opens a stream against the active provider and forwards each `StreamEvent` to the UI as a Tauri event; respects a 180 s ceiling and a cancellation token.
* SSE / NDJSON parser (`providers/stream.rs`) handles `data: ...\n\n` (SSE) and bare JSON lines (NDJSON) and preserves partial frames across chunks.

## Verified end-to-end

* `cargo build` — clean, 0 warnings.
* `cargo test` — **11 tests pass**:
  * Phase 1: `config_round_trip`, `config_persists_data_dir`.
  * Phase 2: `frame_parser_sse_data`, `frame_parser_sse_done`, `frame_parser_partial_buffer`, `frame_parser_ndjson`, `lmstudio_provider_health_unreachable`, `ollama_provider_normalises_v1_suffix`, `provider_set_default_active`, `config_round_trip_unchanged`.
  * **Live**: `live_lmstudio_stream` against a real LM Studio with Qwen3.5 9B Q4_K_M. The test asserted the stream completed in <120 s, captured `reasoning_content` (191 chars), received a `done` event, and the `ChatSummary.finish_reason` was `length` (model used all 60 tokens on reasoning — expected behaviour).
* `tsc -b` — 0 errors.
* `vite build` — succeeds.
* `nexus.exe` boots, `data_dir=%APPDATA%\ai.nexus.workbench`, working set ~45 MB, clean shutdown.

## Important runtime finding

* Qwen3.5 9B returns a separate `reasoning_content` field in the SSE delta before any visible `content`. With the model's default sampling this can consume the entire `max_tokens` budget. Phase 2 captures `reasoning_content` as a `reasoning` stream event so the UI can show it on the activity timeline without mixing it into the answer. The user can raise `max_tokens` (Model Manager → New profile) to ensure the model has room for both reasoning and an answer.
* LM Studio returns two identifiers for the same model after multiple `lms load` calls (`qwen/qwen3.5-9b` and `qwen/qwen3.5-9b:2`). The list-models endpoint surfaces both. The user picks which one to use in the profile's `model_id` field.

## Memory budget

* With LM Studio server running and Qwen3.5 9B Q4_K_M loaded: ~5.6 GB resident for the model, ~6.5 GB peak working set including OS cache. NEXUS itself stayed at ~45 MB. This is well within the 16 GB host budget; loading the 14B coder in addition would exceed it and is blocked by the "one heavyweight model at a time" rule (enforced by the user choosing when to switch).

## Open issues deferred

* The test-prompt handler uses a fixed 60 ms emit interval to throttle events to the UI; if the user wants higher granularity, it will be configurable in Phase 10.
* The "Test prompt" panel uses a raw text area; in Phase 3 the chat UI gets the same streaming path with Markdown rendering.
* Native tool calling is plumbed through `StreamEvent::tool_call` and the `ChatRequest.tools` field but the registry is not yet built. Phase 8 wires the registry.

## Next: Phase 3

* Conversation list, rename / archive / delete, search.
* Streaming chat UI with Markdown + code highlighting (`shiki`).
* Per-conversation system prompt, model switcher, message sources (citations as opaque tokens for now; Phase 4 fills them).
* Activity timeline (thinking / retrieving / generating / cancelled / error) reused from the Phase 2 stream.
* Persistence: `conversations` + `messages` + `message_sources` tables (migrations already in place).
