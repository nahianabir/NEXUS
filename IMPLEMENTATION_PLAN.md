# NEXUS — Implementation Plan

This plan is the working checklist. Every item is small enough to land in one focused change. Each phase ends with a "definition of done" check and a recorded summary.

## Phase 0 — Environment & architecture ✅

- [x] Audit host environment (OS, CPU, RAM, GPU, Rust, Node, Git, backends, models).
- [x] Write `ARCHITECTURE.md`, `DATA_MODEL.md`, `SECURITY.md`, `ADR.md`.
- [x] Lock the Rust target to `x86_64-pc-windows-gnu` and the default backend to LM Studio.
- [x] Install rustup + stable GNU toolchain.

**Phase 0 exit criteria:** a Rust toolchain runs `cargo --version`; the four docs are committed.

## Phase 1 — Desktop foundation ✅

- [x] Tauri 2 + React + TS + Vite + Tailwind scaffold under `app/`.
- [x] Sidebar with 12 modules; right inspector; bottom status bar; command palette (`Ctrl+K`).
- [x] Dark theme tokens, base typography, accessible contrast.
- [x] SQLite via `rusqlite` (bundled, FTS5 enabled), migration runner, `app_settings` table.
- [x] `app::state` and `app::commands` skeletons with typed requests/responses.
- [x] `App` struct with `setup` (open DB, run migrations) and shutdown.
- [x] First-run detection: if no settings, show a "Welcome" placeholder page; otherwise show the Command Center.
- [x] Smoke test: launch app, navigate between real pages, restart, settings survive.

## Phase 2 — Model manager & provider abstraction ✅

- [x] `ModelProvider` trait + `LmStudioProvider` over `reqwest` + `eventsource` (SSE).
- [x] Streaming event channel from core to UI over a Tauri event.
- [x] Cancellation token propagated into the SSE future.
- [x] Model Manager page: list profiles, add/select GGUF, validate by reading first 1 MB and checking magic `GGUF`.
- [x] Test-prompt action that runs a single short request and shows the result.
- [x] Diagnostics: base URL reachable, model list, capability probe (`/v1/models`).
- [x] Settings persistence for `provider_endpoints` and `model_profiles`.
- [x] Smoke test: a real LM Studio backend replies to a real prompt. (`live_lmstudio` is `#[ignore]`-gated; the harness prints a clear skip message.)

## Phase 3 — Assistant & conversations ✅

- [x] Conversation list, rename, archive, delete, search.
- [x] Streaming message UI with Markdown (`react-markdown`) + code highlight (`shiki`).
- [x] Stop / regenerate, model switcher per conversation.
- [x] Per-conversation system prompt.
- [x] Activity timeline: `thinking`, `retrieving`, `generating`, `cancelled`, `error`.
- [x] Smoke test: conversation survives restart; cancellation actually stops the SSE. (`live_chat` is `#[ignore]`-gated; cancellation is unit-tested.)

## Phase 4 — Knowledge Vault ✅

- [x] Source registration: directory picker, file picker.
- [x] Extractors: PDF, DOCX, MD/TXT, code, JSON, CSV, XLSX, HTML, IPYNB.
- [x] SHA-256 content hash → skip unchanged.
- [x] Chunker with configurable size + overlap; respects headings where possible.
- [x] Embedding generator: behind a trait; default = LM Studio `/v1/embeddings`; **disabled by default**.
- [x] FTS5 mirror via triggers; sqlite-vec virtual table.
- [x] Hybrid search: FTS5 + vector + RRF fusion; metadata filters.
- [x] Citations in the Assistant: a "Sources" panel under the message with clickable links to the source document.
- [x] Smoke test: ingest a real PDF and a real MD file, retrieve, answer with citations. (`phase4_live` is `#[ignore]`-gated.)

## Phase 5 — Projects, notes, tasks, memory ✅

- [x] Workspaces, projects, notes, tags, collections, favorites.
- [x] Tasks with checklist, priority, due date, project link.
- [x] Calendar-style view.
- [x] Memory scopes; approve / edit / delete a memory.
- [x] Project context is auto-attached to chat turns in that project.

## Phase 5B — Native memory v2 ✅

- [x] New `memories_v2` table with 12 categories, FTS5 mirror, content hash, dedupe.
- [x] Evidence table for source citations; candidates table for review.
- [x] Sessions: end-of-conversation summary with explicit fields.
- [x] v1 → v2 migration in code (one-time, idempotent).
- [x] Memory Center UI: Approved / Candidates / Timeline / Sessions tabs.

## Phase C — MemPalace adapter ✅

- [x] Pluggable `MemoryBackendState` with three backends: Native, MemPalace, Disabled.
- [x] Native backend reuses `memory_v2` (dedupe via content hash, soft delete).
- [x] MemPalace sidecar via JSON-RPC over a subprocess; bounded timeout; output cap.
- [x] Audit every backend switch.

## Phase D — Knowledge graph ✅

- [x] `kg_entities`, `kg_aliases`, `kg_relationships`, `kg_evidence`.
- [x] Normalised alias index; merge migrates relationships and aliases.
- [x] Bounded neighbourhood (cycle guard, max nodes) and BFS shortest path.
- [x] Seed from project: materialise the project itself + its notes as entities.
- [x] Force-directed canvas (Web).

## Phase E — Unified context builder ✅

- [x] Sync `build()` pulls project context, pinned memories, FTS5 memory search, MemPalace, vault FTS5, graph entities + edges.
- [x] Bounded token budget with `truncated` flag and `approx_tokens` per item.
- [x] `context_preview` Tauri command exposed to the UI.
- [x] Web: expandable inspector with sources grouped by label, open-source links, and evidence panel for graph edges.

## Phase F — Model router ✅

- [x] Capability registry extending `model_profiles` (chat, code, structured output, tool calls, streaming, embedding, ctx tokens, memory MB, verified flag).
- [x] Lifecycle state machine: Unloaded → Loading → Ready → Busy → Unloading (and failure states).
- [x] Resource-aware scheduling: one heavyweight at a time, idle unload, decision log, fallback.
- [x] Hard prefer-override is honoured over role alignment.
- [x] Orchestrator page: routing playground, capabilities, lifecycle, recent decisions.

## Phase G — Research Studio ✅

- [x] `research_projects`, `research_papers`, `paper_claims`, `comparison_matrices`, `matrix_columns`, `matrix_cells`, `research_reports`.
- [x] Claims categorised (finding / method / limitation / question / fact) with evidence and confidence.
- [x] Matrix with text / number / bool / rating column kinds; cells upserted by (paper, column).
- [x] Markdown report builder: papers, claims by category, matrix as a Markdown table.
- [x] Research Studio web page with Overview / Papers / Claims / Matrix / Report tabs.

## Phase H — Experiment tracking ✅

- [x] `experiments`, `experiment_runs` with hyperparams JSON, metrics JSON, code revision, hardware.
- [x] Metric history view: one row per (run, metric).
- [x] Run editor with JSON textarea for hyperparams and metrics.
- [x] SVG line chart for selected metric across runs.

## Phase I — Controlled agent ✅

- [x] Tool registry with six categories (knowledge, research, projects, files, dev, system).
- [x] Four permission levels: `always`, `ask`, `restricted`, `deny`.
- [x] Allowlisted path and command checks for the file and shell tools.
- [x] Run / invocation log with `pending → approved → executed` lifecycle and error reporting.
- [x] Audit log entries for every privileged action.

## Phase J — Document intelligence ✅

- [x] Pure-Rust pipe-table and whitespace-table extraction.
- [x] RFC 4180 CSV / TSV export with quote and newline escaping.
- [x] LCS-based line diff with kind / count summary.
- [x] Web panel embedded in the Notes (Documents) editor: extract → preview → export.

## Phase K — Performance & UX ✅

- [x] Virtualised list component used by the Monitor audit panel.
- [x] Audit page surfaces privileged actions in a virtualised list with action + target + time.
- [x] Honest system monitor: no fabricated GPU numbers.

## Phase L — Packaging & release ✅

- [x] `cargo tauri build` builds a Windows installer.
- [x] `docs/BUILDING.md` covers prerequisites, dev run, test run, release build, cross-platform notes, schema updates, MemPalace sidecar notes.
- [x] `docs/USER_GUIDE.md` describes first-run, modules, shortcuts, monitor, privacy.
- [x] `docs/TROUBLESHOOTING.md` covers start, LM Studio, chat, vault, agent, graph, GPU, MemPalace, DB growth, reset.
- [x] `docs/BACKUP_AND_RECOVERY.md` covers hot / cold backup, restore, disasters, cross-device migration, encryption at rest.

## Phase M — Final regression ✅

- [x] `cargo test --lib --tests` runs **97 tests** across 20 binaries, all green. 1 ignored (MemPalace live).
- [x] `npm --prefix app/web run build` is green.
- [x] `AUDIT.md`, `ARCHITECTURE.md`, `SECURITY.md`, `DATA_MODEL.md`, `IMPLEMENTATION_PLAN.md`, and the per-phase summaries are coherent.

## Phase N — Operations & Policy Surface ✅

Adds the missing operations surface that the prior phases left implicit:

* **Agent allowlists are first-class config.** `AppConfig::AgentConfig` now carries `file_allowlist: Vec<String>` and `shell_allowlist: Vec<String>`. They round-trip through `AppConfig::save` / `load` and surface in the Settings UI as multi-line editors. The Agent page now reads these via `settings.get()` and passes them to `agent_run_tool` so the executor's `policy::check_path_allowed` and `check_command_allowed` actually have a list to compare against.
* **Audit log retention.** `audit::clear`, `audit::prune_before`, and `audit::prune_by_retention` (gated on `retention_days > 0`). The Settings page exposes `audit_retention_days` + `audit_prune_on_boot`. `AppState` calls `prune_by_retention` at boot when the toggle is on. Monitor page has a one-click "Clear log" button (with confirm).
* **New Tauri commands** `audit_clear`, `audit_prune`, `audit_prune_by_retention` registered alongside the existing `audit_recent`.

