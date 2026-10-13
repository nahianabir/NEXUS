# NEXUS — Architecture

**Status:** Phase 0 (Environment verified, architecture defined).
**Last updated:** Phase 0.

---

## 1. Goals and constraints (recap)

NEXUS is a **local-first, desktop AI workbench** for one user on a Windows 11 PC with a Ryzen 7 7700, 16 GB RAM, and an Intel Arc A750 (4 GB VRAM). It must:

* Run as a **native desktop app** (Tauri 2).
* Use **GGUF models already on disk** (Qwen3.5-9B Q4_K_M, Qwen2.5-Coder-14B Q4_K_M) by reference, never copying them into the project directory.
* Talk to local LLM backends (LM Studio's OpenAI-compatible API, llama.cpp's server, Ollama) without depending on the cloud.
* Stay responsive with two large GGUF files in reach but **only one loaded at a time by default**.
* Treat any text produced by a model, document, or imported file as **untrusted data** and enforce policy in the Rust core.
* Avoid the cost and risk of unnecessary Python sidecars.

## 2. Verified environment (Phase 0 audit)

| Concern | Status |
| --- | --- |
| OS | Windows 11 Pro 10.0.26300 (also works on Win10) |
| CPU | AMD Ryzen 7 7700, 8C/16T |
| RAM | 16 GB DDR5 |
| GPU | Intel Arc A750, 4 GB VRAM, driver 32.0.101.9034 |
| Vulkan runtime | `C:\Windows\System32\vulkan-1.dll` present (Intel ICD) |
| Vulkan SDK | **not installed** — not required for runtime; only for building llama.cpp from source |
| OpenCL runtime | `C:\Windows\System32\OpenCL.dll` present |
| Node.js | 24.14.0 (npm 11.9.0) |
| Git | 2.51.2 |
| Rust toolchain | **not installed** — Phase 0.1 will install rustup + `x86_64-pc-windows-gnu` |
| MSVC / Windows SDK | **not installed** (only the VS installer metadata exists under `D:\Visual Studio`) — we will use the GNU target |
| MinGW / UCRT64 | `C:\msys64\ucrt64\bin\gcc.exe` present — sufficient linker for the GNU target |
| CMake | not present (not required for the GNU toolchain and pure-Rust crates) |
| Package managers | none (no winget/choco/scoop) — direct downloads only |
| LM Studio CLI (`lms`) | present at `~/.lmstudio/bin/lms.exe` — supports `lms server start` and OpenAI-compatible API |
| LM Studio bundled models | `D:\LmStudio\lmstudio-community\Qwen3.5-9B-GGUF\Qwen3.5-9B-Q4_K_M.gguf` (5.6 GB), `mmproj-Qwen3.5-9B-BF16.gguf` (922 MB, vision projector — **not used**) |
| Qwen2.5-Coder-14B | `D:\LmStudio\Qwen\Qwen2.5-Coder-14B-Instruct-GGUF\qwen2.5-coder-14b-instruct-q4_k_m.gguf` (8.99 GB) |
| Default model port (LM Studio) | 1234 |
| Port 8080 | currently bound by an unrelated process (PID 10844); NEXUS will **not** use 8080 |
| Free space | C: 8.7 GB, D: 48.2 GB, E: 25.2 GB, F: 63.9 GB |

### Consequence for the design

* **No CUDA.** Anything depending on `nvml`, `cuda`, or NVIDIA libs is excluded. llama.cpp will be used through its **OpenAI-compatible server** rather than embedded; GPU offload on the Arc A750 will use **Vulkan** at the backend level (LM Studio / llama-server Vulkan build). NEXUS itself will not embed llama.cpp.
* **GNU toolchain.** Tauri and pure-Rust deps build fine with `x86_64-pc-windows-gnu` and MinGW; the resulting binary uses the UCRT and runs on stock Windows 11.
* **Default backend = LM Studio** because it is already installed, already hosts the GGUF files, exposes an OpenAI-compatible HTTP API with streaming, and supports Vulkan GPU offload on Intel Arc. Ollama and llama.cpp are added as additional backends in later phases.

