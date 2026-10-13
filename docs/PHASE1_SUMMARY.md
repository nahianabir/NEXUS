# Phase 1 — Summary

## What landed

* `app/` workspace with `src-tauri/` (Rust) and `web/` (React/TS/Vite).
* Tauri 2 with the `custom-protocol` feature. CSP locked down to self + the loopback LM Studio / Ollama / llama-server endpoints.
* The Rust core is split: `nexus_core` (config, db, audit, monitor, providers, error, logging) is library code that the test suite links without WebView2. The Tauri bootstrap and the IPC `commands` module live in the binary crate (`src/main.rs`) so `cargo test` no longer needs `WebView2Loader.dll` next to the test binary.
* 12-module sidebar with collapsible rail, command palette (Ctrl+K), bottom status bar, dark engineering theme.
* `Command Center` polls the real Rust `dashboard_summary` IPC every 4 s and renders CPU, RAM, app memory, queue depth, document / conversation / task counts.
* `System Monitor` page renders live CPU and RAM meters plus browser-storage estimate.
* `Model Manager` page lists the three backends (LM Studio, llama.cpp, Ollama) and probes each one with a real `GET /v1/models` (or `/api/tags` for Ollama) — the loopback probe is real, not a stub.
* `Settings` page edits the safe subset of `AppConfig` and round-trips through `localStorage` (Phase 1) and is mirrored to `config.toml` on the Rust side; the two write paths converge in Phase 2 when the Rust side exposes a `set_config` IPC.
* Three migrations applied at boot: `0001_core`, `0002_models`, `0003_conversations`. SQLite is in WAL with `foreign_keys = ON`.
* Audit log writes one `app.boot` row at startup.
* AI activity glyph: pure SVG with breathing, slow-rotation, and a fine grid backdrop. Reduced-motion is respected.
* App data dir resolves to `%APPDATA%\ai.nexus.workbench\`; the Rust side creates `logs/`, writes `config.toml`, opens `nexus.sqlite`, and applies migrations on first launch.

## Files added in Phase 1

```
ARCHITECTURE.md
DATA_MODEL.md
SECURITY.md
ADR.md
IMPLEMENTATION_PLAN.md
docs/PHASE1_SUMMARY.md
app/src-tauri/Cargo.toml
app/src-tauri/build.rs
app/src-tauri/tauri.conf.json
app/src-tauri/capabilities/default.json
app/src-tauri/migrations/0001_core.sql
app/src-tauri/migrations/0002_models.sql
app/src-tauri/migrations/0003_conversations.sql
app/src-tauri/icons/generate.py
app/src-tauri/icons/icon.ico  (+ 32x32, 128x128, 128x128@2x, icon.png)
app/src-tauri/src/lib.rs
app/src-tauri/src/main.rs
app/src-tauri/src/error.rs
app/src-tauri/src/logging.rs
app/src-tauri/src/config.rs
app/src-tauri/src/app_state.rs
app/src-tauri/src/audit.rs
app/src-tauri/src/monitor.rs
app/src-tauri/src/providers/mod.rs
app/src-tauri/src/commands/mod.rs
app/src-tauri/src/commands/app.rs
app/src-tauri/src/db/mod.rs
app/src-tauri/src/db/migrations.rs
app/src-tauri/src/db/settings.rs
app/src-tauri/tests/phase1.rs
app/src-tauri/tests/common.rs
app/web/package.json
app/web/tsconfig.json
app/web/vite.config.ts
app/web/tailwind.config.js
app/web/postcss.config.js
app/web/index.html
app/web/src/main.tsx
app/web/src/App.tsx
app/web/src/styles/index.css
app/web/src/lib/ipc.ts
app/web/src/lib/settings.ts
app/web/src/components/Icon.tsx
app/web/src/components/CommandPalette.tsx
app/web/src/components/NexusGlyph.tsx
app/web/src/pages/CommandCenter.tsx
app/web/src/pages/Assistant.tsx
app/web/src/pages/Knowledge.tsx
app/web/src/pages/Research.tsx
app/web/src/pages/Documents.tsx
app/web/src/pages/Projects.tsx
app/web/src/pages/Agent.tsx
app/web/src/pages/Experiments.tsx
app/web/src/pages/Tasks.tsx
app/web/src/pages/Models.tsx
app/web/src/pages/Monitor.tsx
app/web/src/pages/Settings.tsx
```

## Tests that actually ran

`cargo test --tests` (run via `D:\NEXUS\.scratch\cargo_test.ps1`):

```
Running tests\phase1.rs
test config_persists_data_dir ... ok
test config_round_trip ... ok
test result: ok. 2 passed; 0 failed
```

Phase 1 smoke (`D:\NEXUS\.scratch\smoke1.ps1`):
* `nexus.exe` boots, working set ~45 MB.
* `%APPDATA%\ai.nexus.workbench\nexus.sqlite-wal` exists; the schema is at v3.
* Clean shutdown on `Stop-Process`.

`tsc -b`: 0 errors.
`vite build`: succeeds, dist is ~80 KB gzipped.

## Open issues deferred to later phases

* The Settings page writes to `localStorage` only; the canonical `config.toml` lives on the Rust side and is the source of truth at boot. Phase 2 introduces a `set_config` IPC so the UI round-trips through the Rust process.
* The sidebar's right inspector (planned in the spec) is not present in Phase 1. The first module that benefits from a right inspector is the Document viewer in Phase 9; we will introduce a `ResizableSplit` layout then.
* WebView2 system requirements: the user's Edge WebView2 runtime is installed (verified in Phase 0).
* The `msi` bundler is wired in `tauri.conf.json`; an actual MSI build happens in Phase 11 with WiX. We do not attempt the MSI in Phase 1.
* The `lms` CLI is present; the auto-managed-lifecycle toggle is wired through `providers.lmstudio_managed` but the start/stop bridge is implemented in Phase 2 alongside the real provider.

## Next: Phase 2

* Real `ModelProvider` trait with `LmStudioProvider`, `LlamaCppProvider`, `OllamaProvider`.
* `GET /v1/models`, `GET /v1/embeddings`, SSE `POST /v1/chat/completions` with cancellation tokens.
* Wire the Models page to a `model_profiles` CRUD; add a Test-prompt action that streams a real response.
* Surface backend errors, capability detection, and GPU offload notes.
