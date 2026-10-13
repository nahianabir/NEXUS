# NEXUS — Architecture Decision Records

Each ADR captures one significant decision: context, options, choice, and consequence. ADR numbers are stable.

## ADR-0001: Tauri 2 + Rust core + React/TS frontend

**Context.** NEXUS must feel like a native desktop application on Windows 11. Options considered: Electron, Tauri, native Win32, .NET MAUI.

**Decision.** Tauri 2 with a Rust core and a React + TypeScript frontend.

**Why.**
* Smallest installer and idle memory of the three web-based options.
* Lets the team keep all sensitive logic (file I/O, agent, tools, policy) in Rust with a thin IPC surface.
* The Tauri 2 ecosystem is mature on Windows as of 2025.
* React + Vite + Tailwind gives a fast design system iteration loop and works in a WebView2.

**Consequence.** Two languages to maintain. The IPC contract must be explicitly typed; we will generate TypeScript types from Rust structs using `ts-rs`.

## ADR-0002: x86_64-pc-windows-gnu toolchain (not MSVC)

**Context.** The host has no MSVC compiler and no Windows SDK installed. Tauri can build with either MSVC or GNU.

**Decision.** Use the GNU host target `x86_64-pc-windows-gnu` and the MinGW/UCRT64 toolchain that already exists at `C:\msys64\ucrt64`.

**Why.**
* Avoids the multi-GB Visual Studio Build Tools + Windows SDK install.
* The UCRT64 toolchain is already present and tested.
* All of our chosen Rust deps build with the GNU target.

**Consequence.** We will not link against any prebuilt MSVC-only static libraries. We pin crates that historically shipped MSVC-only wheels carefully. The `windows` crate is usable on the GNU target with reduced surface area; we will only use it for a small set of APIs (notifications, app data path) that the GNU target supports.

## ADR-0003: SQLite + FTS5 + sqlite-vec as the storage stack

**Context.** NEXUS needs structured records, full-text search, and vector search, all local.

**Decision.** SQLite with the bundled `rusqlite` (FTS5 enabled at build) and `sqlite-vec` for vectors.

**Why.**
* Single file, WAL, easy to back up.
* FTS5 is well-maintained and works in-process.
* `sqlite-vec` is pure C with a clean Rust binding, supports Windows, and stores vectors as a first-class vtable.

**Alternatives considered.** LanceDB (file format, but extra crate surface), Qdrant (extra service), FAISS (native, more dependencies), pgvector (Postgres not local-only). All rejected as heavier.

**Consequence.** The vector dimension is fixed at migration time. The embedding model must be reconfigured through a migration; we will refuse to start if the configured dimension does not match the stored vectors.

## ADR-0004: LM Studio as the default local inference backend

**Context.** The user already has LM Studio installed and models loaded. The other backends (llama.cpp server, Ollama) are not installed.

**Decision.** Default backend is LM Studio over its OpenAI-compatible HTTP API. llama-server and Ollama are additional backends in later phases.

**Why.**
* Already installed; no extra download.
* The OpenAI-compatible API gives us streaming, chat templates, and a documented surface.
* LM Studio already supports Vulkan GPU offload for Intel Arc.

**Consequence.** NEXUS does not embed llama.cpp. We cannot tune a model's own sampling state machine; we rely on whatever the backend exposes. We do, however, validate capability responses (e.g. does the model claim `tools` support?) before enabling native tool calling.

## ADR-0005: Tool-calling protocol uses application-managed JSON with optional native function calling

**Context.** Qwen2.5-Coder-Instruct and Qwen3.5 have varying degrees of native tool-call reliability. The user wants guaranteed agent behavior.

**Decision.** The Rust core always issues requests through a single normalized schema: a system prompt that defines the tool catalog, a user turn, and a structured `assistant_message` that the model is asked to fill. If the active model passes a "native tool call smoke test" in the Model Manager, the request is also served through the OpenAI-compatible `tools` field; the Rust core still validates every tool call before executing.

**Why.**
* Application-managed JSON is reliable across backends.
* A validated native path is faster when available, and our tests confirm it.

**Consequence.** Every tool call is filtered through a JSON schema + permission check, regardless of whether it came from native or text. The agent never runs code it did not explicitly call.

## ADR-0006: Single heavyweight model in memory at a time

**Context.** Two 4-bit Qwen models together need ~15 GB of address space. The host has 16 GB.

**Decision.** Only one heavyweight model is loaded at a time. Switching models surfaces a warning and asks the backend to unload before the next load.

**Why.**
* The math does not fit; overshooting produces heavy swap and broken UX.

**Consequence.** The Model Manager shows a "Currently loaded" indicator and a "Switch" action that issues an unload → load pair. Embedding models are small and can coexist with the active generation model.

## ADR-0007: Permission model is four levels

**Context.** The user wants a controllable agent that is not "either fully on or fully off".

**Decision.** Four levels:
1. `readonly` — search, retrieve, summarize.
2. `workspace_write` — create or edit files inside approved workspaces.
3. `confirm` — anything that mutates state outside an approved workspace, runs commands, or sends a network request.
4. `denied` — anything outside the registered catalog.

**Why.** Mirrors the user's mental model from the spec and makes the agent inspector UI concrete.

**Consequence.** Each tool declares its required level in `tools.required_permission`. The core compares the active scope before each execution. The UI never invents a permission that was not granted.

## ADR-0008: No always-on Python sidecar

**Context.** Python is requested for optional MLflow/scientific integrations.

**Decision.** Python is launched only when a user-triggered integration needs it, with explicit JSON I/O and a deterministic teardown. The desktop app starts and is fully functional with no Python on the path.

**Why.** Avoids the "must run two services to open a chat app" failure mode.

**Consequence.** MLflow browsing, optional OCR, and HF helper scripts use a transient sidecar. The Rust core never imports Python.

## ADR-0009: Visual identity is a dark engineering console

**Context.** The spec calls for a JARVIS/FRIDAY-inspired atmosphere without copying the films.

**Decision.** Dark engineering console: charcoal + near-black surfaces, ice-blue primary, restrained violet for selected state, amber for warning, restrained blur on panels, grid details in the AI visualization canvas. No neon, no constant pulsing.

**Why.** A research workbench should feel like an instrument panel, not a game HUD. Color is reserved for status.

**Consequence.** Reduced motion and an optional light theme are tracked but not part of the first release.

## ADR-0010: Schema migrations are forward-only and explicit

**Context.** A local app that loses user data on a bad upgrade is unusable.

**Decision.** Each migration is a separate file `migrations/0001_core.sql` …, with explicit `BEGIN`/`COMMIT`, an `INSERT INTO schema_migrations`, and a code-side check at boot.

**Why.** Predictable. Easy to back up before. Easy to roll forward.

**Consequence.** We never write `DROP COLUMN` in an auto-migration. The boot path explicitly refuses to start if the recorded `schema_migrations` version is greater than the highest migration the binary knows about.