## 3. High-level architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                       Windows 11 host (user PC)                        │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                  NEXUS desktop app (Tauri 2)                     │  │
│  │                                                                  │  │
│  │   ┌─────────────────────────┐    ┌──────────────────────────┐    │  │
│  │   │  React + TypeScript     │    │  Rust core (Tokio)       │    │  │
│  │   │  Vite + Tailwind        │◄──►│  Tauri commands + events │    │  │
│  │   │  shadcn-style comps     │IPC │  Modules:                │    │  │
│  │   │  AI activity canvas     │    │   - assistant            │    │  │
│  │   └─────────────────────────┘    │   - knowledge vault      │    │  │
│  │                                  │   - research             │    │  │
│  │                                  │   - experiments          │    │  │
│  │                                  │   - projects / tasks     │    │  │
│  │                                  │   - agent + tools        │    │  │
│  │                                  │   - model manager        │    │  │
│  │                                  │   - system monitor       │    │  │
│  │                                  │   - documents            │    │  │
│  │                                  └────────┬─────────────────┘    │  │
│  │                                           │                      │  │
│  │              ┌────────────────────────────┼────────────────────┐  │  │
│  │              │                            │                    │  │  │
│  │              ▼                            ▼                    ▼  │  │
│  │   ┌──────────────────┐     ┌──────────────────┐  ┌────────────┐│  │
│  │   │ SQLite (rusqlite)│     │ sqlite-vec       │  │ Local FS   ││  │
│  │   │  FTS5 + schema   │     │  vector index    │  │ (user dirs)││  │
│  │   └──────────────────┘     └──────────────────┘  └────────────┘│  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                  │                                     │
│                                  │  HTTP (loopback only)               │
│                                  ▼                                     │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │  Local LLM backend  (LM Studio server, llama-server, or Ollama)  │  │
│  │   - Qwen3.5-9B Q4_K_M  (general assistant)                        │  │
│  │   - Qwen2.5-Coder-14B  (coding specialist)                        │  │
│  │   - Optional embedding model (nomic-embed / bge-small)            │  │
│  │   - Vulkan GPU offload (Intel Arc A750)                           │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

## 4. Process and language boundaries

* **Rust core** owns: schema/migrations, file I/O, ingestion pipelines, job queue, model provider abstraction, agent loop, tool registry, permission checks, audit log, structured logging, system metrics, command scheduling, persistence.
* **TypeScript/React** owns: presentation, layout, animation, command palette, Markdown rendering, citation UI, agent inspector UI, forms, dialogs, charts. It **does not** decide authorization.
* **Python (optional sidecar)**: appears only when MLflow import or a user-requested research script is invoked. It runs as a transient subprocess with explicit JSON I/O contracts; the sidecar is killed when the work is done. The main app **must boot and work** with Python absent.

## 5. Memory budget (16 GB host)

Target working set, with the heavier of the two 4-bit Qwen models loaded:

| Item | Budget |
| --- | --- |
| Windows + system services | ~3.0 GB |
| NEXUS desktop (Tauri/WebView2) | ~0.4 GB |
| Rust core (DB, queues, in-flight jobs) | ~0.3 GB |
| LM Studio server (idle) | ~0.3 GB |
| Qwen3.5-9B Q4_K_M loaded into RAM | ~6.0 GB |
| Embedding model (if enabled) | ~0.5–1.0 GB |
| Headroom for prompts, retrieval results, working set | ~5.0–5.5 GB |
| **Qwen2.5-Coder-14B at the same time** | **NOT supported by default** — the user must unload the 9B before switching. NEXUS exposes "Switch model" and surfaces a blocking warning if the active model cannot be released. |

Mitigations:
* Configurable context length (default 4096 for the 9B, 8192 for the 14B).
* Streaming ingestion with bounded chunk size and a semaphore.
* Pagination on long lists; virtualized tables in the UI.
* `jemalloc` is **not** used on Windows; the standard system allocator is fine.
* Background jobs are checkpointed, so a crash mid-ingestion resumes from the last completed chunk.

## 6. Concurrency model

