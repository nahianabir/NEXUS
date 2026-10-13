# NEXUS 1.0.0 — Release Notes

**NEXUS is a local-first AI workbench.** Everything runs on your PC.
No cloud. No telemetry. No accounts. Your chats, notes, tasks, vault,
knowledge graph, experiments, and audit log stay on your machine in a
single SQLite database at `%APPDATA%\ai.nexus.workbench`.

This is the first public release. It is feature-complete against the
master specification: every module listed in `IMPLEMENTATION_PLAN.md`
is implemented, tested, and visible in the UI.

## Headline features

* **Project & workspace model.** Multiple workspaces, multiple projects
  per workspace. Each project carries goals, instructions, notes, and
  tasks (kanban + 14-day calendar).
* **Local chat with streaming.** Talk to LM Studio, llama.cpp, or
  Ollama. Per-conversation model profile. Per-project system prompt.
  The model router picks a profile that fits the requested role and
  context size; you can override with one click.
* **Native memory v2.** Pin facts to a project. Approve candidates
  proposed by the assistant. Every memory carries evidence, an FTS5
  mirror, and a soft-delete audit trail. Dedup is content-hash based;
  supersession is a chain you can walk.
* **Vault.** Drop a directory in; NEXUS ingests Markdown, text, code,
  JSON, CSV, and notebooks. Search with FTS5 (keywords) and
  sqlite-vec (semantic), with a per-source detail view. Export any
  Markdown table to CSV / TSV.
* **Knowledge graph.** Entities, aliases, relationships, all with
  evidence. Seed a project to materialise its entities. Merge
  duplicates; the merge records an audit. Walk the neighborhood
  (bounded) or the shortest path between two entities.
* **Research studio.** Organise papers, extract claims with category
  and confidence, build a comparison matrix (text / number / bool /
  rating), and generate a Markdown report with a click.
* **Experiment tracking.** Track runs with hyperparameters, metrics
  (JSON), code revision, and hardware. Plot a metric across runs in
  the embedded SVG chart.
* **Controlled agent.** A whitelist of tools the assistant can call
  on your behalf (vault search, knowledge-graph search, memory recall,
  project note / task add, file read / list, shell run, git status,
  settings read / write). Every tool has a permission (`always`,
  `ask`, `restricted`, `deny`). The `file` and `shell` tools are
  allowlisted from the Settings page. Every invocation is recorded.
* **Unified context builder.** The chat pulls the active project's
  context, pinned memories, FTS5 memory matches, the optional
  MemPalace backend, the vault, and the knowledge graph — all inside
  a bounded token budget. The Assistant page lets you expand the
  bundle to see what the model saw and why each item earned its
  place.
* **System monitor.** CPU, RAM, disk, and GPU sampled every three
  seconds. A virtualised audit log with one-click clear and a
  retention knob (prune on boot if you want).
* **About panel.** App version, data directory, schema version, and
  link to the docs, all on the Settings page.

## What is not in this release

* No native GPU sampling on macOS / Linux (Windows: nvml.dll
  best-effort).
* No code signing (Tauri's optional Windows signing is documented
  in `docs/BUILDING.md` but is left to the distributor).
* No cloud sync, no multi-device, no team workspace. By design.

## Installing

Download the installer (`NEXUS_1.0.0_x64-setup.exe` or the matching
`.msi`) from the release page. The installer is per-machine; it does
not require administrator rights if you pick the "Just me" option in
the NSIS dialog.

Your data directory is `%APPDATA%\ai.nexus.workbench` on Windows, with
analogous locations on macOS and Linux. `docs/BACKUP_AND_RECOVERY.md`
covers cold backup, hot backup, restore, and migration.

## First-run checklist

1. Open **Model Manager**, start LM Studio (or Llama.cpp / Ollama),
   and create at least one **General** and one **Coding** profile.
2. Open **Projects** → create a workspace and a project. Optionally
   set goals and instructions on the project — they get baked into
   the system prompt.
3. Open **Settings** → **Agent** → enter the folders the agent may
   read and the command basenames the agent may run. Leave empty to
   deny everything.
4. Open **Vault** → drop a directory in. Try a search.
5. Go to **Memory** → capture a fact, approve it, then ask the
   assistant a question in the project.

## Documentation

* `docs/USER_GUIDE.md` — end-user walkthrough.
* `docs/BUILDING.md` — developer build / test / release instructions.
* `docs/TROUBLESHOOTING.md` — common failures and fixes.
* `docs/BACKUP_AND_RECOVERY.md` — backup / restore / migration.
* `docs/MEMPALACE_INTEGRATION.md` — MemPalace wire format and the
  sidecar integration.
* `docs/PHASE1_SUMMARY.md` … `docs/PHASE_N_SUMMARY.md` — per-phase
  implementation notes, what shipped, what was tested.
* `AUDIT.md`, `ARCHITECTURE.md`, `SECURITY.md`, `DATA_MODEL.md`,
  `IMPLEMENTATION_PLAN.md` — top-level reference.

## Verified end-to-end

* `cargo test --lib --tests` — 100 tests pass, 1 ignored (MemPalace
  live). 0 failed.
* `npm --prefix app/web run build` — clean.
* `cargo tauri build` — clean, NSIS + MSI produced.
* `nexus.exe` boots, schema migrates to v11, log writes to
  `%APPDATA%\ai.nexus.workbench\logs\`.
* Live chat round-trip through LM Studio + Qwen3.5 9B Q4_K_M
  completed in <120 s, captured `reasoning_content`, received a
  `done` event, and persisted the message with `latency_ms`.