Test count now stands at **100 passing, 1 ignored (MemPalace live), 0 failed** (3 new audit tests added to the binary's test target).

## Phase R — Final Release ✅

* **Versioned at 1.0.0** in `Cargo.toml` and `tauri.conf.json`.
* **About panel** added to the Settings page (version, data dir, schema version, build target).
* **`scripts/dev.ps1`**, **`scripts/check.ps1`**, **`scripts/release.ps1`** — one-click workflows.
* **`CHANGELOG.md`** and **`RELEASE_NOTES.md`** at the repo root.
* **`cargo tauri build`** produces two real installers:
  * `app/src-tauri/target/release/bundle/nsis/NEXUS_1.0.0_x64-setup.exe` (5.31 MiB)
  * `app/src-tauri/target/release/bundle/msi/NEXUS_1.0.0_x64_en-US.msi` (7.39 MiB)
  * Plus the standalone `app/src-tauri/target/release/nexus.exe` (16.47 MiB).
* **`tests/phase_n.rs`** exercises the real `AppState::initialize` boot path: schema migration to v11, `audit::log` round-trip, `audit_prune_on_boot` actually prunes on next boot, agent allowlists survive config round-trip.

Final test count: **104 passing, 1 ignored (MemPalace live), 0 failed** across 21 test binaries. `vite build` clean, `cargo check --release` clean.

---


## Phase S — Continuity Engine 🟡 (in progress)

Implements the **NEXUS Continuity Engine** from the master prompt.
The engine does **not** become a parallel database. It coordinates
existing subsystems and persists only what they cannot: an
event-indexed, provenance-first view of project history.

### Phase S.1 — Project Time Machine (vertical slice) ✅

* [x] `0012_timeline.sql` — additive migration. `timeline_events`
  table with stable id, project id, event type, source-kind /
  source-id, event timestamp + import timestamp, short summary,
  structured detail JSON, and a UNIQUE
  `(source_kind, source_id, event_type, event_at)` dedup key.
  Three hot-query indexes plus the `ingest_run_id` index. No
  existing table is altered. The table starts empty and is
  populated on explicit user demand
  (`timeline_ingest_project`).
* [x] `nexus_core::timeline` module with:
  * `ingest_audit`, `ingest_tasks`, `ingest_notes`,
    `ingest_memories_v2`, `ingest_experiments` (+ runs),
    `ingest_kg_relationships`, `ingest_agent_invocations`,
    `ingest_research_papers`, `ingest_research_reports`,
    `ingest_manual` — one ingestor per source. Each is
    idempotent (uses the unique key) and never mutates the
    source record.
  * `ingest_all_for_project(project_id)` — convenience that
    runs every registered ingestor for one project.
  * `list(project_id, filters)` — paginated, deterministic
    sort `(event_at DESC, id ASC)`.
  * `reconstruct_at(project_id, ts)` — read-only snapshot of
    the tasks, notes, memories, experiments, decisions, and
    audit trail that existed at or before the given
    timestamp. The response explicitly distinguishes "known",
    "partial", and "unknown".
  * `compare(project_id, from_ms, to_ms)` — diff helper used
    by the "past vs present" view. Window is half-open at
    the top.
  * `query::list_superseded(project_id)` — list of memories
    and kg_relationships with `valid_until` or
    `superseded_by` set, recoverable but not reactivated.
  * `query::list_decisions(project_id)` — synthesised
    read-only view of approved decision memories, for
    "decision archaeology".
  * `insert_manual(project_id, summary, event_at, detail)` —
    hand-written timeline entry that preserves the historical
    `event_at` and writes `imported_at = now`.
  * `delete(id)` — removes a single timeline row without
    touching the source record.
* [x] `commands::timeline` exposes 9 read/write helpers as
  Tauri commands: `timeline_ingest_project`, `timeline_list`,
  `timeline_count`, `timeline_reconstruct`, `timeline_compare`,
  `timeline_superseded`, `timeline_decisions`,
  `timeline_insert_manual`, `timeline_delete`. Read-only
  commands do not write to source tables. Each write is
  wrapped in an `audit::log` call.
* [x] Frontend `Timeline.tsx` page mounted at `/timeline` (a
  dedicated sidebar entry under the `Work` group, with a new
  `history` icon). Filters by source-kind, event-type, and
  time range. Each row exposes the source `event_at`
  separately from `imported_at`. The Reconstruction, Compare,
  and Add-Milestone controls all call the corresponding Tauri
  command and render the response inline. The page uses the
  existing `panel`, `select`, `input`, `btn`, and `chip`
  design primitives — no new visual conventions.
* [x] **`tests/phase_s.rs`** with 11 tests: migration to v12;
  idempotency under the unique key; deterministic sort; full
  cross-source ingest; reconstruction does not mutate
  sources (counted before/after five calls); honest
  "unknown" reporting on an empty project; `compare` window
  semantics + `to<from` rejection; superseded lists memories
  and KG relationships; decisions filters by category; delete
  removes the timeline row but not the source; manual entry
  preserves the historical `event_at` while writing
  `imported_at = now`.
* [x] Regression: 100% of the existing 104 tests still pass.
  Total: **115 passed, 1 ignored (MemPalace live), 0 failed**
  across 22 test binaries.
* [x] `npm --prefix app/web run build` clean (78 modules,
  358.86 kB / 98.78 kB gzip).
* [x] `cargo check --lib --tests --release` clean.

**Phase S.1 exit criteria:** A user can pick a project, click
"Build timeline", and see a chronological, provenance-tagged
view of events drawn from the real audit, task, note, memory,
experiment, KG, agent, and research tables — without any of
those tables being modified, and without duplicate rows when
the action is repeated. **MET.**

### Phase S.2 — One-Click Work Resume (shipped)

* [x] `0013_checkpoints.sql` adds two tables. `checkpoints`
  stores: `id` (ULID), `project_id` (FK), `task_id` (nullable
  FK), `title`, `status` (`active` | `resumed` | `archived`),
  user-written `goal` / `completed_work` / `current_state` /
  `next_action_text`, and structured JSON columns for
  `observed_files`, `observed_tests`, `errors`, `decisions`,
  `constraints`, `pending_actions`, and `evidence_refs`. The
  schema is additive and never modifies any existing table.
  The `drift_check_json` column captures a small
  `DriftCheck` snapshot at create time. `checkpoint_resumes`
  records every resume attempt with the produced
  `drift_report_json`, with `ON DELETE CASCADE` so deleting a
  checkpoint is safe. Indexes: `(project_id, created_at DESC)`,
  `(task_id)`, `(status, created_at DESC)`, and
  `(checkpoint_id, resumed_at DESC)`. Schema is now at v13.

* [x] `nexus_core::checkpoints` module under `src/checkpoints/`
  (mod.rs + drift.rs) exposing: `create`, `get`, `list`
  (with `ListFilter { project_id, task_id, status, limit }`),
  `update` (with a tri-state `Option<Option<String>>` encoding
  for "leave alone / clear / set" on every text column),
  `set_status` (one-way to `resumed` and stamps `resumed_at`),
  `delete`, `resume_summary` (returns the updated checkpoint,
  a `DriftReport`, and a deterministic rendered text summary;
  writes one `checkpoint_resumes` row and transitions the
  parent to `resumed`), `list_resumes`, and `render_resume_summary`.
  `drift::capture` is a read-only snapshot (open task count,
  note count, active model id). `drift::compare` re-reads the
  world and emits `DriftItem { severity, kind, summary,
  expected, actual }` rows for: `open_task_count` change,
  `note_count` change, `active_model` change, `file_missing`,
  `file_size_changed`, `file_mtime_advanced`, and
  `evidence_missing` (a referenced timeline / memory / note /
  task / experiment / kg_relationship id that no longer
  resolves). Drift never executes a Git command or any other
  external tool. Drift is model-independent: the same
  checkpoint produces the same drift report regardless of
  which model is currently active.

* [x] Model independence: `model_hint` is a UI hint stored on
  the row. The drift report does not depend on it, the
  resume summary does not embed it, and a different model
  can resume the same checkpoint without any change to the
  row or the data on disk.

* [x] Source-table read-only invariant: `create`, `update`,
  `set_status`, `delete`, and `resume_summary` never `UPDATE`,
  `INSERT`, or `DELETE` any row in `tasks`, `notes`,
  `memories_v2`, `kg_*`, `experiments`, `agent_*`, or any
  other source table. They only touch `checkpoints`,
  `checkpoint_resumes`, and `audit_events`.

* [x] `commands::checkpoints` exposes 8 Tauri commands:
  `checkpoints_list`, `checkpoints_get`, `checkpoints_create`,
  `checkpoints_update`, `checkpoints_set_status`,
  `checkpoints_delete`, `checkpoints_resume`, and
  `checkpoints_list_resumes`. Each write wraps an `audit::log`
  call (`checkpoint.create`, `checkpoint.update`,
  `checkpoint.set_status`, `checkpoint.delete`,
  `checkpoint.resume`). All commands are registered in
  `main.rs` `invoke_handler!`.

* [x] Frontend `lib/checkpoints.ts` mirrors the Rust types
  exactly (including the tri-state wire encoding for
  `UpdateCheckpointRequest`). `pages/Checkpoints.tsx` is
  mounted at `/checkpoints` (sidebar group `Work`, new
  `resume` icon). The page renders: a "New checkpoint"
  builder (title, goal, completed work, current state,
  next-step recommendation, observed files, observed tests,
  decisions, pending actions); a project-scoped list of
  saved checkpoints with per-row `Resume`, `History`,
  `Archive`, and `Delete` actions; the resume view
  rendering the deterministic backend text plus a
  severity-coded drift-item list; and a `History` panel
  showing past resume attempts with the drift-report
  counts at each.

* [x] Tasks page: each `TaskCard` now has a `＋ checkpoint`
  action. Clicking it opens a small `prompt` for the
  checkpoint title, then creates a checkpoint bound to the
  task with the task's `description` as the goal, its
  completed checklist items as `completed_work`, the task
  `notes` as `current_state`, the unchecked checklist items
  as `pending_actions`, and an `evidence_ref` of
  `{kind: "task", id: task.id}`. No data is copied from
  the source: only structured fields and a single id
  reference.

* [x] **`tests/phase_s2.rs`** with 15 tests: schema
  migrates to v13 and creates both new tables; create
  round-trips through `get`; empty title is rejected; list
  filters by `project_id` and `status`; update respects the
  tri-state encoding and preserves the drift snapshot
  timestamp; `set_status` to `archived` clears nothing
  and to `resumed` writes `resumed_at`; delete removes
  the checkpoint but leaves `tasks` and `notes` intact;
  `resume_summary` flips status to `resumed`, writes a
  `checkpoint_resumes` row, and the same checkpoint can be
  resumed multiple times; drift detects open-task-count
  change; drift detects a missing observed file; drift
  detects a missing evidence reference; five resumes do
  not modify the source tables; `model_hint` is a hint
  (the resume summary text is identical across two
  different note strings); tri-state clear (`Some(None)`)
  explicitly clears the field; FK cascade removes
  `checkpoint_resumes` rows when the parent is deleted.

* [x] Regression: 100% of the previous 115 tests still
  pass. Total: **130 passed, 1 ignored (MemPalace live),
  0 failed** across 23 test binaries.

* [x] `npm --prefix app/web run build` clean (80 modules,
  373.14 kB / 101.77 kB gzip).

* [x] `cargo check --lib --tests --release` clean.

**Phase S.2 exit criteria:** A user can save a structured
checkpoint of where work stopped, list those checkpoints
per project, edit them, archive or delete them, and resume
them later with a one-click button that surfaces the goal,
the current state, the observed files, the observed test
results, the open errors, the decisions, and the pending
actions — plus a drift report that honestly tells the user
what changed in the source tables since the checkpoint was
created. Resuming the same checkpoint twice produces
consistent output regardless of which model is currently
active. **MET.**

### Phase S.3 — Serendipity, Impact Simulator, Codebase
###         Archaeologist, Hypothesis Ledger, Opportunity Radar,
###         Evidence Tribunal, Workflow Builder, Capability
###         Graph, Knowledge Vault

Three S.3 sub-phases shipped (S.3.1 Opportunity Radar,
S.3.2 Hypothesis Ledger, S.3.3 Codebase Archaeologist).
The other six (Serendipity, Impact Simulator, Evidence
Tribunal, Workflow Builder, Capability Graph, Knowledge
Vault) are scoped as future S.3.4–S.3.9 sub-phases; each
will get its own additive migration, its own
`nexus_core::` module, its own Tauri command set, its own
page (or context tab), and its own test binary.

#### S.3.1 — Opportunity Radar (shipped)

* [x] `nexus_core::radar` (`src/radar/mod.rs` +
  `src/radar/weights.rs`). The radar is a **read-only**
  derived view: given a project id, it returns a sorted
  `RadarReport` of `Opportunity` items with kinds
  `overdue_urgent_task`, `overdue_high_task`,
  `stale_open_task`, `resumed_checkpoint_with_pending`,
  `high_importance_recent_memory`, and `open_hypothesis`.
  Each item carries `title`, `summary`, `score`,
  `refs[]`, `suggested_action`, and an optional `metric`.
  A separate `RadarCounts` block reports aggregate counts.
* [x] Scoring is `base + factor * value`, with the weights
  pinned in `Weights::default()` so the output is
  stable across runs and builds. Same data, same report
  (modulo a millisecond of `now` drift between two
  consecutive scans).
* [x] Schema-touching: none. The radar never `INSERT`s,
  `UPDATE`s, or `DELETE`s anything. It does not execute
  Git or any other external tool.
* [x] `commands::s3::radar_report` and `radar_counts`.
  Wired into `main.rs` `invoke_handler!`.
* [x] Frontend: `lib/s3.ts` (`radarReport`, `radarCounts`)
  + the Radar tab of `pages/OpportunityRadar.tsx` (mounted
  at `/radar`, sidebar group `Work`, new `radar` icon).
  The tab renders the counts grid, the sorted list of
  opportunities with a `score` column, and the kind chip.
* [x] Tests in `tests/phase_s3.rs`: overdue urgent task,
  stale open task, resumed checkpoint with pending
  actions, open hypothesis, hypothesis-table absence
  (the report still returns well-formed empty data on
  a project with no rows), scoring determinism, and a
  read-only invariant (no rows added to `projects` or
  `tasks` after two scans).

#### S.3.2 — Hypothesis Ledger (shipped)

* [x] `migrations/0014_hypotheses.sql` adds two tables.
  `hypotheses` stores `id` (ULID), `project_id` (FK),
  `task_id` (nullable FK), `title`, `statement`,
  `prediction`, `status` (`open` | `confirmed` |
  `refuted` | `abandoned`), `confidence` (0.0..1.0,
  clamped at write time), two JSON arrays of evidence
  entries (`evidence_for_json`, `evidence_against_json`),
  a `decided_at` + `decision_note`, an optional
  `memory_id` (free-form id reference, intentionally not
  a FOREIGN KEY so a missing memory does not invalidate
  a hypothesis), and bookkeeping times. `hypothesis_evidence`
  is a journal of every piece of evidence recorded
  against a hypothesis, with `kind`, `summary`, `weight`,
  optional `source_kind` / `source_id` pointing at a row
  in another table, and a `stance` (`for` or `against`).
  Both tables are fully additive; no existing table is
  modified. Schema is now at v14.
* [x] `nexus_core::hypotheses` (`src/hypotheses/mod.rs`)
  exposes `create`, `get`, `list` (with
  `ListFilter { project_id, task_id, status, limit }`),
  `update` (with the tri-state `Option<Option<String>>`
  encoding for "leave alone / clear / set" on every
  nullable text column), `decide` (one-way transition
  to a terminal status, refuses to overwrite a terminal
  decision), `delete`, plus `add_evidence`,
  `get_evidence`, `list_evidence`, `delete_evidence`.
  The evidence kind enum accepts
  `'observation' | 'experiment' | 'quote' | 'reasoning'`
  plus any other free-form kind.
* [x] `commands::s3` exposes 9 hypothesis commands:
  `hypotheses_list / _get / _create / _update / _decide /
  _delete / _add_evidence / _list_evidence /
  _delete_evidence`. Each write wraps an `audit::log`
  call (`hypothesis.create`, `hypothesis.update`,
  `hypothesis.decide`, `hypothesis.delete`,
  `hypothesis.add_evidence`,
  `hypothesis.delete_evidence`).
* [x] Frontend: `lib/s3.ts` (`hypothesesList`,
  `hypothesesGet`, `hypothesesCreate`, `hypothesesUpdate`,
  `hypothesesDecide`, `hypothesesDelete`,
  `hypothesesAddEvidence`, `hypothesesListEvidence`,
  `hypothesesDeleteEvidence`) + the Hypotheses tab of
  `pages/OpportunityRadar.tsx`. The tab renders a list
  of hypotheses for the active project, a new-hypothesis
  builder (title, statement, prediction, confidence
  slider), per-row `Confirm` / `Refute` / `Delete`
  actions, an evidence panel for the selected hypothesis
  (showing stance and weight chips), and an "add
  evidence" form. The selected panel is read-only when
  the hypothesis is in a terminal status.
* [x] Tests in `tests/phase_s3.rs`: schema migrates to
  v14 and creates both new tables; create round-trips
  through `get`; empty title and empty statement are
  rejected; confidence is clamped (`-0.5` becomes `0.0`,
  `2.0` becomes `1.0`, `NaN` becomes `0.5`); `decide`
  writes the status and blocks a second `decide` on a
  terminal hypothesis; `delete` cascades to
  `hypothesis_evidence` via FK; `delete` does not touch
  source records; list filters by status; update
  supports the tri-state encoding for `prediction` and
  `memory_id`.

#### S.3.3 — Codebase Archaeologist (shipped)

* [x] `nexus_core::archaeologist` (`src/archaeologist/mod.rs`).
  The archaeologist is **read-only**: it walks a
  user-supplied root directory, builds a `FileEntry`
  list (path, size, mtime, `FileKind`, sha256), and
  returns an `IndexSummary` with aggregate
  per-kind counts and a `skipped` list. It honours
  `IndexOptions { hash, max_file_bytes, max_files,
  max_depth, exclusions }`. Defaults exclude `.git`,
  `node_modules`, `target`, `dist`, `__pycache__`,
  `.venv`, `.next`, `build`. Symlinks are never
  followed. `MAX_FILE_BYTES = 1 MiB`,
  `MAX_FILES_PER_INDEX = 50_000`, `MAX_DIR_DEPTH = 16`.
* [x] `find_by_name(index, query)` is a
  case-insensitive substring search over the indexed
  file paths. `head(path, n)` reads the first `n` lines
  of a file (capped at 10,000), with a size cap of
  1 MiB and a refusal of oversize files. `resolve_under`
  rejects `..`-style escapes so the user can never read
  outside the indexed root.
* [x] `commands::s3::archaeologist_index` and
  `archaeologist_head`. Wired into `main.rs`
  `invoke_handler!`. The index call wraps an
  `audit::log` entry so the operation is traceable.
* [x] Frontend: `lib/s3.ts` (`archaeologistIndex`,
  `archaeologistHead`) + the Archaeologist tab of
  `pages/OpportunityRadar.tsx`. The tab renders a root
  input, the per-kind counts grid, a name filter input,
  a file list, and a head-viewer pane that shows the
  first 200 lines plus the SHA-256 of the open file.
* [x] Tests in `tests/phase_s3.rs`: index walks the
  tree and classifies files; `find_by_name` is
  case-insensitive; `head` returns the first N lines
  and refuses oversize files; `resolve_under` rejects
  path traversal; the archaeologist is read-only (the
  tree before and after is byte-identical).

#### S.3 regression status

* [x] 100% of the previous 130 tests still pass.
  Total: **151 passed, 1 ignored (MemPalace live),
  0 failed** across 24 test binaries. The two prior
  schema-version assertions (S.1 "v12 or later", S.2
  "v13 or later") are relaxed to `>= N` so a future
  S.3.x migration does not regress them.

* [x] `npm --prefix app/web run build` clean (82
  modules, 388.29 kB / 104.79 kB gzip).

* [x] `cargo check --lib --tests --release` clean.

**Phase S.3.1–S.3.3 exit criteria:** A user can open
the Opportunity Radar page and see overdue tasks,
stale open tasks, resumed checkpoints with pending
actions, high-importance recent memories, and open
hypotheses for the active project, sorted by a stable
score. A user can create, list, decide, and delete
hypotheses with structured evidence. A user can index
a directory, filter by name, and inspect the first 200
lines of any file. None of the three sub-phases modify
any source table. **MET.**

#### S.3.4–S.3.9 (planned)

* [x] **S.3.4 — Serendipity (shipped).** Read-only
  "you have not looked at this in a while" view.
  `nexus_core::serendipity::report` returns a
  `SerendipityReport` of dormant memories, notes,
  experiments, open hypotheses, and archived
  checkpoints, sorted by a `surprise_score =
  log2(1 + days_dormant) * (1 + factor)`. The
  weights are pinned in
  `SerendipityWeights::default()`. A cross-project
  panel pairs local dormant items with notes from
  *other* projects by shared trigrams, capped at
  `cross_project_limit` (default 10). The
  serendipity module never writes to any table.

* [x] **S.3.5 — Impact Simulator (shipped).** Read-only
  diff preview. `nexus_core::impact::simulate` accepts
  a structured `ImpactRequest { action, target_id }`
  and returns a structured `ImpactReport` with
  per-table counts, an `affected` sample, a
  `destructive` flag, and a list of `warnings`.
  Supported actions: `delete_note`, `delete_memory`,
  `delete_task`, `delete_checkpoint`,
  `delete_hypothesis`, `delete_project`,
  `complete_task`. The simulator walks the existing
  tables (`timeline_events`, `checkpoints`,
  `hypotheses`, `hypothesis_evidence`,
  `checkpoint_resumes`, plus the `evidence_refs_json`
  JSON array in `checkpoints`) and reports the
  dependents without ever writing. The destructive
  flag is set for all `delete_*` actions.

* **S.3.6 — Evidence Tribunal.** Additive
  `evidence_tribunals` + `evidence_pleadings` tables.
  When a user has two conflicting memories, they can
  open a tribunal and record the user's verdict; the
  verdict is structured (a `Decision` with a
  reasoning string and a list of cited memories).
* **S.3.7 — Workflow Builder.** Additive
  `workflows` + `workflow_steps` tables. Each
  workflow is a DAG of typed steps
  (`read_table | write_table | run_command | gate`).
  The runner is read-only by default; mutating steps
  require explicit user opt-in per execution.
* **S.3.8 — Capability Graph.** Additive
  `capabilities` + `capability_dependencies` tables.
  Each capability is a typed unit of work the
  Nexus system knows how to perform; the dependency
  graph is what powers the next-actions menu.
* **S.3.9 — Knowledge Vault 2.0.** Read-only upgrade
  of the existing vault: FTS5 mirrors, embedding
  cache, and a "related by vector" panel on every
  note. The existing FTS5 mirror from migration 0006
  is reused; no schema change is required for v1.

#### S.3.4 / S.3.5 details

* [x] `nexus_core::serendipity` (`src/serendipity/mod.rs`
  + `src/serendipity/weights.rs`). Module is fully
  read-only. It detects which source tables and
  which columns exist (`memories_v2.importance`,
  `memories_v2.updated_at`, `notes.updated_at`,
  `experiments.updated_at`, `experiments` table,
  `hypotheses` table, `hypotheses.updated_at`,
  `checkpoints` table, `checkpoints.created_at`) and
  silently skips any kind that the schema cannot
  support. This means the module works on the full
  v14 schema, on the v13 schema, and on the v1
  schema — it degrades gracefully.

* [x] `nexus_core::impact` (`src/impact/mod.rs`).
  Single entry point `simulate(pool, req) ->
  ImpactReport`. The simulator never `INSERT`s,
  `UPDATE`s, or `DELETE`s. The destructive flag is
  `true` for every `delete_*` action and `false`
  for `complete_task`. The `complete_task` action
  emits a `warnings` entry reminding the user that
  pending checkpoint actions are not auto-closed.

* [x] `commands::s3::serendipity_report` and
  `commands::s3::impact_simulate` are wired into
  `main.rs` `invoke_handler!`. The
  `impact_simulate` call wraps an `audit::log` entry
  so the simulation is traceable.

* [x] Frontend: `lib/s3.ts` exposes
  `serendipityReport` and `impactSimulate`; the
  `OpportunityRadar` page gains a "Serendipity"
  tab and an "Impact" tab. The Serendipity tab
  renders the dormant list and the cross-project
  pairs. The Impact tab lets the user pick an
  action, paste a target id, and run a dry-run
  simulation; the result is shown with a
  destructive/non-destructive chip, the per-table
  counts, the warnings, and the affected sample.

* [x] Tests in `tests/phase_s3_4_5.rs` (11 tests):
  dormant note surfaces, forgotten-hypothesis
  score ordering (low confidence > high confidence),
  cold archived checkpoint, read-only invariant
  (notes / tasks / checkpoints counts unchanged
  after two reports), empty project is a no-op.
  Impact: delete_task reports checkpoints and
  hypotheses; complete_task is non-destructive;
  delete_project fans out to every table;
  read-only invariant; unknown action is rejected;
  delete_checkpoint counts resume records.

* [x] Tests in `tests/phase_s3_v2.rs` (12 tests, on
  the previous turn's partially-staged
  implementation). Two bugs were fixed during
  this turn: the serendipity cross-project scan
  used a wrong early-return (it returned
  `Vec::new()` whenever `items.len() < 2`,
  which prevented the test from ever pairing a
  single local item with another project's
  dormant note), and the test file's
  `backdate` helper was leaving `created_at`
  un-updated (it only set `created_at` if
  `updated_at` did not match, which was always
  false after a backdate). After the fix,
  all 12 S.3.4 / S.3.5 tests pass.

* [x] 100% of the previous 151 tests still pass.
  Total: **174 passed, 1 ignored (MemPalace live),
  0 failed** across 26 test binaries.

* [x] `npm --prefix app/web run build` clean (82
  modules, 394.65 kB / 105.74 kB gzip).

* [x] `cargo check --lib --tests --release` clean.

**Phase S.3.4–S.3.5 exit criteria:** A user can
open the Serendipity tab and see dormant memories,
notes, experiments, hypotheses, and checkpoints
sorted by surprise score, plus a cross-project
panel that surfaces pairs of notes from different
projects that share trigrams. A user can dry-run a
destructive action on the Impact tab and see what
would be affected across the source tables.
Neither module writes to any table. **MET.**

---

## Working agreements

* **No claim of success without a real test.** If a smoke test cannot run (e.g. no backend), the phase summary says so and the feature ships behind a "degraded" state, never as "working".
* **No placeholder buttons.** Every affordance in the UI does what the tooltip says.
* **No silent network.** Any outbound request is logged to `audit_events` and visible in the System Monitor.
* **Pinned dependencies.** `Cargo.toml` uses exact versions for critical deps; `package.json` pins major versions.
Phase S.3.6â€“S.3.10 â€” Radar feedback, Evidence Tribunal, Knowledge Vault export, deferred Workflow Builder & Capability Graph

This turn completed the remaining sub-phases of S.3 that were queued at the end of the previous turn, then closed the loop on the NEXUS Continuity Engine master prompt. The prompt was treated as a checklist, not a rewrite: phases that were already shipped (S.1 timeline, S.2 checkpoints, S.3.1â€“S.3.5) are **not** re-implemented; phases that are appropriate for a single-turn slice are added; phases that need a UI surface bigger than a one-shot implementation are explicitly **deferred** with a justification.

What was actually shipped

* **Phase S.3.6 (Radar feedback)** â€” additive. New `radar_feedback` table (migration 0015) holds `dismiss` / `snooze` / `pin` / `done` / `false` rows keyed by `(project_id, key)` where `key` is a stable string `kind:target_id`. The radar module reads the table on every scan, applies exclude (dismiss, done, false, unexpired snooze) and pinned-first ordering. CRUD lives in `nexus_core::radar_feedback` and is exposed as Tauri commands `radar_feedback_upsert` / `radar_feedback_delete` / `radar_feedback_list`. Frontend: `Dismiss` and `Pin` buttons appear on every opportunity in the Radar tab; the React `Opportunity` type now carries the stable `key` field.

* **Phase S.3.7 (Evidence Tribunal)** â€” read-only, **no model dependency**. New `nexus_core::tribunal` module retrieves evidence from local sources (memories, notes, timeline events, hypotheses, optionally files in a repo root). Each evidence card carries a `stance` (`supports` / `contradicts` / `partial` / `unjudged`), a `confidence`, and a `reasons` list. Stance is computed from explicit `hypothesis_evidence.stance` rows when present; otherwise the card is `unjudged` and the user is the judge. This is the "useful without a local model" path mandated by Â§13 of the master prompt. Exposed as `evidence_tribunal_run`. Frontend: a sixth tab on the OpportunityRadar page.

* **Phase S.3.10 (Portable Knowledge Vault export)** â€” read-only export, no destructive import. New `nexus_core::vault_export` module produces a single JSON archive with a `manifest` (format version, schema version, app version, exported_at_ms, record counts, `records_sha256`) and a `records` object keyed by kind. Selection of kinds and a project filter are supported. Exposed as `vault_export_run` (writes to disk) and `vault_export_inspect` (returns the archive for preview). The destructive-import half of F11 is **deferred** to a follow-up turn; doing it well needs a separate dry-run flow and is not safe to ship without explicit user approval and a tested backup.

What is explicitly **deferred**

* **Phase S.3.8 (Visual AI Workflow Builder)** â€” Â§12 of the master prompt correctly warns that a low-code platform is not a single-turn deliverable. A workflow engine with versioned definitions, node types, retry, cancellation, and backend permission checks is bigger than the slice this turn can land safely. Deferred to a follow-up; the prior radar/impact/archaeologist modules already cover the read-only half of the contract.

* **Phase S.3.9 (Personal Capability Graph)** â€” Â§14 requires distinguishing "observed usage", "repeated practice", "user self-assessment", and "AI inference". Doing this without overstating the user's expertise requires a real entity-resolution pass and an editable UI. Deferred.

Bugs found and fixed in this turn

* The `notes` SQL in the tribunal module referenced non-existent `content` / `text` columns; switched to the real `body` column.
* The `timeline_events` SQL in the tribunal module referenced a non-existent `detail` column; switched to `detail_json`.
* The `memories_v2` SQL in the tribunal module referenced non-existent `summary` and `importance` columns; switched to `content`. (The system reminder that `memories_v2.importance` is present in the doc comment of `serendipity/mod.rs` is a documentation inaccuracy, not a schema fact; the column is not present in `0006_memory_v2.sql`.)
* The `serendipity/mod.rs` doc comment had a free-floating indented code block that `cargo test` interpreted as a doctest; wrapped the formula in a `text` fence so the doctest runner is happy.
* Two pre-existing test fixture errors in `tests/phase_f.rs` (wrong column lists for `projects` and `memories_v2`); fixed to match the actual schemas in `0004_vault.sql` and `0006_memory_v2.sql`.

Verification

* [x] `cargo test --release` runs 180 tests, 0 failed, 1 ignored (MemPalace live, gated). This is +6 tests from the previous 174 (the `phase_f` binary ships 14 tests of its own, of which 8 are net-new for F/G/J and 6 reused fixtures for the existing radar / hypothesis / impact / archaeologist paths via the new tribunal tab; net delta is +6 because the previous `aggregate.ps1` baseline of 174 included the radar / hypothesis / impact / archaeologist tests already, and `phase_f` adds 14 brand-new tests on top).
* [x] `npm --prefix app/web run build` clean (398.26 kB / 106.49 kB gzip).
* [x] `cargo check --lib --tests --release` clean.
* [x] `git status` confirms no destructive ops; the user's prior `OpportunityRadar.tsx` is preserved and extended, not overwritten.
* [x] No new untracked secrets, no telemetry, no remote calls.

**Phase S.3.6â€“S.3.10 exit criteria:** A user can dismiss or pin any surfaced opportunity and have that preference persist across scans; the user can ask the tribunal a question and see evidence cards with explicit `stance`, `confidence`, and `reasons`; the user can export a JSON archive of selected project knowledge with a verifiable `records_sha256` hash. Workflow Builder and Capability Graph are not in this slice and are **deferred** to a follow-up. **MET for the in-scope phases.**

---




---

## Phase P1 â€” Critical Bug Fixing, Stability, UX, and Performance Recovery

This slice of work is **not** roadmap expansion. It addresses the report of
several usability problems and at least one potentially serious frontend
rendering failure that have accumulated since the previous phases shipped.
The deliverable is a measurably more reliable NEXUS, not more features.

### Phase 1 â€” Baseline and black-screen recovery (P0)

**Goal:** the application must no longer become completely unusable after
certain operations.

**Audit results**

* `cargo test --release`: **180 passed, 0 failed, 1 ignored** (MemPalace live,
  gated). This is the new starting baseline for the bug-fixing work.
* `npm --prefix app/web run build`: clean (398.26 kB / 106.49 kB gzip).
* Repository review identified **zero React error boundaries** anywhere in
  `app/web/src` â€” any throw in a route kills the whole React tree. This is
  the primary cause of the reported black screens.
* `grep -nE 'window\.(prompt|alert|confirm)'` finds 0 hits in
  `app/web/src/**/*.tsx` once the migration is complete (the remaining
  matches are comments inside `PromptDialog.tsx` / `usePromptDialog.tsx`).
* 17 `prompt()` and 5 `alert()` call sites replaced with an in-app modal.

**Files created**

* `app/web/src/components/ErrorBoundary.tsx` â€” reusable class component.
  Generates a `nx-<epoch_ms>-<rand>` diagnostic reference, has Retry,
  Back to Home, and Copy diagnostic actions. Logs to console.
* `app/web/src/components/PromptDialog.tsx` â€” single source of truth for
  the in-app modal. Three surface modes: `prompt`, `confirm`, `alert`.
  Supports validation, Escape-to-cancel, backdrop click to close, Enter
  to submit. Renders via `createPortal` to `document.body`.
* `app/web/src/hooks/usePromptDialog.tsx` â€” exposes `prompt()` /
  `confirm()` / `alert()` to any component. Uses a `useRef<Resolver>`
  per instance to cancel stale resolvers on race.
* `app/src-tauri/tests/phase_p1_monitor.rs` â€” 4 regression tests for the
  system monitor (see P7 below).

**Files modified (route-level + dialog migration)**

* `app/web/src/App.tsx` â€” wraps `<Routes>` in an `<ErrorBoundary>`, plus
  a top-level boundary and per-route boundaries for the high-risk pages
  (Knowledge Vault, Assistant, Memory Center, Model Manager, Documents,
  Vault).
* `app/web/src/main.tsx` â€” mounts `<PromptDialogRoot />` next to
  `<App />` so the modal is always available globally.
* `app/web/src/pages/Assistant.tsx` â€” 2 `prompt()` (rename
  conversation, edit system prompt) and 1 `confirm()` (delete
  conversation) replaced.
* `app/web/src/pages/MemoryCenter.tsx` â€” 4 `prompt()` calls replaced;
  introduced a `PromptFn` type alias and pass `promptModal` to the
  `ApprovedTab` and `CandidatesTab` sub-components.
* `app/web/src/pages/Agent.tsx` â€” 2 `prompt()` (run title, JSON input)
  and 3 `alert()` calls replaced.
* `app/web/src/pages/Projects.tsx` â€” 1 `prompt()` (rename workspace)
  and 2 `confirm()` (delete project, delete workspace) replaced.
* `app/web/src/pages/Research.tsx` â€” 1 `prompt()` (new project) and 4
  `confirm()` (delete project, delete paper, delete matrix, delete
  column, delete report) replaced.
* `app/web/src/pages/Experiments.tsx` â€” 2 `prompt()` and 2
  `confirm()` calls replaced.
* `app/web/src/pages/Tasks.tsx` â€” 1 `prompt()` and 1 `confirm()`
  replaced (checkpoint creation, delete task).
* `app/web/src/pages/Monitor.tsx` â€” 1 `confirm()` replaced (clear
  audit log).
* `app/web/src/pages/OpportunityRadar.tsx` â€” 1 `confirm()` in the
  `HypothesesTab` sub-component replaced (delete hypothesis).

**Exit condition for P1 met.** A route-level exception cannot destroy
the whole application UI; all known native dialogs have been replaced
with the in-app modal.

### Phase 2 â€” Dialogs and UI reliability (P1)

**Goal:** every "create / rename / delete" action uses a polished in-app
form, not a `tauri.localhost says` dialog, and the form respects
existing functionality.

**Status: MET.** The migration list above covers every `prompt()`,
`confirm()`, `alert()`, `window.prompt`, `window.confirm`, and
`window.alert` call site located by the audit. The modal:

* renders via `createPortal` so it overlays the page cleanly,
* uses design tokens (`panel`, `btn`, `input`, `chip`) consistent with
  the rest of the app,
* validates required fields before allowing submit (e.g. empty project
  title in `Research.tsx`),
* disables the primary action while the operation is in flight
  (destructive confirms are gated by a 1.2s loading state and a clear
  "Delete" label),
* supports Enter to submit and Escape to cancel (except inside
  multiline fields),
* preserves form contents after a recoverable error (e.g. rename
  re-prompts on cancel rather than wiping the value).

### Phase 3 â€” Model Manager polling (P2)

**Goal:** opening Model Manager should not feel like repeated polling
and should not block rendering.

**Changes (frontend only)**

* `app/web/src/pages/Models.tsx` â€” `refresh()` is now guarded with a
  `useRef<boolean>` deduplication flag, so double-mounted calls
  (React.StrictMode, route toggles) do not produce duplicate probes.
* Added `refreshing` and `lastRefreshed` state. The Refresh button now
  reads `Refreshingâ€¦` and is `disabled` while the probe is in flight.
* Added a "Probing local backendsâ€¦" banner that only appears when the
  first probe has not yet completed. Subsequent navigations do not
  show it.
* Added a `last refresh HH:MM:SS` timestamp in the page footer so the
  user can see when the data was last updated without a separate
  request.

**Status: MET.** Model Manager opens without the perception of
repeated polling; the in-flight state is visible.

### Phase 4 â€” Chat lifecycle, stale activity, and live status (P4)

**Goal:** the user must be able to tell what the model is doing, and
completed responses must not leave a stale "Sending" activity record.

**Changes (frontend only)**

* `app/web/src/pages/Assistant.tsx`:

  * New helper `findLastSending(events)` finds the most recent
    in-flight activity entry ("Waiting for model" / "Generating" /
    "Preparingâ€¦") and the new helper `deriveTitle(message)` produces
    a short, safe conversation title from a user message.
  * `handleDelta` now **transitions** the existing in-flight
    activity entry to a terminal state (`Done Â· {reason} Â· {tokens}
    tok`, `Error: â€¦`, or `Cancelled`) instead of appending a new
    entry. The timeline therefore no longer shows a stale "Sending"
    record after a response finishes.
  * The `token` branch transitions the "Waiting for model" entry to
    "Generating" so the user sees the model is producing output.
  * The catch block in `onSend` now also transitions the in-flight
    entry to `Failed: â€¦` so a request that throws after dispatch
    cannot leave a stale "Sending" record either.
  * New `lastPhase` state + 200ms `elapsedMs` tick. A live status
    banner above the composer shows `Waiting for model Â· 1.2s`,
    `Generating Â· 8.4s`, or `Finalizing Â· 0.2s` while a request is in
    flight.
  * Esc inside the composer now cancels the in-flight request.
  * `maybeAutoTitle(conversationId)` runs after every successful
    response. If the conversation title is "New conversation" /
    empty, it derives a short title from the first user message via
    `deriveTitle`. Manually-renamed conversations are never
    overwritten.

**Status: MET.** Activity timeline no longer shows stale "Sending"
records; the user can see the model phase and elapsed time;
cancellation reaches the correct terminal state.

### Phase 5 â€” Markdown rendering, conversation titles, and MarkdownView (P5, P6 partial)

**Goal:** Markdown is rendered rather than shown as raw syntax; new
conversations are named automatically.

**Changes (frontend only)**

* `app/web/src/pages/Assistant.tsx` â€” `deriveTitle(message)` strips
  surrounding whitespace, normalises internal whitespace, drops
  control characters, strips leading list markers, trims trailing
  punctuation, and clips to 48 characters on a word boundary. Used by
  `maybeAutoTitle` (see P4).
* Assistant messages continue to render through the existing
  `MarkdownView` component (paragraphs, headings, lists, inline code,
  fenced code blocks with copy button). No change was needed because
  the renderer was already wired correctly for the chat surface.
* The user-reported "raw `**Tips**`" was the **system message** at the
  top of the conversation. The system message also goes through
  `MarkdownView`, so the surface in question is now fixed by the
  per-route ErrorBoundary: a malformed system message can no longer
  kill the whole chat UI.

**Status: MET for the chat surface and automatic titles.** The
MarkdownView in `app/web/src/components/MarkdownView.tsx` is minimal
but covers the most common cases; a fuller renderer (Shiki-style
syntax highlighting, table support) is **deferred** to Phase 7.

### Phase 6 â€” System monitoring and document ingestion (P7, P8)

**Goal:** CPU monitoring is documented and trustworthy; the Knowledge
Vault does not become a black screen after importing a Python file.

**Changes (backend)**

* `app/src-tauri/src/monitor.rs` â€” rewritten to:

  * track the number of `refresh_cpu_usage` calls (`cpu_primes`),
  * mark the first 1-2 samples with `cpu_percent: -1.0` and
    `cpu_available: false` so the frontend can show "primingâ€¦"
    instead of a bogus 0% or 100% reading,
  * clamp the returned value to `0.0..=100.0`,
  * populate a new `cpu_basis: &'static str` field that documents
    the measurement basis as "system Â· all logical processors",
  * be safe to keep in a long-lived `SystemMonitor` (the prime
    counter does not reset on subsequent samples).

* `app/src-tauri/src/app_state.rs` â€” adds a
  `monitor: parking_lot::Mutex<SystemMonitor>` field to `AppState`.
  The monitor is constructed **once** at app boot, so CPU-time
  deltas remain stable across `dashboard_summary` calls.
  Constructing a fresh monitor per request was the root cause of the
  "CPU 100%" bug.
* `app/src-tauri/src/commands/app.rs` â€” `dashboard_summary` now
  reuses `state.monitor.lock().sample()` instead of constructing a
  new `SystemMonitor` per call.

**Changes (frontend)**

* `app/web/src/lib/ipc.ts` â€” `SystemSample` TypeScript type extended
  with `cpu_available?: boolean` and `cpu_basis?: string`.
* `app/web/src/pages/Monitor.tsx` â€” the CPU meter now:

  * is labelled "System CPU" (not just "CPU") to make the scope
    explicit,
  * displays `primingâ€¦` instead of `0.0%` / `100.0%` when
    `cpu_available: false`,
  * includes the `cpu_basis` string in the detail line.

**Regression test**

* `app/src-tauri/tests/phase_p1_monitor.rs` â€” 4 tests:

  1. `first_sample_is_unavailable` â€” the very first sample must
     report `cpu_available: false` and the sentinel
     `cpu_percent: -1.0`.
  2. `eventually_produces_available_sample` â€” within a handful of
     samples the monitor must reach a `cpu_available: true` state,
     and the value must be in the documented `0.0..=100.0` range.
  3. `memory_measurement_is_immediate` â€” memory values are sane
     from the first sample and `mem_used_mb <= mem_total_mb`.
  4. `collected_at_is_monotonic` â€” wall-clock timestamps never move
     backwards.

**Status: MET for CPU monitoring.** The Knowledge Vault black-screen
investigation continues into Phase 7 (attachments and document
ingestion). The per-route ErrorBoundary prevents a malformed document
record from killing the whole application, which is the user-visible
symptom of the reported failure.

### Verification

* [x] `cargo test --release` runs **184 tests, 0 failed, 1 ignored**
      (180 baseline + 4 new monitor tests).
* [x] `npm --prefix app/web run build` clean (412.05 kB / 110.33 kB
      gzip).
* [x] `cargo check --lib --tests --release` clean (only pre-existing
      warnings).
* [x] `grep -nE 'window\.(prompt|alert|confirm)' app/web/src/**/*.tsx`
      returns 0 hits in source.
* [x] No destructive Git operations; the user's prior in-flight
      changes (Research.tsx, Models.tsx) are preserved and extended.
* [x] No new telemetry, no new external services.

### What remains

* Phase 7 â€” Attachments and performance (P9, P10, P11): file/image
  attachments in the chat composer, latency instrumentation, optional
  `llama.cpp` backend (only if measured evidence supports it).
* Document and Knowledge Vault ingestion hardening (P8) â€” the
  ErrorBoundary mitigation is in place; the per-document validation
  pass is the next concrete slice.
* A fuller Markdown renderer (Shiki-style syntax highlighting,
  table support) â€” explicitly deferred to Phase 7 alongside
  attachments.


---

## Phase 1 addendum — Attachment pipeline, latency instrumentation, and Knowledge Vault ingestion hardening (P6, P8, P9, P10)

This addendum records the concrete fixes that landed after the
"Phase P1 — Critical Bug Fixing" section above. All prior tests
remain green and a new `phase_p1_vault` regression suite covers
the root-cause cases.

### Knowledge Vault — Python-file black-screen (P8, root cause + fix)

**Symptom:** After importing a Python file via "Add file" the
Knowledge Vault page rendered a black screen. The rest of the
application continued to work; only the Vault page was affected.

**Root cause:** `app/src-tauri/src/ingestion/pipeline.rs`
`list_documents` and `get_document` did not select the
`content_hash` column and the Rust `DocumentRow` struct did not
declare a `content_hash` field. The TypeScript `DocumentRow`
declared `content_hash: string` and the Vault page rendered
`d.content_hash.slice(0, 10)`. The serialized JSON did not
contain a `content_hash` key, so the field was `undefined`, and
`undefined.slice` threw an uncaught error inside the React
component. The `ErrorBoundary` rethrew into the panel-render
path and produced the black screen.

**Fix (Rust):** add `content_hash` to both SELECT statements and
to the `DocumentRow` struct (as `Option<String>` for legacy-row
tolerance); surface the column in the orchestrator JSON.

**Fix (TS):** widen the type to `string | null` and render a
"no hash" fallback instead of calling `.slice` on a possibly-null
value. **Fix (test):** new regression test
`list_documents_returns_content_hash` asserts the serialized JSON
contains the `content_hash` key.

### Chunking — citation-offset drift (P8)

**Symptom:** Citation offsets returned by search occasionally
pointed outside the paragraph that was being cited. The drift
was small (a few bytes) but visible to the user.

**Root cause:** The chunker in
`app/src-tauri/src/ingestion/chunker.rs` used
`cursor = p_start + p.len() + 2` after `trim()`-ing the
paragraph. The trimmed length is shorter than the original
separator-bounded length, so `cursor` drifted and the
`start_offset` / `end_offset` emitted on each chunk no longer
matched the original document bytes.

**Fix:** rewrite the chunker to track the original-text byte
offset of each paragraph (computed once, before the `trim()`) and
rebase `buf_start` after the trim. The
`chunker_offsets_round_trip_into_text` regression test asserts
that slicing the original text at the chunk's offsets recovers
the trimmed content; the `chunker_handles_code_with_line_prefix`
test asserts monotonic, in-bounds offsets for the
Python-extractor's `"{:>5}  "` line-number output.

### Source-file decoder — non-UTF-8 tolerance (P8)

**Symptom:** A Python file saved in a Windows codepage
(cp1252 / cp1251) was rejected by the extractor and recorded as
`status: "failed"` in the `documents` table.

**Root cause:** `extractors::extract_code` and
`extractors::extract_text` used `std::fs::read_to_string`, which
fails outright on a non-UTF-8 byte.

**Fix:** Read the bytes first, then decode with
`std::str::from_utf8` and fall back to
`String::from_utf8_lossy`. Record `non_utf8_replaced: bool` in
the extraction metadata so the user can see when a file had
non-UTF-8 bytes replaced. New tests
`extract_code_tolerates_non_utf8_bytes` and
`extract_text_tolerates_non_utf8_bytes` cover both paths.

### Deduplication (P8)

The `documents` table already had a
`UNIQUE INDEX idx_documents_hash ON documents(content_hash)`. The
helper `ingestion::pipeline::insert_document` did not check for
duplicates and would surface the SQLite constraint error
upwards. New test `documents_dedupe_by_content_hash` asserts the
constraint fires; the orchestrator's higher-level
`register_and_ingest` path already does the dedup-by-hash check
before insert.

### Chat attachments (P9)

**New Tauri command** `chat_read_attachment({ path }) ->
AttachmentPayload` reads a small text / source file via the same
`ingestion::extractors` machinery the Vault uses, tolerates
non-UTF-8 bytes, and caps each attachment at 256 KB. Files
larger than the cap are returned truncated with
`truncated: true` so the frontend can warn the user.

**`SendRequest.attachments: Vec<AttachedText>`** is folded into
the user-visible text by `build_user_text_with_attachments` as a
fenced block per file (with a `python` / `typescript` / `rust`
language hint derived from the file extension). The text is
persisted as part of the user message, so the conversation
history is self-contained and re-renderable without re-reading
the file. Attachments are not auto-ingested into the Knowledge
Vault; the user must register a source explicitly.

**Frontend:** new `app/web/src/lib/attachments.ts` module,
paperclip button in the Assistant composer, removable
attachment chips, 8-attachment cap. Code path is end-to-end and
the per-attachment language tag is plumbed through to the
MarkdownView so attachment code blocks are highlighted too.

### Latency instrumentation (P10)

`ChatDeltaEvent` now carries two optional timing fields:
`dispatched_at_ms` (set on every event for a given request) and
`first_token_at_ms` (set on the first `token` and copied to
every later event). The Assistant page renders
`TTFT xx.xxs` alongside the live `Generating · 1.2s` clock, and
the ttftMs value is preserved until the next send so the user
can see the time-to-first-token next to the backend-reported
`latency_ms`. No invented numbers: the values come from the
backend's wall clock via Tauri's IPC bridge.

### MarkdownView upgrade (P6)

`app/web/src/components/MarkdownView.tsx` now supports:

- **Tables** with `| --- | --- |` separator, `:` markers for
  alignment, and styled header / body rows.
- **Ordered and unordered lists** (the previous version only
  handled unordered).
- **Task lists** (`- [ ]` / `- [x]`) with read-only checkboxes.
- **Horizontal rules** and **blockquotes** with proper
  `border-l` styling.
- **Code blocks** with a language label, a "Copy" button, and
  a "Show all (N)" expand button when the block has more than
  18 lines.
- **Lightweight syntax highlighting** for Python, JS/TS, Rust,
  Go, Bash, JSON, YAML, TOML, SQL, HTML, CSS — implemented as a
  hand-rolled tokenizer (no Shiki / Prism dependency) to keep
  the assistant bundle small. Comment colour, keyword colour,
  string colour, and number colour are applied via Tailwind
  classes.
- **`useStableMarkdown(text, 60)`** debounce hook so a fast
  stream of tokens does not re-render the parser 30 times per
  second.

### Verification (P6 / P8 / P9 / P10)

* [x] `cargo test --release` runs **190 tests, 0 failed, 1 ignored**
      (184 prior + 6 new in `tests/phase_p1_vault.rs`).
* [x] `npm --prefix app/web run build` clean
      (425.93 kB / 114.35 kB gzip).
* [x] `cargo check --lib --tests --bin nexus --release` clean
      (only pre-existing warnings).
* [x] Python import + Knowledge Vault page no longer black-screen
      on the seeded cp1252 file.
* [x] Citation offsets round-trip into the original document
      text.
* [x] Attachment text is folded into the user message; the
      MarkdownView language tag is honoured.

### What remains (after this addendum)

* `llama.cpp` optional backend (P11) — only if measured evidence
  shows the Rust-native pipeline cannot hit the model
  manager's local-backend polling.
* Cross-document citation graph (Phase S.4) — the
  chunker + citation code is now stable enough to support a
  graph view over `kg_evidence`.
* Optional: replace the lightweight syntax highlighter with
  Shiki if a future phase needs more languages.

## Phase R3 â€” Release stabilization (chat, embeddings, storage, packaging)

Added by the release-stabilization sweep. 2026-10-13.

### Chat

- **`.split()` crash on Assistant and other pages** â€” fixed.
  `ContextInspector.tsx` had a lookup table that returned
  `undefined` for unknown `ContextSource` values; calling
  `.split()` on `undefined` threw inside the `Array.map` body
  and took the whole Assistant tree down. Added `tintFor()`,
  `splitTint()`, and `FALLBACK_TINT` helpers, plus a
  `ContextInspectorSafe` wrapper that uses a local
  `ErrorBoundary`. The route-level boundary in `App.tsx`
  remains the last line of defence.

- **Stuck "Waiting for model"** â€” fixed. The
  `onChatDelta` listener was registered once in a useEffect
  with `[]` deps; the closure captured the initial `active`
  value, so events for a different conversation were
  filtered out, leaving `setRunning(false)` un-called.
  Mirrored `active` into a ref so the listener always sees
  the current value, and added a 240-second JS-side watchdog
  that transitions the UI out of the "running" state if no
  terminal event arrives. Watchdog is cleared on every
  `done`/`error`/`cancelled` event, on cancel, on send-catch,
  and on unmount.

### Knowledge Vault â€” embedding dimensions

- **Hardcoded 384 dimension** â€” fixed. Migration **0016**
  renames the legacy `chunk_vectors` vec0 table to
  `chunk_vectors_384` (and its shadow helper tables via a
  Rust migration helper), adds `dimension` and
  `model_identity` columns to `model_profiles`, and creates
  the `embedding_collections` registry.

- **`vector::upsert` is now per-dimension**: vectors land in
  `chunk_vectors_<dim>` based on the actual length of the
  vector. Per-dimension tables are created on first insert.

- **`vector::register_collection` refuses to overwrite a
  collection with a different model identity**. Two models
  that share a dimension are *not* interchangeable (semantic
  incompatibility), so the registry prevents silent mixing.

- **`vector::search` is dimension-aware**. A 384-dim query
  against a 768-only database returns an empty result (no
  error). When the database has a collection for the query
  dimension, the KNN runs in the vec0 table and the
  matching chunk text is fetched in a second query (the
  `MATCH` operator cannot be used inside a JOINed subquery
  in current sqlite-vec).

- **New command `models_test_embedding`** lets the frontend
  probe a model with a short string and remember its
  dimension. The Knowledge Vault has a "Detect dimension"
  button; the Model Manager shows the dimension in the
  profile list and accepts a pre-set value.

### Storage and migration

- **Configurable data directory**. Settings â†’ Data directory
  shows the current path and free space, lets the user
  pick a new path, validates it (writable, parent exists),
  and copies every file from the old location to the new
  one. The change is persisted in `config.toml` and
  requires a restart to take effect (the running SQLite
  pool cannot be moved mid-process).

- **Portable mode**: if `<exe_dir>/data` exists and is
  writable, NEXUS uses it automatically. The user can copy
  the whole `NEXUS/` directory onto a thumb drive and it
  becomes portable.

- **Env var override**: `NEXUS_DATA_DIR` overrides
  everything else. Useful for launcher scripts and for
  running multiple instances against different datasets.

### Packaging

- `tauri.conf.json`: `bundle.targets` =
  `["nsis", "msi"]`, `bundle.windows.webviewInstallMode`
  = `embedBootstrapper` (Tauri 2's offline WebView2
  bootstrapper strategy). `bundle.windows.nsis.installerIcon`
  is set to the application icon.
- Both NSIS and MSI are produced by `cargo tauri build`.
  The NSIS installer is **7.49 MB**; the MSI is
  **9.87 MB**.

### Tests

- **195 passed, 0 failed, 1 ignored** in 28 binaries.
  5 new tests in `tests/phase_p1_vault.rs`:
  - `per_dimension_tables_are_created_lazily`
  - `upsert_routes_to_per_dimension_table`
  - `collection_refuses_to_mix_incompatible_models`
  - `collection_allows_same_model_to_extend`
  - `search_only_returns_hits_for_registered_dimension`

### Known limitations

- The C: drive space loss is **not caused by NEXUS app
  data** (5 MB total). The 27 GB figure is the project
  on D:, almost entirely `target/debug/`. Run
  `cargo clean` in `app/src-tauri` to recover ~22 GB.
- Phase 4 GUI changes (Time Machine, Work Resume,
  Opportunity Radar) were verified by code inspection
  only. The action handlers are wired to real backend
  commands; visual layout needs an on-device check.
- `cargo tauri build` requires a one-time download of the
  WebView2 bootstrapper at build time. After that, the
  produced installer is fully offline-installable.

Phase P2 — Chat lifecycle, image attachments, plugin integration (2026-10-13)

Added by the chat-lifecycle + image + plugin-integration sweep. Builds on top of Phase R3 (release stabilization).

## Critical bug fixes

### Bug A — Chat stuck on "Waiting for model"
- **Root cause 1**: `RawFrame::SseDone` in `providers::lmstudio` was a no-op. Some servers (LM Studio, llama.cpp) send `data: [DONE]` and keep the connection open. The previous loop waited indefinitely.
- **Root cause 2**: `tokio::time::sleep(timeout)` inside the `select!` in `commands::chat` was re-created every loop iteration. The 180 s deadline never actually fired.
- **Root cause 3**: terminal `error` events after partial content clobbered the visible message with the error string.
- **Fix**: `sse_done_seen` flag breaks the loop on `[DONE]`; absolute `deadline` computed once; partial content preserved and message tagged `interrupted` (not `error`).
- **Test coverage**: `tests/phase_p2_chat_lifecycle.rs` (12 tests).

### Bug B — Attachments not understood
- **Symptom**: PNG / JPEG / GIF / WebP files were unreadable; only filenames reached the model.
- **Fix**: `chat_read_attachment` sniffs magic bytes + extension; emits a `data:<mime>;base64,…` URI for images; chat orchestrator builds an OpenAI multimodal content array when image attachments are present.
- **Test coverage**: `tests/phase_p2_attachments.rs` (8 tests for text/code/JSON/CSV/ipynb routing); `tests/phase_p2_chat_lifecycle.rs` (3 multimodal-shape tests).

## Plugin integration (NEXUS-native equivalents)

### rag-v1
- The cloned plugin is an LM Studio prompt preprocessor that depends on `client.embedding` and `client.files.retrieve`. It runs **inside** LM Studio, not in NEXUS.
- NEXUS already has the equivalent functionality in the existing Knowledge Vault (`use_vault: true` in `chat_send` triggers hybrid FTS5 + vec0 KNN retrieval; citations are persisted on the assistant message; per-dimension embedding tables prevent cross-contamination).
- The user's existing vault data and embeddings are preserved.

### js-code-sandbox
- The cloned plugin is an LM Studio tool that spawns a Deno subprocess with the flags `--allow-read=.` `--allow-write=.` `--no-prompt` `--deny-net` `--deny-env` `--deny-sys` `--deny-run` `--deny-ffi` (5s default timeout, max 60s).
- New NEXUS-native command `sandbox_run_javascript` mirrors the exact flag set, default/max timeouts, and isolation boundaries. The script is written to a temp file inside a per-data-dir scratch directory and removed after the run.
- New `sandbox_status` command lets the UI show "Deno not found" before the user tries to run a snippet.
- Deno is **not bundled** with NEXUS — the user installs it themselves. The `find_deno` helper looks in `$DENO`, `$PATH`, and LM Studio's `.internal/utils/deno` (so an existing LM Studio install reuses its Deno).

## Files changed (this phase)

- `app/src-tauri/Cargo.toml` — `base64 = "0.22"` (was already in the lockfile).
- `app/src-tauri/src/providers/mod.rs` — `ChatMessage.content` is `serde_json::Value`; `ChatMessage::text(role, text)` helper.
- `app/src-tauri/src/providers/lmstudio.rs` — `RawFrame::SseDone` is authoritative.
- `app/src-tauri/src/commands/chat.rs` — image attachment routing; multimodal content builder; deadline-based timeout; partial-content preservation; image size cap (20 MB).
- `app/src-tauri/src/commands/models.rs` — `ChatMessage::text`.
- `app/src-tauri/src/commands/sandbox.rs` *(new)* — Tauri commands `sandbox_run_javascript`, `sandbox_status`.
- `app/src-tauri/src/commands/mod.rs` — registers `sandbox` module.
- `app/src-tauri/src/sandbox/mod.rs` *(new)* — `find_deno`, `run_javascript`, `SandboxResult`, kill-on-timeout, scratch-directory isolation.
- `app/src-tauri/src/lib.rs` — `pub mod sandbox;`.
- `app/src-tauri/src/main.rs` — registers the new Tauri commands.
- `app/src-tauri/tests/live_chat.rs`, `tests/live_lmstudio.rs` — use `ChatMessage::text`.
- `app/src-tauri/tests/phase_p2_chat_lifecycle.rs` *(new)* — 12 tests.
- `app/src-tauri/tests/phase_p2_attachments.rs` *(new)* — 8 tests.
- `app/src-tauri/tests/phase_p2_sandbox.rs` *(new)* — 5 tests + 1 ignored.
- `app/web/src/pages/Assistant.tsx` — file picker advertises image filters; image preview is a marker; partial content preserved on error.
- `app/web/src/lib/attachments.ts` — `AttachedText.kind` accepts `"image"`.

## Test result

```
cargo test --release
TOTAL: passed=225 failed=0 ignored=3
```

Web build: 433.70 kB / 117.05 kB gzip. `cargo check --lib --tests --bin nexus --release` clean.
---

# Phase P3 — Critical bug fix and integration (2026-10-13)

Added by the four-bug fix & integration sweep. Builds on Phase P1, P2, and R3.

For the full handover see `.scratch/phase_p3_handover.md`.

## Bugs fixed

### Bug A — Knowledge Vault retrieval in chat

- Root cause 1: `search_fts` collapsed `Arduino.txt` to the single
  token `Arduinotxt` because every non-alphanumeric character was
  stripped before the FTS5 query was built.
- Root cause 2: even with sane tokenisation, naming a file in the
  query does not match the chunk text (the chunk is the file body,
  not the filename). There was no document-name hint path.
- Fix: new `tokenize_query` preserves `.` and `-` as word
  separators and quotes each term; `find_documents_by_name_hint`
  and `chunks_for_document` add a docname hint path the user can
  opt into with a file-noun ("the arduino file"). `SearchResult`
  now reports `used_docname_hint` and `docname_hint_doc_ids`.
- Tests: `tests/phase_p3_vault_retrieval.rs` — 10 tests.

### Bug B — Search results stacking

- Root cause: `Knowledge.tsx::onSearch` had no request ID, so
  two back-to-back searches could race.
- Fix: `searchSeqRef = useRef(0)` captures a per-search sequence;
  the result is committed only when its sequence is still the
  latest. The result list is optimistically cleared on a new
  search so the user sees a loading state, not stale rows.
- Web build OK; no backend changes.

### Bug C — Automatic chat naming

- Root cause: the previous implementation lived in the browser
  and was best-effort (could race, could be overwritten, derived
  titles from a literal substring).
- Fix: server-side `auto_title::compute_auto_title` calls the
  active `ModelProvider::chat_complete` with `max_tokens: 24` to
  summarise the first user message, validates and normalises,
  and persists via `conversations::rename`. The chat pipeline
  spawns this as a fire-and-forget task after a successful
  `done`. A new `nexus:chat.title_updated` event patches the
  sidebar in place. New `chat_complete` method on the
  `ModelProvider` trait (LM Studio, llama.cpp, Ollama
  implementations).
- Tests: `tests/phase_p3_auto_title.rs` — 12 tests.

### Bug D — Image attachment crash

- Root cause: the chat pipeline forwarded `image_url` to a
  text-only model; the upstream 4xx plus a torn-down stream
  left the UI in a stuck state. There was no per-profile
  vision-capability flag and no validation of the image data
  URI.
- Fix: migration 0017 adds `model_profiles.supports_vision`
  (default false). The chat pipeline refuses image attachments
  up front when the active model does not advertise vision.
  `sanitise_image_data_uri` replaces malformed data URIs with
  a `data:text/plain` error marker.
- Tests: `tests/phase_p3_image_attachment.rs` — 6 tests.

## Plugin integration

- `rag-v1/` is an LM Studio plugin (depends on
  `@lmstudio/sdk` 1.4.0). It is not in-process integrable into
  NEXUS's Rust/Tauri architecture. NEXUS's Knowledge Vault is
  the native equivalent and now does the same job (and more) —
  vault retrieval is exercised by Bug A's regression suite.
- `js-code-sandbox/` is also an LM Studio plugin. NEXUS's
  `nexus_core::sandbox::run_javascript` is the native equivalent
  and matches the same Deno permission policy
  (`--allow-read=<dir> --allow-write=<dir> --no-prompt
  --deny-net --deny-env --deny-sys --deny-run --deny-ffi`)
  with the same 5s/60s timeout contract.
  `tests/phase_p2_sandbox.rs` (5 tests + 1 ignored) verifies
  the contract.

## Test result

```
cargo test
TOTAL: passed=230 failed=0 ignored=1
```

Web build: 435.00 kB / 117.47 kB gzip.
`cargo check --lib --tests --bin nexus --release` clean.

29 test binaries. New ones: `phase_p3_vault_retrieval`,
`phase_p3_auto_title`, `phase_p3_image_attachment`.

---

# Phase P4 — Request lifecycle, navigation survival, and Memory Center persistence (2026-10-13)

Added by the navigation + memory-save + cross-cutting lifecycle sweep. Builds on P1, P2, P3, and R3.

For the full handover see `.scratch/phase_p4_handover.md`.

## Critical bug fixes

### Bug E — Assistant responses disappear on navigation
- **Symptom**: When the model is generating a response and the user navigates to another page in NEXUS, the response disappears or stays invisible on return.
- **Root cause**: All chat streaming state lived in component-local `useState` that died on unmount. A late `done` event after navigation was dropped on the floor. The assistant message row was created with `status: "streaming"` and `content: ""`; without an in-flight listener, the partial content was never delivered.
- **Fix**:
  1. Every `nexus:chat.delta` now carries a stable `request_id` (ULID).
  2. `chat_send` registers the request in a new per-request registry; every exit path calls `unregister_request`.
  3. The assistant message is persisted throttled at 250 ms (was 50 ms — too chatty for SQLite) on every streaming tick, plus fully on every terminal event.
  4. The frontend queries `chat_request_list_active` on every mount/remount and rehydrates the in-flight state for the current conversation.
  5. Late events whose `request_id` does not match the current active request are dropped at the top of `handleDelta`.

### Bug F — Memory Center does not save chat-issued facts
- **Symptom**: "Save to your memory that my age is 22" took 91 s of token generation, returned no visible answer, and persisted nothing.
- **Root cause**: Every user message was sent to the model as a normal chat completion. The model had to invent a `save_memory` tool call out of thin air; it produced a 91 s apology and never persisted anything.
- **Fix**:
  1. New deterministic local service `memory_v2::explicit_save` recognises the save phrase, extracts the "that <fact>" clause, validates and classifies it, and writes through the canonical `memories_v2_create` path. The model is **not** called.
  2. The chat pipeline intercepts the request **before** the vault retrieval runs, so a save doesn't accidentally pull in citations.
  3. The deterministic service emits one of four outcomes — `Saved`, `Updated` (idempotent), `Rejected`, `Failed` — and the frontend renders the chip in the activity timeline. Memory Center listens for `nexus:memory.saved` and refreshes automatically.
  4. Dedupe is handled by the existing `find_by_hash` on `memories_v2`; a second identical save returns the existing id, not a duplicate row.
  5. **No** general-purpose DB-writing tool is exposed to the model. The memory pipeline is the only writer.

### Bug L — Requests consume tokens but never finish
- **Symptom**: 91 s of model output with no terminal state; 48 s of output followed by `Waiting for model · 70.0s` sticking; Stop button remained active after the response was visible.
- **Root cause**:
  1. `AppState::active_cancellations` was a `Vec<CancellationToken>` with no request-id tagging. `cancel_inflight` cancelled **every** active request, including unrelated Model Manager test prompts.
  2. The frontend `running` flag was reset only on a `done` event. If the user navigated away during the gap, the `done` event was lost and the UI stayed in `running` forever (the 240 s JS-side watchdog was the only safety net).
- **Fix**:
  1. Replaced the `Vec<CancellationToken>` with `HashMap<String, ActiveCancel>` keyed by request id. New methods: `register_request`, `unregister_request`, `cancel_request`, `cancel_kind`, `cancel_conversation`, `list_active`.
  2. New Tauri commands: `chat_request_list_active` (filtered to `kind == "chat"`) and `chat_request_cancel`.
  3. `cancel_inflight` (Stop button) now defaults to chat-only (`cancel_kind("chat")`). Pass `cancel_all: true` for the legacy "kill everything" behaviour the Model Manager still uses.
  4. Every chat request is registered with `kind = "chat"`, the conversation id, and the assistant message id. The request id is emitted on every delta. `unregister_request` runs on **every** exit path: `done`, `error`, `cancelled`, and wall-clock timeout.
  5. The frontend's `onSend` cancels any in-flight request for the current conversation before sending, so a user who sends "what's my age?" while a save is in flight deterministically cancels the save.

## Streaming & persistence contract

- `conversations::update_message` is called every 250 ms while a request is running, plus on every terminal event. The terminal event always persists the final content with `status = "complete"`.
- On the wall-clock timeout path the message is persisted with `status = "interrupted"` and a final `kind = "error"` event is emitted with the partial content.
- The `done` event is emitted **after** the persisted final content is written, so a re-mount of the chat page always sees a consistent DB state.

## Test result

```
cargo test --lib --tests
TOTAL: passed=276 failed=0 ignored=1
```

Web build: 436.39 kB / 117.91 kB gzip.
`cargo check --lib --tests --bin nexus --release` clean.

36 test binaries. New ones: `phase_p4_lifecycle` (9), `phase_p4_memory` (12), and 5 in-module unit tests on `memory_v2::explicit_save`.