* Single Tokio multi-thread runtime owned by the Tauri app.
* `JobQueue` with a per-priority semaphore: ingestion, embedding, reindex, agent, experiments.
* Cancellation tokens propagated through every long-running call.
* Inference is single-flight per provider; switching models awaits the prior in-flight job.

## 7. Local model provider abstraction

```rust
trait ModelProvider {
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;
    async fn capabilities(&self, model: &ModelId) -> Result<ModelCapabilities>;
    async fn stream_chat(
        &self,
        model: &ModelId,
        request: ChatRequest,
        tx: mpsc::Sender<StreamEvent>,
        cancel: CancellationToken,
    ) -> Result<ChatSummary>;
    async fn embeddings(&self, model: &EmbeddingModelId, inputs: &[String]) -> Result<Vec<Vec<f32>>>;
    async fn unload(&self) -> Result<()>;
    async fn health(&self) -> ProviderHealth;
}
```

Backends:
* `LmStudioProvider` — uses LM Studio's OpenAI-compatible `/v1/chat/completions` (SSE) and `/v1/embeddings` endpoints. Default base URL `http://127.0.0.1:1234/v1`. The `lms` CLI is used as a convenience to start/stop the server when the user opts in to "managed lifecycle"; otherwise the user runs LM Studio manually.
* `LlamaCppProvider` — speaks the same OpenAI-compatible API to `llama-server` (`llama-server -m model.gguf --port 8081 --host 127.0.0.1`).
* `OllamaProvider` — speaks Ollama's `/api/chat` and `/api/embeddings`. Detected at `http://127.0.0.1:11434`.

Provider discovery order (configurable): user choice wins; otherwise LM Studio if reachable, else Ollama, else llama-server.

## 8. Data flow for a chat turn

1. UI sends `assistant.send_message { conversation_id, text, attachments }`.
2. Core loads conversation, recent N turns, project context, and (if enabled) hybrid-retrieval hits from the vault.
3. Core deduplicates retrieved chunks, ranks them, and trims to fit the model's effective context budget.
4. Core opens a stream against the active provider; tokens flow to the UI over a Tauri event channel.
5. As tokens arrive, the UI updates incrementally. Sources, model state, and activity timeline are written to a `Message` row with citations.
6. If the user cancels, the cancellation token is forwarded; the stream is closed; partial content is preserved with a `(cancelled)` flag.

## 9. Knowledge Vault data flow

1. User adds a directory or file.
2. Discovery walks the root, skipping ignore rules, secrets, and excluded folders.
3. Each file gets a content hash; unchanged files are skipped.
4. Per-type extractors (PDF, DOCX, MD, code, notebook, CSV/XLSX, HTML) produce structured text with offsets.
5. Chunker splits text into ~500-token chunks with overlap; large files stream.
6. Each chunk is embedded with the local embedding model (or skipped if disabled).
7. SQLite stores the chunk, metadata, and FTS5 entry; `sqlite-vec` stores the vector.
8. Retrieval at query time fuses FTS5 score and vector similarity (RRF or weighted sum), with optional reranker.

## 10. Agent loop

`understand → plan → (request approval if required) → execute tool → observe → verify → next step or finalize`.

* The model is asked to emit either `tool_call` (if native tool calling is verified) or a JSON envelope in a controlled schema.
* Core parses, validates against the tool's JSON schema, checks the permission policy, and asks the user via UI when the action is above the granted permission level.
* Tools write a row to `tool_executions` with input, output, status, duration, and audit metadata.
* A reflection pass asks the model to confirm completion against the original goal; if it does not, one retry is allowed, then the loop halts.

## 11. Security boundaries

* Tauri CSP forbids remote scripts; only the bundled assets load.
* `127.0.0.1`-only HTTP for the provider API. No public bind.
* Path resolution is always performed in Rust against an allowlisted root.
* No shell strings are passed to `cmd.exe`; commands go through an allowlisted registry with explicit argv.
* Tool results and document content are never treated as instructions.
* All SQL is parameterized through `rusqlite`.
* Audit log is append-only and survives migrations.

## 12. Packaging

`cargo tauri build` produces a Windows MSI/NSIS installer and a portable `.exe`. Both run **without** a dev server. Models remain outside the installer. The first launch runs the setup wizard.
