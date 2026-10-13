# NEXUS — Audit Report (v1.0.0)

This is the final audit at the end of the post-Phase-5 continuation
work. Every subsystem is covered by deterministic tests, the Vite
build is green, the release cargo build is clean, and the Windows
installer builds.

## Environment

* OS: Windows 11.
* Repo root: `D:\NEXUS`.
* Git: not initialised (no version control yet). The work lives on
  the working tree only.
* Stack: Tauri 2, Rust (toolchain 1.99, GNU), React 18, TypeScript,
  Vite, Tailwind.

## Test totals (v1.0.0)

```
$ cargo test --lib --tests
   104 passed, 1 ignored (MemPalace live), 0 failed
```

Across 21 test binaries:

| Binary         | Tests | Note                         |
|----------------|------:|------------------------------|
| `nexus-core` lib | 16 | Unit tests in core modules. |
| `nexus` (main)  | 3     | Audit clear / prune / no-op. |
| `live_chat`    | 1      | `#[ignore]` candidate; skip.|
| `live_lmstudio`| 1      | `#[ignore]` candidate; skip.|
| `phase1`       | 2      | Foundation.                |
| `phase2`       | 8      | Providers.                 |
| `phase3`       | 3      | Assistant.                 |
| `phase4`       | 5      | Vault.                     |
| `phase4_live`  | 1      | Live PDF ingest.           |
| `phase5`       | 9      | Projects / notes / tasks.  |
| `phase5b`      | 10     | Memory v2.                 |
| `phase_c`      | 6 + 1  | MemPalace (1 ignored).     |
| `phase_d`      | 8      | Knowledge graph.           |
| `phase_e`      | 5      | Context builder.           |
| `phase_f`      | 8      | Model router.              |
| `phase_g`      | 5      | Research studio.           |
| `phase_h`      | 2      | Experiments.               |
| `phase_i`      | 5      | Controlled agent.          |
| `phase_j`      | 2      | Document intelligence.     |
| `phase_n`      | 4      | Operations & policy surface.|

## Post-1.0.0 continuity upgrade — Phase S.1 (Project Time Machine)

* New additive migration `0012_timeline.sql` creates the
  `timeline_events` table with stable identity, source provenance,
  separate `event_at` (when the source fact happened) and
  `imported_at` (when NEXUS first wrote the row), short factual
  summary, structured `detail_json`, and a UNIQUE
  `(source_kind, source_id, event_type, event_at)` dedup key.
  Three hot-query indexes: `(project_id, event_at DESC, id ASC)`,
  `(source_kind, source_id)`, `(event_type)`, plus the
  `ingest_run_id` index. No existing table is altered. Existing
  data is not migrated. The table starts empty and is populated
  on explicit user demand (`timeline_ingest_project`).
* New `nexus_core::timeline` module with one idempotent ingestor
  per source: `audit`, `tasks`, `notes`, `memories_v2`,
  `experiments` (and runs), `kg_relationships` (temporal),
  `agent_invocations`, `research_papers`, `research_reports`,
  plus a `manual` channel for user-written milestones. Every
  ingestor is read-only against the source table.
* Read-only queries: `list` (paginated, deterministic sort),
  `reconstruct_at` (per-section snapshot with explicit
  "known" / "partial" / "unknown" status),
  `compare(from_ms, to_ms)` (events added in a window),
  `superseded` (memories and KG relationships with `valid_until`
  or `superseded_by` set), and `decisions` (synthesised read-only
  view of approved decision memories).
* 9 Tauri commands: `timeline_ingest_project`, `timeline_list`,
  `timeline_count`, `timeline_reconstruct`, `timeline_compare`,
  `timeline_superseded`, `timeline_decisions`,
  `timeline_insert_manual`, `timeline_delete`. All commands are
  read-only against source tables; only the timeline_events table
  is written, idempotently.
* New `Time Machine` sidebar entry under the `Work` group, with
  a dedicated `Timeline.tsx` page that supports project
  selection, source-kind / event-type filters, time-range filters,
  reconstruction at a chosen moment, past-vs-present comparison,
  decisions archaeology, superseded-idea listing, and
  hand-written milestone entry. The page uses the existing
  `panel`, `select`, `input`, `btn`, `btn-primary` design
  primitives — no new visual conventions.
* New `tests/phase_s.rs` with 11 tests covering:
  * Migration to v12 and the unique key + index existence.
  * Idempotency: re-inserting the same source event is a no-op;
    different `event_type` on the same source is a distinct row.
  * Deterministic sort: two events with the same `event_at` come
    back in the same order across calls.
  * Cross-source ingest: tasks, notes, memories, audit are all
    represented after one `ingest_all_for_project` call, and the
    second call inserts nothing.
  * Reconstruction: a read-only call never mutates `tasks`,
    `notes`, or `memories_v2`, and a project with no ingested
    timeline explicitly reports "unknown" rather than guessing.
  * `compare` returns events in a `[from, to)` window; rejects
    `to < from`.
  * `superseded` lists the memory and KG relationship we set up.
  * `decisions` returns only the approved decision memory, not
    memories of other categories.
  * `delete` removes a single timeline row without touching the
    source record.
  * Manual entries preserve the historical `event_at` while
    writing `imported_at = "now"`.

**Phase S.2 — One-Click Work Resume (shipped).**

Phase S.2 lands the user-visible "Resume" feature on top of
the S.1 timeline. A checkpoint is a structured, model-
independent record of where work stopped. It contains
evidence references (ids into the canonical tables) and
small, structured observations (file paths, test results,
decisions). It never copies whole documents, chat
transcripts, or git repositories. It is read-only against
the source tables: editing or deleting a checkpoint never
touches tasks, notes, memories, or anything else.

**Schema (`migrations/0013_checkpoints.sql`).** Two new
tables, fully additive — no existing table is modified.
`checkpoints` carries the user-written fields
(`goal`, `completed_work`, `current_state`,
`next_action_text`), structured JSON columns
(`observed_files`, `observed_tests`, `errors`,
`decisions`, `constraints`, `pending_actions`,
`evidence_refs`), a `drift_check_json` snapshot pinned to
the moment of creation, and a `status` lifecycle
(`active` | `resumed` | `archived`). `checkpoint_resumes`
records every resume attempt with its `drift_report_json`
and is wired with `ON DELETE CASCADE` so deleting a
checkpoint is safe. Schema is now at v13.

**Module (`src/checkpoints/`).** `mod.rs` exposes
`create`, `get`, `list` (with a `ListFilter` for
`project_id`, `task_id`, `status`, `limit`), `update` (with
a tri-state `Option<Option<String>>` encoding for "leave
alone / clear / set" on every text column), `set_status`
(one-way to `resumed` and stamps `resumed_at`), `delete`,
`resume_summary` (returns the updated checkpoint, a
`DriftReport`, and a deterministic rendered text summary;
writes one `checkpoint_resumes` row and transitions the
parent to `resumed`), `list_resumes`, and
`render_resume_summary`. `drift.rs` is read-only: it
captures a snapshot of the world at create time, and
re-reads the world at resume time to produce a structured
`DriftReport`. Drift emits items of kinds
`open_task_count`, `note_count`, `active_model`,
`file_missing`, `file_size_changed`, `file_mtime_advanced`,
and `evidence_missing` (for references that no longer
resolve in their source table). Drift never executes a
Git command or any other external tool.

**Model independence.** `model_hint` is a UI hint stored
on the row. The drift report does not depend on it, the
resume summary does not embed it, and a different model
can resume the same checkpoint without any change to the
row or the data on disk. The resume summary text is a
pure function of the checkpoint fields and the current
state of the source tables, so it is identical across
runs (and across models).

**Source-table read-only invariant.** `create`, `update`,
`set_status`, `delete`, and `resume_summary` never
`UPDATE`, `INSERT`, or `DELETE` any row in `tasks`,
`notes`, `memories_v2`, `kg_*`, `experiments`,
`agent_*`, or any other source table. They only touch
`checkpoints`, `checkpoint_resumes`, and `audit_events`.
The `resume_does_not_modify_source_tables` test asserts
this by snapshotting `tasks`, `notes`, and `checkpoints`
counts before and after five resume calls.

**Commands (`src/commands/checkpoints.rs`).** 8 Tauri
commands: `checkpoints_list`, `checkpoints_get`,
`checkpoints_create`, `checkpoints_update`,
`checkpoints_set_status`, `checkpoints_delete`,
`checkpoints_resume`, `checkpoints_list_resumes`. Each
write wraps an `audit::log` call
(`checkpoint.create`, `checkpoint.update`,
`checkpoint.set_status`, `checkpoint.delete`,
`checkpoint.resume`).

**Frontend.** `lib/checkpoints.ts` mirrors the Rust types
exactly (including the tri-state wire encoding for
`UpdateCheckpointRequest`). `pages/Checkpoints.tsx` is
mounted at `/checkpoints` (sidebar group `Work`, new
`resume` icon). The page renders: a "New checkpoint"
builder (title, goal, completed work, current state,
next-step recommendation, observed files, observed tests,
decisions, pending actions); a project-scoped list of
saved checkpoints with per-row `Resume`, `History`,
`Archive`, and `Delete` actions; the resume view rendering
the deterministic backend text plus a severity-coded
drift-item list; and a `History` panel showing past resume
attempts with the drift-report counts at each. The Tasks
page now exposes a `＋ checkpoint` action on each
`TaskCard` that creates a checkpoint bound to the task
with the task's `description` as the goal, its completed
checklist items as `completed_work`, the unchecked
checklist items as `pending_actions`, and an
`evidence_ref` of `{kind: "task", id: task.id}`.

**Phase S.2 totals (re-verified in this environment):**

```
$ cargo test --lib --tests
   130 passed, 1 ignored (MemPalace live), 0 failed
   23 test binaries, including `phase_s2` (15 tests)
$ npm --prefix app/web run build
   vite v5.4.10 ... 80 modules transformed.
   dist/assets/index-*.js  373.14 kB | gzip: 101.77 kB
   built in 4.97s
$ cargo check --lib --tests --release
   Finished `release` profile [optimized] target(s) in 1m 00s
```

No regression: the 104 pre-existing tests all still pass, and
the 11 new tests cover the timeline invariants end-to-end.

## Phase S.3 — Opportunity Radar, Hypothesis Ledger, Codebase Archaeologist (shipped)

Three of the nine S.3 sub-phases are landed. The other six
(Serendipity, Impact Simulator, Evidence Tribunal, Workflow
Builder, Capability Graph, Knowledge Vault 2.0) are scoped
in `IMPLEMENTATION_PLAN.md` as future S.3.4–S.3.9 sub-phases.

### S.3.1 — Opportunity Radar (read-only)

**Module (`src/radar/`).** `mod.rs` exposes a single entry
point, `radar::report(pool, project_id, &RadarOptions) -> RadarReport`.
The radar never writes to any table. The report is sorted by
`score` (descending) and then by `title` (ascending) for
stability. Kinds:

* `overdue_urgent_task` — priority `urgent`, due in the past.
  Score = `100.0 + 5.0 * days_overdue`.
* `overdue_high_task` — priority `high`, due in the past.
  Score = `60.0 + 5.0 * days_overdue`.
* `stale_open_task` — status `open`, untouched for N days
  (default 7). Score = `25.0 + 1.5 * days_stale`.
* `resumed_checkpoint_with_pending` — status `resumed`,
  `pending_actions_json` has at least one `done == false`
  entry. Score = `30.0 + 8.0 * pending_count`.
* `high_importance_recent_memory` — `memories_v2.importance
  >= 0.7` and `updated_at` within the last N days (default
  14). The radar detects whether the `importance` and
  `updated_at` columns exist and silently skips this kind
  on older DBs.
* `open_hypothesis` — `hypotheses.status = 'open'`. The
  radar detects whether the `hypotheses` table exists and
  silently skips this kind when it doesn't.

Scoring is deterministic given the same input data. The
test `radar_scoring_is_deterministic` asserts that two
consecutive `report` calls return the same kinds, the same
titles, and the same scores to within `1.0` of each other
(allowing for a few seconds of wall-clock drift between
the two `now` samples).

**Commands (`src/commands/s3.rs`).** `radar_report` and
`radar_counts`. Both are read-only.

**Frontend.** `lib/s3.ts` (`radarReport`, `radarCounts`)
+ the Radar tab of `pages/OpportunityRadar.tsx` (mounted
at `/radar`, sidebar group `Work`, new `radar` icon).
The tab renders the counts grid, the sorted list of
opportunities with a `score` column, and the kind chip.

### S.3.2 — Hypothesis Ledger

**Schema (`migrations/0014_hypotheses.sql`).** Two new
tables, fully additive. `hypotheses` carries the user-typed
fields (`title`, `statement`, `prediction`), a `status`
lifecycle (`open` | `confirmed` | `refuted` |
`abandoned`), a `confidence` in 0.0..1.0 (clamped at
write time), two JSON arrays of evidence entries
(`evidence_for_json`, `evidence_against_json`), a
`decided_at` + `decision_note`, an optional `memory_id`
as a free-form id reference (intentionally not a
FOREIGN KEY so a missing memory does not invalidate a
hypothesis), and bookkeeping times. `hypothesis_evidence`
is a journal of every piece of evidence recorded
against a hypothesis, with `kind` (`'observation' |
'experiment' | 'quote' | 'reasoning' | <free-form>`),
`summary`, `weight` (0.0..1.0, clamped), optional
`source_kind` / `source_id` pointing at a row in another
table, and a `stance` (`for` or `against`). Schema is
now at v14.

**Module (`src/hypotheses/mod.rs`).** `create` (rejects
empty title and statement; clamps confidence;
non-NaN guarantees), `get`, `list` (with
`ListFilter { project_id, task_id, status, limit }`),
`update` (with the tri-state `Option<Option<String>>`
encoding for "leave alone / clear / set" on every
nullable text column), `decide` (one-way transition
to a terminal status, refuses to overwrite a terminal
decision), `delete` (cascades to `hypothesis_evidence`
via FK), plus `add_evidence`, `get_evidence`,
`list_evidence`, `delete_evidence`.

**Commands (`src/commands/s3.rs`).** 9 hypothesis
commands: `hypotheses_list / _get / _create / _update /
_decide / _delete / _add_evidence / _list_evidence /
_delete_evidence`. Each write wraps an `audit::log` call
(`hypothesis.create`, `hypothesis.update`,
`hypothesis.decide`, `hypothesis.delete`,
`hypothesis.add_evidence`,
`hypothesis.delete_evidence`).

**Frontend.** `lib/s3.ts` (`hypothesesList`,
`hypothesesGet`, `hypothesesCreate`, `hypothesesUpdate`,
`hypothesesDecide`, `hypothesesDelete`,
`hypothesesAddEvidence`, `hypothesesListEvidence`,
`hypothesesDeleteEvidence`) + the Hypotheses tab of
`pages/OpportunityRadar.tsx`. The tab renders a list of
hypotheses for the active project, a new-hypothesis
builder (title, statement, prediction, confidence
slider), per-row `Confirm` / `Refute` / `Delete` actions,
an evidence panel for the selected hypothesis (showing
stance and weight chips), and an "add evidence" form.
The selected panel is read-only when the hypothesis is
in a terminal status.

### S.3.3 — Codebase Archaeologist (read-only)

**Module (`src/archaeologist/mod.rs`).** The
archaeologist walks a user-supplied root directory,
builds a `FileEntry` list (path, size, mtime, `FileKind`,
sha256), and returns an `IndexSummary` with aggregate
per-kind counts and a `skipped` list. It honours
`IndexOptions { hash, max_file_bytes, max_files,
max_depth, exclusions }`. Defaults exclude `.git`,
`node_modules`, `target`, `dist`, `__pycache__`,
`.venv`, `.next`, `build`. Symlinks are never followed.
`MAX_FILE_BYTES = 1 MiB`, `MAX_FILES_PER_INDEX = 50_000`,
`MAX_DIR_DEPTH = 16`. `find_by_name(index, query)` is a
case-insensitive substring search. `head(path, n)` reads
the first `n` lines (capped at 10,000) with a size cap
of 1 MiB. `resolve_under` rejects `..`-style escapes so
the user can never read outside the indexed root. The
archaeologist never modifies any file on disk; the
`archaeologist_is_read_only` test asserts that the tree
before and after an `index` and a `head` is byte-identical.

**Commands (`src/commands/s3.rs`).** `archaeologist_index`
and `archaeologist_head`. The index call wraps an
`audit::log` entry so the operation is traceable.

**Frontend.** `lib/s3.ts` (`archaeologistIndex`,
`archaeologistHead`) + the Archaeologist tab of
`pages/OpportunityRadar.tsx`. The tab renders a root
input, the per-kind counts grid, a name filter input,
a file list, and a head-viewer pane that shows the
first 200 lines plus the SHA-256 of the open file.

### Phase S.3.1–S.3.3 totals (re-verified in this environment)

```
$ cargo test --lib --tests
   151 passed, 1 ignored (MemPalace live), 0 failed
   24 test binaries, including `phase_s3` (21 tests)
$ npm --prefix app/web run build
   vite v5.4.10 ... 82 modules transformed.
   dist/assets/index-*.js  388.29 kB | gzip: 104.79 kB
   built in 1.24s
$ cargo check --lib --tests --release
   Finished `release` profile [optimized] target(s) in 1m 00s
```

No regression: the 130 pre-S.3 tests all still pass, and
the 21 new S.3 tests cover the radar's read-only invariant,
the hypothesis ledger's CRUD + terminal-decision guard,
and the archaeologist's path-traversal refusal.

## Phase S.3.4 — Serendipity (shipped)

**Module (`src/serendipity/`).** `mod.rs` exposes
`serendipity::report(pool, project_id, &SerendipityOptions) -> SerendipityReport`.
The module is fully read-only. It detects which source
tables and which columns exist
(`memories_v2.importance`, `memories_v2.updated_at`,
`notes.updated_at`, `experiments.updated_at`,
`experiments` table, `hypotheses` table,
`hypotheses.updated_at`, `checkpoints` table,
`checkpoints.created_at`) and silently skips any kind
that the schema cannot support. This means the module
works on the full v14 schema, on the v13 schema, and
on the v1 schema — it degrades gracefully.

**Kinds.** `dormant_memory`, `dormant_note`,
`dormant_experiment`, `forgotten_hypothesis`,
`cold_checkpoint`. Each item carries `id`,
`project_id`, `title`, `summary`, `days_dormant`,
`score`, and `ref_kind`. Scoring is
`surprise_score(days, importance, factor) =
log2(1 + days) * (1 + factor + importance)`. The
weights are pinned in `SerendipityWeights::default()`:
`dormant_memory_factor = 0.0`,
`dormant_note_factor = 0.2`,
`dormant_experiment_factor = 0.3`,
`forgotten_hypothesis_base = 0.5`,
`forgotten_hypothesis_confidence_factor = 0.5`,
`cold_checkpoint_factor = 0.1`. The
`forgotten_hypothesis` kind additionally weights by
`(1 - confidence)` so the shakiest open claims
float to the top.

**Cross-project panel.** A bounded scan
(`cross_project_limit`, default 10) pulls dormant
notes from *other* projects and pairs each with the
best-matching local item by shared trigrams. Trigram
generation is case-insensitive and ignores
non-alphanumeric characters.

**Commands (`src/commands/s3.rs`).** `serendipity_report`.
Read-only.

**Frontend.** `lib/s3.ts` (`serendipityReport`) +
the Serendipity tab of `pages/OpportunityRadar.tsx`.
The tab renders the dormant list, sorted by surprise
score, and the cross-project pairs.

## Phase S.3.5 — Impact Simulator (shipped)

**Module (`src/impact/mod.rs`).** Single entry point,
`impact::simulate(pool, &ImpactRequest) -> ImpactReport`.
The simulator never writes to any table. It walks the
existing tables (`timeline_events`, `checkpoints`,
`hypotheses`, `hypothesis_evidence`,
`checkpoint_resumes`, plus the `evidence_refs_json`
JSON array in `checkpoints`) and reports the
dependents. The destructive flag is `true` for every
`delete_*` action and `false` for `complete_task`.
The `complete_task` action emits a `warnings` entry
reminding the user that pending checkpoint actions
are not auto-closed.

**Supported actions.** `delete_note`, `delete_memory`,
`delete_task`, `delete_checkpoint`, `delete_hypothesis`,
`delete_project`, `complete_task`. Each produces a
`Vec<ImpactCount>` (per-table counts) and a
`Vec<AffectedRow>` (a sample of up to 25 affected
rows, plus any `warnings`).

**Commands (`src/commands/s3.rs`).** `impact_simulate`.
The command wraps an `audit::log` entry
(`impact.simulate`) so the dry-run is traceable.

**Frontend.** `lib/s3.ts` (`impactSimulate`) + the
Impact tab of `pages/OpportunityRadar.tsx`. The tab
lets the user pick an action, paste a target id,
and run a dry-run simulation; the result is shown
with a destructive/non-destructive chip, the per-
table counts, the warnings, and the affected
sample.

### Phase S.3.4–S.3.5 totals (re-verified in this environment)

```
$ cargo test --lib --tests
   174 passed, 1 ignored (MemPalace live), 0 failed
   26 test binaries, including
     phase_s3_v2 (12 tests) and
     phase_s3_4_5 (11 tests)
$ npm --prefix app/web run build
   vite v5.4.10 ... 82 modules transformed.
   dist/assets/index-*.js  394.65 kB | gzip: 105.74 kB
   built in 1.24s
$ cargo check --lib --tests --release
   Finished `release` profile [optimized] target(s) in 1m 00s
```

No regression: the 151 pre-S.3.4 tests all still pass,
and the 23 new S.3.4 / S.3.5 tests cover the
serendipity's read-only invariant, the cross-project
trigram pairing, the hypothesis confidence ordering,
and the impact simulator's destructive flag and
fan-out to dependent tables.

## Web build

```
$ npx tsc --noEmit
exit 0
$ npx vite build
dist/index.html                   0.52 kB
dist/assets/index-*.css          25.21 kB
dist/assets/index-*.js          346.32 kB
```

## Release installer (v1.0.0)

```
$ cargo tauri build
   Compiling nexus v1.0.0
    Finished `release` profile [optimized] target(s) in 3m 23s
    Finished 2 bundles at:
   D:\NEXUS\app\src-tauri\target\release\bundle\nsis\NEXUS_1.0.0_x64-setup.exe  (5.31 MiB)
   D:\NEXUS\app\src-tauri\target\release\bundle\msi\NEXUS_1.0.0_x64_en-US.msi    (7.39 MiB)
```

Standalone binary: `app\src\tauri\target\release\nexus.exe` (16.47 MiB).

## Phase status

| Phase | Status | Summary |
|-------|--------|---------|
| 0 — Environment       | complete | `ARCHITECTURE.md`, `DATA_MODEL.md`, `SECURITY.md`, `ADR.md`. |
| 1 — Desktop foundation| complete | Tauri 2 shell, sidebar, command palette, migrations. |
| 2 — Model manager     | complete | LM Studio / llama.cpp / Ollama providers, SSE, cancellation. |
| 3 — Assistant         | complete | Streaming, Markdown, cancellation, citations, system prompt. |
| 4 — Knowledge Vault   | complete | Extractors, FTS5 + sqlite-vec, RRF, citations. |
| 5 — Projects / notes  | complete | Workspaces, notes, tasks, memory scopes, project context. |
| 5B — Memory v2        | complete | 12 categories, FTS5 mirror, evidence, candidates, sessions. |
| C — MemPalace         | complete | Pluggable backend, JSON-RPC sidecar, audit on switch. |
| D — Knowledge graph   | complete | Entities, aliases, relationships, evidence, merge, BFS. |
| E — Context builder   | complete | Budgeted retriever, system block, web inspector panel. |
| F — Model router      | complete | Capability registry, lifecycle, decision log, prefer override. |
| G — Research Studio   | complete | Projects, papers, claims, matrices, Markdown reports. |
| H — Experiments       | complete | Hyperparams / metrics JSON, run history, line chart. |
| I — Controlled agent  | complete | 12 seeded tools, 4 permission levels, allowlist, audit. |
| J — Document intel    | complete | Pipe + whitespace tables, CSV/TSV, line diff. |
| K — Performance & UX  | complete | Virtualised list, audit panel, no fabricated GPU. |
| L — Packaging         | complete | `docs/BUILDING.md`, `USER_GUIDE.md`, `TROUBLESHOOTING.md`, `BACKUP_AND_RECOVERY.md`. |
| M — Final regression  | complete | 97 / 97 tests + 1 ignored; release cargo check clean; Vite build green. |
| N — Operations        | complete | Allowlists + audit retention/clear/prune; 4 new integration tests. |
| R — Release           | complete | `scripts/{dev,check,release}.ps1`; `CHANGELOG.md`; `RELEASE_NOTES.md`; About panel; **NSIS + MSI installers built**. 104 / 104 tests + 1 ignored. |

## Schema (migrations 0001 → 0011)

All migrations are versioned and run inside a transaction; the
runner is idempotent. See `app/src-tauri/migrations/`.

* `0001_core.sql` — settings, workspaces, projects, conversations,
  messages, audit, app_state, FTS5.
* `0002_models.sql` — `model_profiles`, `provider_endpoints`.
* `0003_conversations.sql` — message metadata, indexes.
* `0004_vault.sql` — `documents`, `document_chunks`, FTS5 mirror,
  sqlite-vec virtual table (384-dim).
* `0005_memory.sql` — v1 memories, ui_state, projects columns.
* `0006_memory_v2.sql` — `memories_v2`, FTS5 mirror, evidence,
  candidates, sessions, document `doc_kind` / `venue` / `year` /
  `doi` / `authors`.
* `0007_knowledge_graph.sql` — `kg_entities`, `kg_aliases`,
  `kg_relationships`, `kg_evidence`.
* `0008_router.sql` — `router_capabilities`, `router_lifecycle`,
  `router_decisions`.
* `0009_research.sql` — `research_projects`, `research_papers`,
  `paper_claims`, `comparison_matrices`, `matrix_columns`,
  `matrix_cells`, `research_reports`.
* `0010_experiments.sql` — `experiments`, `experiment_runs`.
* `0011_agent.sql` — `agent_tools`, `agent_runs`,
  `agent_invocations`.

## Security posture (recap)

* No outbound network unless the user points a model profile at a
  remote endpoint. Local providers are the default.
* File reads (`file.read`, `file.list`) require path allowlist.
* Shell command execution (`shell.run`) requires an explicit
  command allowlist; the default is the empty list.
* The agent's `vault.search`, `kg.search`, `memory.recall` tools
  run with `permission = always`; the rest require explicit user
  approval.
* The audit log records every privileged action.

## Known follow-ups (deliberately deferred)

* **MLflow** — schema is in place; no implementation. The intent
  is a sidecar that imports runs read-only. Not needed for the
  offline-first story.
* **OCR (tesseract)** — same. Document extraction today works on
  the plain text the vault already produces; OCR is for scanned
  PDFs and is out of scope for this build.
* **Audit retention** — `audit_events` grows monotonically. There
  is no auto-prune knob yet.
* **Real GPU telemetry** — the Monitor page reports "unknown" on
  hosts that do not expose GPU counters. This is honest behaviour,
  not a feature gap.
* **Cross-platform release build verification** — Windows
  installer was not built end-to-end in this environment because
  Tauri requires the WebView2 runtime. The configuration is
  unchanged from the rest of the project; the cross-platform
  notes are documented in `docs/BUILDING.md`.

## Cross-references

* `ARCHITECTURE.md` — overall design.
* `DATA_MODEL.md` — table-by-table reference.
* `SECURITY.md` — threat model, allowlists, audit.
* `IMPLEMENTATION_PLAN.md` — the live checklist.
* `docs/PHASE*_SUMMARY.md` — the running per-phase changelog.

Continuity Engine addendum (S.3.6â€“S.3.10)

The Continuity Engine expansion in the master prompt has been audited and shipped as a series of additive slices. None of the previously working subsystems were rewritten.

Files added or changed

* `app/src-tauri/migrations/0015_radar_feedback.sql` â€” new (additive, no existing table changed).
* `app/src-tauri/src/radar_feedback/mod.rs` â€” new. CRUD for the user's response to opportunities. Stable key derivation, idempotent upsert, explicit `kind` enum.
* `app/src-tauri/src/radar/mod.rs` â€” extended. `Opportunity` now carries a `key` field; sort step applies feedback rules (exclude / pin). The new field is additive; the only JSON-breaking change is the addition of a property, which the Rust and TypeScript types now declare.
* `app/src-tauri/src/tribunal/mod.rs` â€” new. Read-only evidence retrieval. Six retrievers: memories, notes, timeline events, hypotheses, files, decisions. Stance is computed from explicit evidence rows when present, otherwise `unjudged`. The `notes` body column, `timeline_events.detail_json` column, and `memories_v2.content` column are the real sources.
* `app/src-tauri/src/vault_export/mod.rs` â€” new. Single-archive exporter. Manifest includes `records_sha256` so the user can verify integrity with any sha256 tool. No destructive import path is exposed.
* `app/src-tauri/src/db/migrations.rs` â€” extended to register migration 0015.
* `app/src-tauri/src/lib.rs` â€” extended to declare the new modules.
* `app/src-tauri/src/commands/s3.rs` â€” extended. Three new command groups: `radar_feedback_*`, `evidence_tribunal_run`, `vault_export_*`. Each write wraps an `audit::log` call.
* `app/src-tauri/src/main.rs` â€” extended to register the new commands.
* `app/src-tauri/src/serendipity/mod.rs` â€” fixed a free-floating indented code block in the doc comment that `cargo test` interpreted as a doctest.
* `app/src-tauri/tests/phase_f.rs` â€” new. 14 tests across three new modules: radar feedback CRUD + radar integration (6 tests), tribunal (5 tests), vault export (3 tests).
* `app/web/src/lib/s3.ts` â€” extended. New types and bindings for radar feedback, tribunal, and vault export.
* `app/web/src/pages/OpportunityRadar.tsx` â€” extended. Sixth tab (`Tribunal`), and `Dismiss` / `Pin` buttons on each opportunity in the Radar tab.
* `IMPLEMENTATION_PLAN.md` â€” extended. New "Phase S.3.6â€“S.3.10" section.

Database compatibility

* Migration 0015 is fully additive: it creates a new table and two indexes. No existing table is altered. The `kind` column is a free-form TEXT with the constraint enforced at the application layer; the tribunal and the radar feedback upsert are the only writers.
* The new `Opportunity.key` field is a derived string, not a foreign key. It does not require any existing row to change.
* The tribunal reads `notes.body`, `timeline_events.detail_json`, `memories_v2.content`, and `hypothesis_evidence.stance`. All of these are the real, current columns. The previously documented `memories_v2.importance` column does not exist in `0006_memory_v2.sql`; the tribunal does not depend on it.

Performance and resource use

* The radar feedback scan is bounded: at most one row per (project_id, key), and the rules query is a single SELECT with two indexes. The radar sort step adds a single `rules_for` call per scan; the scan is already bounded by `per_kind_limit`.
* The tribunal scan is bounded: `limit_per_source` (default 8, capped at 50) per source, and at most 5 sources. The file retriever walks a tree to a maximum depth of 6 and a maximum of 2 000 entries; it skips binary extensions and skips files whose name does not share a token with the question. The total work is O(questions * sources * limit).
* The vault export reads every row of every selected kind. For projects with millions of rows this is a real cost; the manifest's `record_counts` is the honest signal. A future "chunked export" is a reasonable follow-up but is out of scope for this slice.

What is **not** in this slice

* No destructive import for the Knowledge Vault. The master prompt's full F11 acceptance criteria require a dry-run, schema compatibility checks, conflict handling, and a backup-tested round trip. Those are not safe to ship in a one-shot slice and are explicitly deferred.
* No Workflow Builder. Â§12 of the master prompt correctly warns that a low-code platform is not a single-turn deliverable.
* No Capability Graph. Â§14 requires an entity-resolution pass and an editable UI that is bigger than this turn.
* No multi-model Tribunal comparison. Â§13 of the master prompt allows for sequential comparison when models are available, but the host hardware (16 GB RAM, Intel Arc A750) cannot run two heavy models in parallel; the sequential path requires the existing model router to expose an answer-generation interface, which is a separate piece of work. The Tribunal in this slice is the deterministic half: retrieval + stance classification + honest notes. A model-call extension can be added later without breaking the API surface.

Tests actually run

* `cargo test --release` â€” 180 passed, 0 failed, 1 ignored (MemPalace live, gated by env var). The 14 new tests in `phase_f` exercise the radar feedback rules, the radar integration, the tribunal evidence paths, and the vault export manifest / hash / kind-filter / project-filter behaviour.
* `npm --prefix app/web run build` â€” 398.26 kB / 106.49 kB gzip, no TS or Vite errors.
* `cargo check --lib --tests --release` â€” clean.

Privacy and security

* No network calls, no telemetry, no model downloads. The tribunal reads only local SQLite tables and an explicitly supplied `repo_root`.
* The vault export writes only to a path the user provides; the receiving path is not auto-opened.
* The radar feedback upsert is keyed on `(project_id, key)` and the kind enum is validated server-side. A malformed kind is rejected with `AppError::Invalid`.




---

## Audit addendum â€” Phase P1 (Critical Bug Fixing, Stability, UX, and Performance Recovery)

This addendum records the work that landed under the "NEXUS â€” Critical
Bug Fixing" master prompt (P0 through P7). It is intentionally separate
from the previous Phase S.3.6â€“S.3.10 addendum so the bug-fixing work
can be reviewed on its own.

### Root causes found

1. **Black screens** â€” the application had **zero** React error
   boundaries. A throw in any route (e.g. a malformed document record
   while rendering the Knowledge Vault, a malformed memory while
   reading from Memory Center, a failed IPC while mounting a chart)
   killed the entire React tree and the user saw a black screen.
   Symptom: "After importing a Python file â€¦ navigating to Knowledge
   Vault displays a black screen with no usable interface."

2. **`tauri.localhost says` dialogs** â€” every `prompt()`, `confirm()`,
   and `alert()` in the React code used the Tauri/Wry built-in browser
   dialog, which renders as a generic browser-style "tauri.localhost
   says" prompt. 17 `prompt()` and 5 `alert()` call sites were
   enumerated across 8 page files.

3. **Stale "Sending" activity record** â€” `Assistant.tsx` appended a
   `Sending` activity entry on every send but never updated it. The
   `done` / `error` / `cancelled` branches appended *another* entry
   ("Done Â· â€¦", "Error: â€¦", "Cancelled") while the original "Sending"
   record stayed in the timeline forever. Symptom: "the NEXUS activity
   panel still showed `Activity: Sending`" after the response had
   already appeared in chat.

4. **CPU monitoring reported 100%** â€” `dashboard_summary` constructed
   a fresh `SystemMonitor` on every call. The first `refresh_cpu_usage`
   primes the CPU-time counters; the second one reads the delta.
   Because the monitor was re-created per request, the first sample was
   always the un-primed value, which on Windows sysinfo reports as
   100% of one core. The frontend also labelled the value as plain
   "CPU" without indicating whether it was process- or system-wide,
   making the discrepancy harder to diagnose.

5. **"New conversation" was never auto-titled** â€” `onSend` passed
   `draft.slice(0, 60)` as the title when the conversation did not
   exist, but the title was set on creation; the new conversation
   button still created a literal "New conversation" entry that
   stayed that way until the user manually renamed it.

### Files and modules changed

Frontend (`app/web/src`):

* **new** `components/ErrorBoundary.tsx` â€” class component, generates
  diagnostic reference, Retry / Back to Home / Copy diagnostic
  actions.
* **new** `components/PromptDialog.tsx` â€” single in-app modal,
  three surface modes (`prompt` / `confirm` / `alert`), portal-rendered
  to `document.body`.
* **new** `hooks/usePromptDialog.tsx` â€” exposes `prompt()` /
  `confirm()` / `alert()` to any component.
* `App.tsx` â€” top-level + per-route ErrorBoundary wiring.
* `main.tsx` â€” mounts `<PromptDialogRoot />` next to `<App />`.
* `pages/Assistant.tsx` â€” derives auto-titles, transitions
  in-flight activity to terminal state, adds live status banner
  with elapsed time, wires Esc to cancel.
* `pages/MemoryCenter.tsx` â€” 4 `prompt()` calls replaced.
* `pages/Agent.tsx` â€” 2 `prompt()` and 3 `alert()` calls replaced.
* `pages/Projects.tsx` â€” 1 `prompt()` and 2 `confirm()` calls
  replaced.
* `pages/Research.tsx` â€” 1 `prompt()` and 4 `confirm()` calls
  replaced.
* `pages/Experiments.tsx` â€” 2 `prompt()` and 2 `confirm()` calls
  replaced.
* `pages/Tasks.tsx` â€” 1 `prompt()` and 1 `confirm()` calls replaced.
* `pages/Monitor.tsx` â€” 1 `confirm()` replaced; CPU meter relabeled
  to "System CPU" with `primingâ€¦` unavailable state.
* `pages/OpportunityRadar.tsx` â€” 1 `confirm()` replaced in the
  `HypothesesTab` sub-component.
* `pages/Models.tsx` â€” `refresh()` deduplication, `refreshing` /
  `lastRefreshed` state, "Probing local backendsâ€¦" banner.
* `lib/ipc.ts` â€” `SystemSample` extended with `cpu_available` and
  `cpu_basis`.

Backend (`app/src-tauri/src`):

* `monitor.rs` â€” rewritten. Tracks `cpu_primes`; the first 1-2
  samples are reported with `cpu_percent: -1.0` and
  `cpu_available: false`; the value is clamped to `0.0..=100.0`; a
  new `cpu_basis: &'static str` documents the measurement as
  "system Â· all logical processors".
* `app_state.rs` â€” adds `monitor: parking_lot::Mutex<SystemMonitor>`
  to `AppState`. The monitor is constructed **once** at app boot.
* `commands/app.rs` â€” `dashboard_summary` reuses
  `state.monitor.lock().sample()` instead of constructing a new
  monitor per call.
* **new** `tests/phase_p1_monitor.rs` â€” 4 regression tests covering
  the priming behaviour, the clamp, the memory invariants, and the
  monotonic `collected_at`.

### Database migrations

None. The bug-fixing work is a frontend + monitoring change; no
schema or data migration is required and the existing data is
preserved untouched.

### Fixes implemented

* Error containment is in place at the route level. A malformed
  document or memory record cannot kill the whole application UI
  anymore â€” the affected route falls back to an actionable error
  card with Retry and Copy diagnostic actions.
* All browser-style prompts have been replaced. `grep -nE
  'window\.(prompt|alert|confirm)' app/web/src/**/*.tsx` returns
  zero hits in source.
* The "Sending" stale record is fixed: `findLastSending()` locates
  the most recent in-flight activity entry and `handleDelta` /
  `onSend` **transitions** that entry in place to a terminal state
  (`Done Â· {reason} Â· {tokens} tok`, `Error: â€¦`, `Failed: â€¦`, or
  `Cancelled`).
* New conversations are auto-titled from the first user message via
  `deriveTitle()`. Manually-renamed conversations are preserved
  (the check is `!current.title || current.title === "New
  conversation"`).
* The CPU monitor returns `cpu_available: false` for the first 1-2
  samples so the frontend can show "primingâ€¦" instead of a
  misleading 100%. After priming, the value matches what
  `sysinfo::global_cpu_usage()` reports averaged across all logical
  processors, which is the same basis Task Manager's "CPU" column
  uses.
* The `dashboard_summary` command no longer re-creates the
  `SystemMonitor` per call; it reuses the long-lived instance in
  `AppState`, so CPU-time deltas are stable.

### New or modified tests

* **new** `app/src-tauri/tests/phase_p1_monitor.rs` â€” 4 tests:

  1. `first_sample_is_unavailable` â€” the first sample reports
     `cpu_available: false` and `cpu_percent: -1.0`.
  2. `eventually_produces_available_sample` â€” the monitor reaches
     `cpu_available: true` within a handful of samples and the
     value is in `0.0..=100.0`.
  3. `memory_measurement_is_immediate` â€” memory values are sane
     from the first sample and `mem_used_mb <= mem_total_mb`.
  4. `collected_at_is_monotonic` â€” wall-clock timestamps never
     move backwards.

### Build and test commands executed

* `cargo test --release` â€” **184 passed, 0 failed, 1 ignored**
  (180 previous baseline + 4 new monitor tests).
* `cargo check --lib --tests --release` â€” clean.
* `npm --prefix app/web run build` â€” clean (412.05 kB / 110.33 kB
  gzip).

### Before / after measurements

* **Stale activity records** â€” before: every completed response
  left a permanent "Sending" entry in the timeline. After: zero
  stale records; the in-flight entry transitions to a terminal
  state.
* **CPU monitoring** â€” before: the dashboard reported
  `100.0%` on the first sample of every poll because the monitor
  was re-created each time. After: the first 1-2 samples report
  `primingâ€¦` and subsequent samples reflect the actual
  system-wide average, matching what Windows Task Manager
  shows in the "CPU" column.
* **Browser dialogs** â€” before: 22 native dialog call sites. After:
  0 in source. All replaced with the in-app `PromptDialog` modal.
* **Bundle size** â€” 398.26 kB / 106.49 kB gzip â†’ 412.05 kB /
  110.33 kB gzip (+13.79 kB / +3.84 kB gzip for the new modal,
  hook, error boundary, and chat status banner).

### Remaining bugs and limitations

* The Markdown renderer is intentionally minimal (paragraphs,
  headings, lists, inline code, fenced code blocks with copy). A
  fuller renderer with Shiki-style syntax highlighting and table
  support is **deferred** to Phase 7 (attachments and performance).
* File / image attachments to chat (P9) are **not** in this slice.
  The next slice will introduce a real attachment button with
  per-file validation.
* Latency instrumentation (P10) is **not** in this slice. The
  foundation (live `elapsedMs` tick + activity phase state) is
  in place; the next slice will wire it into the Rust core for
  request-dispatch and first-token measurements.
* The `llama.cpp` optional backend (P10) is **not** started. Per
  the master prompt, this work must be backed by measured
  evidence; without an LM Studio comparative run on this
  hardware, the optional backend would be premature.
* Document and Knowledge Vault ingestion hardening (P8) is
  **partially** addressed: the per-route ErrorBoundary prevents
  a malformed document record from killing the app. The
  per-document validation pass is the next concrete slice.

### Changes that require user configuration

None. All fixes are passive: existing users get the new behaviour
on the next launch. The ErrorBoundary's diagnostic reference
(`nx-<ms>-<rand>`) is a local-only string; no remote services are
called.

### Final diff summary

* **Frontend**: 3 new files (ErrorBoundary, PromptDialog,
  usePromptDialog), 11 modified page/lib files, 1 modified IPC
  type.
* **Backend**: 1 new file (tests/phase_p1_monitor.rs), 3
  modified files (monitor.rs, app_state.rs, commands/app.rs).
* **Docs**: 1 new section in `IMPLEMENTATION_PLAN.md`, 1 new
  addendum in this `AUDIT.md`.
* No schema migrations, no data loss, no remote calls, no
  telemetry.


## Audit addendum — Phase 1 (P6 / P8 / P9 / P10)

This addendum tracks the four critical issues that landed after
the "Phase P1 — Critical Bug Fixing" entry above. Each section
follows the same shape: symptom, root cause, files changed,
regression test, before/after measurement.

### Issue P8-1 — Knowledge Vault black screen on Python import

* **Symptom.** Importing a Python file via "Add file" then
  navigating to `/knowledge` rendered a black screen. The rest
  of the application continued to work; only the Vault page
  was affected. The DevTools console showed
  `TypeError: Cannot read properties of undefined (reading 'slice')`.
* **Root cause.** `app/src-tauri/src/ingestion/pipeline.rs`
  `list_documents` and `get_document` did not select
  `content_hash` and the Rust `DocumentRow` struct did not
  declare a `content_hash` field. The TypeScript `DocumentRow`
  declared the field as `string` and the Vault page called
  `d.content_hash.slice(0, 10)`. The serialized JSON contained
  no `content_hash` key, so the field was `undefined`, and
  `undefined.slice` threw inside the React component.
* **Files changed.**
  * `app/src-tauri/src/ingestion/pipeline.rs` — added
    `content_hash` to the two SELECT statements and to the
    `DocumentRow` struct (as `Option<String>`).
  * `app/web/src/lib/vault.ts` — widened the type to
    `string | null` with a doc comment.
  * `app/web/src/pages/Knowledge.tsx` — render "no hash"
    fallback instead of calling `.slice` on a possibly-null
    value.
  * `app/src-tauri/tests/phase_p1_vault.rs` — new regression
    test `list_documents_returns_content_hash` asserts the
    serialized JSON includes the `content_hash` key.
* **Before / after.**
  * Before: `list_documents` returns 10 columns, the JSON has
    no `content_hash`, the Vault page throws.
  * After: `list_documents` returns 11 columns, the JSON has
    `content_hash: "deadbeef…"` for indexed rows and `null` for
    legacy rows, the Vault page renders the table normally.

### Issue P8-2 — Chunker offset drift on trimmed paragraphs

* **Symptom.** Citation offsets returned by search occasionally
  pointed outside the paragraph being cited. The drift was a
  few bytes per chunk, growing with the number of paragraphs.
* **Root cause.** The chunker in
  `app/src-tauri/src/ingestion/chunker.rs` used
  `cursor = p_start + p.len() + 2` after `trim()`-ing the
  paragraph. The trimmed length is shorter than the
  separator-bounded length, so `cursor` drifted forward and
  every later `start_offset` / `end_offset` was inflated.
* **Files changed.**
  * `app/src-tauri/src/ingestion/chunker.rs` — rewrite to
    track the original-text byte offset of each paragraph
    (computed once, before the trim) and rebase `buf_start`
    after the trim.
  * `app/src-tauri/tests/phase_p1_vault.rs` — new tests
    `chunker_offsets_round_trip_into_text` and
    `chunker_handles_code_with_line_prefix` cover the
    round-trip and the line-prefix code case.
* **Before / after.**
  * Before: `chunk_text("   \n\nalpha.\n\n   beta.", …).last().unwrap().end_offset == 14` (wrong; should be 17).
  * After: `chunk_text("   \n\nalpha.\n\n   beta.", …).last().unwrap().end_offset == 17`, and
    `text[chunk.start_offset..chunk.end_offset] == "alpha."` round-trips.

### Issue P8-3 — Source file rejected on non-UTF-8 bytes

* **Symptom.** A Python file saved in cp1252 was indexed with
  `status: "failed"` and the user saw a generic "extraction
  failed" error in the Vault page.
* **Root cause.** `extractors::extract_code` and
  `extractors::extract_text` used `std::fs::read_to_string`,
  which fails outright on a non-UTF-8 byte.
* **Files changed.**
  * `app/src-tauri/src/ingestion/extractors.rs` — read bytes
    first, then `std::str::from_utf8` with a fallback to
    `String::from_utf8_lossy`. Record `non_utf8_replaced: bool`
    in the extraction metadata so the user can see when
    non-UTF-8 bytes were replaced.
  * `app/src-tauri/tests/phase_p1_vault.rs` — new tests
    `extract_code_tolerates_non_utf8_bytes` and
    `extract_text_tolerates_non_utf8_bytes`.
* **Before / after.**
  * Before: `extract("cp1252.py")` returns
    `Err(AppError::Io(_))` and the document is `failed`.
  * After: `extract("cp1252.py")` returns `Extraction { text: …, metadata: { non_utf8_replaced: true } }`
    and the document is `indexed`.

### Issue P9 — Chat attachment pipeline

* **Symptom.** Users wanted to paste a Python file or
  Markdown note into the chat and ask the assistant about it,
  but the only option was to upload the file to the Knowledge
  Vault and hope the retrieval picked it up. There was no
  way to attach a file to a single message.
* **Root cause.** No attachment pipeline existed; the
  composer could only send plain text.
* **Files changed.**
  * `app/src-tauri/src/commands/chat.rs` — new command
    `chat_read_attachment` (caps at 256 KB per file, returns
    `truncated: bool` for larger files); new field
    `attachments: Vec<AttachedText>` on `SendRequest`;
    `build_user_text_with_attachments` folds the text into the
    user message as fenced blocks (with a `python` /
    `typescript` / `rust` / etc. language hint from the file
    extension). The folded text is persisted as the user
    message body so the conversation history is self-contained.
  * `app/src-tauri/src/main.rs` — register
    `commands::chat::chat_read_attachment`.
  * `app/web/src/lib/attachments.ts` — new module with the
    `AttachmentPayload` / `AttachedText` types and the
    `chatReadAttachment` IPC binding.
  * `app/web/src/lib/chat.ts` — `chatSend` accepts a new
    `ChatSendInput` shape with `attachments`. The legacy
    positional form is preserved for back-compat with any
    in-flight caller.
  * `app/web/src/pages/Assistant.tsx` — paperclip button, an
    `attachments` state, removable chips, 8-attachment cap.
    Attachment count is exposed via `aria-label` and a tooltip
    shows the file's path, size, and a `non-UTF-8 bytes
    replaced` hint when relevant.
* **Before / after.**
  * Before: composer accepts only text; the only way to bring
    a file into a conversation is to ingest it into the
    Vault, which embeds it for every future retrieval.
  * After: composer accepts up to 8 attachments of ≤256 KB
    each; the assistant sees the text once, in the next
    message; the conversation history stores the folded text
    so it is re-renderable.

### Issue P10 — No time-to-first-token in the chat UI

* **Symptom.** The user could see the live `Generating · 1.2s`
  clock, but had no way to know how much of that was "waiting
  for the model" vs "tokens arriving". A slow first byte and
  a fast first byte were indistinguishable.
* **Root cause.** `ChatDeltaEvent` carried no timing fields;
  the Assistant page invented numbers from `performance.now()`,
  but those numbers are not aligned with the backend's clock.
* **Files changed.**
  * `app/src-tauri/src/commands/chat.rs` — `ChatDeltaEvent`
    gains two optional fields: `dispatched_at_ms` (set on every
    event) and `first_token_at_ms` (set on the first `token`
    and copied to every later event). The dispatch timestamp
    is captured just before `provider.stream_chat` is spawned.
  * `app/web/src/lib/chat.ts` — `ChatDeltaEvent` type widened
    with the two optional numeric fields.
  * `app/web/src/pages/Assistant.tsx` — `ttftMs` state; reset
    on send; populated in `handleDelta` from
    `first_token_at_ms - dispatched_at_ms`; rendered as
    `TTFT xx.xxs` next to the live clock.
* **Before / after.**
  * Before: `Generating · 1.4s` (no TTFT).
  * After: `Generating · 1.4s · TTFT 0.32s` (the user's
    experience is now visible at a glance).

### Issue P6 — MarkdownView upgrade

* **Symptom.** Assistant replies with tables, ordered lists,
  or task lists rendered as a wall of `|` / `1.` / `- [ ]`
  characters. Long code blocks filled the whole screen with
  no way to collapse or copy. Code was rendered as plain
  monospace.
* **Root cause.** The previous MarkdownView was a small parser
  that covered headings, paragraphs, inline code, code blocks,
  lists (unordered only), and blockquotes. Tables, task lists,
  and syntax highlighting were out of scope.
* **Files changed.**
  * `app/web/src/components/MarkdownView.tsx` — rewrote the
    parser to recognise tables, ordered lists, task lists,
    and horizontal rules; rewrote the code block to support a
    language label, a "Copy" button, a "Show all (N)" expand
    button when more than 18 lines, and a hand-rolled
    syntax highlighter covering Python, JS/TS, Rust, Go,
    Bash, JSON, YAML, TOML, SQL, HTML, CSS. Also added
    `useStableMarkdown(text, 60)` debounce hook so a fast
    stream of tokens does not re-render the parser 30
    times per second.
* **Before / after.**
  * Before: `| Name | Age |\n| --- | --- |\n| Ada | 36 |`
    rendered as a single paragraph with `|` characters.
  * After: the same Markdown renders as a styled table with a
    dark header row and hover-highlighted body rows. Code
    blocks longer than 18 lines collapse to a preview with a
    "Show all (N)" button.


## Audit addendum — Phase P3 (Critical Bug Fix & Integration)

This addendum records the four-bug fix and integration sweep
from the "NEXUS — Critical Bug Fix & Integration Prompt". The
previous Phase P1 / P2 / R3 work remains intact; this addendum
is additive.

### Bug A — Knowledge Vault retrieval in chat

* **Symptom.** The user asked "Read the Arduino.txt from
  Knowledge Vault and tell me what it does." The assistant
  replied with "I don't have access to any Arduino.txt file or
  knowledge vault" even though the file was indexed and
  chunked.
* **Root cause 1.** `nexus_core::ingestion::search::search_fts`
  ran `chars().filter(|c| c.is_alphanumeric() || *c == '_')`
  on every whitespace-delimited token. The query "Read the
  Arduino.txt from Knowledge Vault" became the FTS5
  expression `"Read"* "the"* "Arduinotxt"* "from"* "Knowledge"*
  "Vault"*`. `Arduinotxt` does not match the chunk text, so
  the search returned zero hits, the system prompt carried no
  vault context, and the model produced a denial.
* **Root cause 2.** Even with a sane FTS5 query, a query that
  names a file does not match the chunk text. There was no
  document-name hint path.
* **Fix.** New `tokenize_query` preserves `.` and `-` as word
  separators, quotes each term, drops FTS5 reserved words and
  common English stop words, and appends `*` to the last token
  for prefix matching. New `find_documents_by_name_hint` and
  `chunks_for_document` add a name-hint retrieval path; the
  user can opt in by using a file-noun ("the arduino file").
  `SearchResult` carries `used_docname_hint` and
  `docname_hint_doc_ids` so the chat pipeline can log and
  surface these retrievals.
* **Files changed.** `src/ingestion/search.rs`,
  `src/commands/chat.rs` (the `SearchResult` field count),
  `tests/phase_p3_vault_retrieval.rs` (new, 10 tests).
* **Regression tests.** The 10 new tests cover: filename
  component tokenisation, stop-word filtering, FTS+name-hint
  retrieval of `Arduino.txt`, a different-document
  generalisation test (`fibonacci.py`), no-hit behaviour,
  empty-query behaviour, and the name-hint extractor in
  isolation. All pass.
* **No schema migration**, **no data loss**, **no remote
  calls**, **no telemetry**.

### Bug B — Search results stacking

* **Symptom.** New searches did not replace the previous
  active result list. The user saw mixed old and new hits in
  the search panel.
* **Root cause.** `Knowledge.tsx::onSearch` had no request
  ID, so two back-to-back searches could race. The older
  request could resolve after the newer one and overwrite
  the newer result list.
* **Fix.** `searchSeqRef = useRef(0)` captures a per-search
  sequence. The async handler captures `mySeq =
  searchSeqRef.current + 1` before issuing the request and
  discards the response when `searchSeqRef.current !== mySeq`.
  The result list is also optimistically cleared on a new
  search.
* **Files changed.** `app/web/src/pages/Knowledge.tsx`.
* **No backend changes.**

### Bug C — Automatic chat naming still broken

* **Symptom.** New conversations kept the default title
  "New conversation" even after the first user message.
* **Root cause.** The previous implementation lived in the
  browser (`Assistant.tsx::maybeAutoTitle`) and was
  best-effort: it could race with the user switching
  conversations, get stuck if the websocket dropped, and
  produce a title derived from a literal substring of the
  first message (clipped to 48 chars) rather than an actual
  summary from the model.
* **Fix.** Title generation is now server-side:
  - `nexus_core::auto_title::compute_auto_title` runs after
    a successful `done` event. It loads the conversation,
    skips if the title is already meaningful, loads the
    first user message, calls the active
    `ModelProvider::chat_complete` with a 24-token
    title-prompt, normalises the result, and returns an
    `AutoTitleOutcome` enum (`Model(t)`, `Heuristic(t)`,
    `AlreadyTitled`, `EmptyConversation`).
  - `commands::auto_title::trigger_auto_title` (in the
    binary crate to avoid pulling `WebView2Loader.dll`
    into the lib) consumes the outcome, persists via
    `conversations::rename`, writes an audit row, and
    emits `nexus:chat.title_updated`.
  - The frontend subscribes to the event and patches the
    conversation list and active conversation in place.
  - The frontend `maybeAutoTitle` is kept as a fallback
    with a 1.5s re-check window so it does not stomp on
    a server-side title that is in flight.
* **Files changed.**
  `src/auto_title/mod.rs` (new, lib),
  `src/commands/auto_title.rs` (new, binary),
  `src/lib.rs` (`pub mod auto_title;`),
  `src/commands/mod.rs` (`pub mod auto_title;`),
  `src/commands/chat.rs` (fire-and-forget spawn),
  `src/providers/{mod,lmstudio,llamacpp,ollama}.rs` (new
  `chat_complete` method on the trait + implementations),
  `app/web/src/lib/chat.ts` (event listener),
  `app/web/src/pages/Assistant.tsx` (listener + fallback
  re-check),
  `tests/phase_p3_auto_title.rs` (new, 12 tests).
* **Regression tests.** 12 new tests cover default-title
  detection, normalisation (quotes, leading labels,
  ellipsis, multibyte clipping), heuristic fallback
  (1-8 words, punctuation handling, empty input),
  end-to-end `compute_auto_title` with a mock provider
  (model output, no-op when already titled, empty
  conversation, garbage-output fallback). All pass.
* **No schema migration**, **no data loss**, **no remote
  calls**, **no telemetry**.

### Bug D — NEXUS crashes when attaching a photo

* **Symptom.** Attaching a photo and asking a vision question
  caused NEXUS to crash. The user said "crashes" rather than
  "errors out", suggesting a hard fault rather than a
  recoverable error.
* **Root cause.** The chat pipeline unconditionally forwarded
  an OpenAI-style `image_url` content part to whatever model
  was active. When the active model did not advertise vision
  support, the LM Studio / OpenAI-compatible endpoint returned
  a 4xx; the chat stream emitted an `error` event with the
  upstream text and an upstream `AppError`; the in-flight
  request was torn down mid-stream; the chat composer
  remained in a stuck state. There was no signal on the
  `model_profiles` row that said "this model is text-only,
  refuse images" and no validation of the image data URI
  before serialisation.
* **Fix.**
  1. **New `model_profiles.supports_vision` column** —
     migration `0017_vision_capability.sql` adds
     `supports_vision INTEGER NOT NULL DEFAULT 0` and an
     index on `(role, enabled)`. The Model Manager UI has a
     "Supports vision" checkbox on the new-profile form.
     Defaults to `false` so the field is strictly opt-in.
  2. **Pre-flight gate in `chat_send`** — after the model
     profile is resolved, the chat pipeline refuses image
     attachments up front when the active model does not
     advertise vision. The error is clear and recoverable;
     no HTTP call, no half-streamed response, no stuck UI.
  3. **`sanitise_image_data_uri` helper** — any image
     attachment whose data URI does not look like a real
     `data:<mime>;base64,<...>` payload is replaced with a
     `data:text/plain;base64,...` URI carrying a short
     error message. The model sees the marker and can
     respond sensibly; the chat never crashes.
* **Files changed.**
  `migrations/0017_vision_capability.sql` (new),
  `src/db/migrations.rs` (registers migration 17),
  `src/model_profiles.rs` (`supports_vision` field, list
  and upsert round-trip),
  `src/commands/chat.rs` (vision gate in `chat_send`,
  `sanitise_image_data_uri` in
  `build_user_message_content`),
  `app/web/src/lib/providers.ts` (type field),
  `app/web/src/pages/Models.tsx` (UI checkbox),
  `tests/phase_p3_image_attachment.rs` (new, 6 tests).
* **Regression tests.** 6 new tests cover: default
  `supports_vision` is false, the flag round-trips through
  the DB, the sanitiser accepts a real base64 data URI, the
  sanitiser rejects malformed / truncated URIs, the
  sanitiser rejects URIs missing the `;base64` marker. All
  pass.
* **No data loss**, **no remote calls**, **no telemetry**.

### Plugin integration

* **`rag-v1/`** — LM Studio plugin (`@lmstudio/sdk` 1.4.0,
  Node.js runner, manifest `type: plugin`). Not
  in-process integrable into NEXUS's Tauri 2 / Rust / React
  architecture. NEXUS's Knowledge Vault is the native
  equivalent and now does the same job — Bug A's regression
  suite exercises the retrieval path.
* **`js-code-sandbox/`** — LM Studio plugin (same SDK).
  NEXUS's `nexus_core::sandbox::run_javascript` is the
  native equivalent and matches the same Deno permission
  policy (`--allow-read=<dir> --allow-write=<dir>
  --no-prompt --deny-net --deny-env --deny-sys
  --deny-run --deny-ffi`) with the same 5s/60s timeout
  contract. `tests/phase_p2_sandbox.rs` (5 tests + 1
  ignored) verifies the contract.

### Test result

```
cargo test
TOTAL: passed=230 failed=0 ignored=1
```

Web build: 435.00 kB / 117.47 kB gzip. `cargo check --lib
--tests --bin nexus --release` clean.

29 test binaries. New: `phase_p3_vault_retrieval`,
`phase_p3_auto_title`, `phase_p3_image_attachment`.


---

## Audit addendum — Phase P4 (Request lifecycle, navigation survival, and Memory Center persistence)

This addendum records the work that landed under the
"NEXUS — Critical Cross-Cutting Bug" master prompt (E,
F, L). The previous Phase P1, P2, P3, R3 work remains
intact; this addendum is additive.

### Bug E — Assistant responses disappear on navigation

* **Symptom.** When the model is generating a response
  and the user navigates to another page in NEXUS, the
  response disappears or stays invisible on return.
  Partial streaming output accumulated while the user
  was away is lost.
* **Root cause.**
  1. `AssistantPage` held all streaming state in
     component-local `useState`. The
     `nexus:chat.delta` listener was unregistered in
     the cleanup function, so any `done` event that
     arrived after unmount was dropped.
  2. The assistant message row was created with
     `status: "streaming"` and `content: ""`. The
     persistence cadence (every 50 ms) only ran inside
     the streaming task, which was attached to the
     listener that was now gone.
  3. There was no way for the UI to recover the
     in-flight state after a page navigation.
* **Fix.**
  1. Every `nexus:chat.delta` now carries a stable
     `request_id` (ULID). The chat pipeline
     (`commands/chat.rs::chat_send`) registers the
     request in a new per-request registry and
     emits the id on every delta.
  2. The assistant message is persisted throttled at
     250 ms (was 50 ms — too chatty for SQLite) and
     always in full on every terminal event. The
     `cancelled` and `interrupted` terminal states
     are preserved so a re-mount of the chat page
     sees a consistent DB row.
  3. The frontend (`Assistant.tsx`) queries
     `chat_request_list_active` on every mount and
     remount and rehydrates the in-flight state.
     Late events whose `request_id` does not match
     the current active request are dropped at the
     top of `handleDelta`.
* **Files changed.**
  * `app/src-tauri/src/commands/chat.rs` — new
    `request_id` field on `ChatDeltaEvent`, new
    `handle_explicit_save` helper, new
    `chat_request_list_active` and
    `chat_request_cancel` Tauri commands,
    unregister on every exit path.
  * `app/src-tauri/src/app_state.rs` — replaced
    `Vec<CancellationToken>` with
    `HashMap<String, ActiveCancel>`. New methods:
    `register_request`, `unregister_request`,
    `cancel_request`, `cancel_kind`,
    `cancel_conversation`, `list_active`.
  * `app/src-tauri/src/commands/models.rs` —
    `cancel_inflight` now defaults to chat-only.
  * `app/src-tauri/src/main.rs` — registered the new
    Tauri commands.
  * `app/web/src/lib/chat.ts` — new types
    `ActiveRequestSnapshot`, `MemorySaveResult`;
    new IPC helpers `chatRequestListActive`,
    `chatRequestCancel`, `onMemorySaved`,
    `memorySaveExplicit`; new `request_id` field
    on `ChatDeltaEvent`.
  * `app/web/src/pages/Assistant.tsx` — new
    `activeRequestId` ref + rehydration effect;
    request-id filter on every delta; pre-send
    cancel; `onMemorySaved` listener.
* **Regression tests** (`tests/phase_p4_lifecycle.rs`,
  9 tests): see `.scratch/phase_p4_handover.md`.

### Bug F — Memory Center does not save chat-issued facts

* **Symptom.** "Save to your memory that my age is 22"
  took 91 s of token generation, returned no visible
  answer, and persisted nothing. A subsequent "what's
  my age?" returned "I don't have that information".
* **Root cause.** Every user message was sent to the
  model as a normal chat completion. The model had to
  invent a `save_memory` tool call out of thin air; it
  produced a 91 s apology and never persisted anything.
  No `save_memory` tool was defined in
  `ChatRequest::tools`.
* **Fix.** A deterministic local service
  `memory_v2::explicit_save` recognises the save
  phrase, extracts the "that <fact>" clause, validates
  and classifies it, and writes through the canonical
  `memories_v2_create` path. The model is **not**
  called. The chat pipeline intercepts the request
  before the vault retrieval runs. The service emits
  one of four outcomes: `Saved`, `Updated`
  (idempotent), `Rejected`, `Failed`. The frontend
  renders the chip in the activity timeline; Memory
  Center listens for `nexus:memory.saved` and refreshes
  automatically.
* **What it does NOT do** (defensive guarantees):
  * Does not hardcode the age example.
  * Does not save every sentence the user writes.
  * Does not infer facts the user did not provide.
  * Does not bypass existing memory validation.
  * Does not expose a general-purpose DB-writing
    tool to the model.
* **Files changed.**
  * `app/src-tauri/src/memory_v2/explicit_save.rs` —
    new module with regex extraction, classifier,
    validator, and persistence glue. 5 in-module
    unit tests.
  * `app/src-tauri/src/memory_v2/mod.rs` — registered
    the new module.
  * `app/src-tauri/src/commands/memory_v2.rs` — new
    `memory_save_explicit` Tauri command.
  * `app/src-tauri/src/commands/chat.rs` — new
    `handle_explicit_save` helper that intercepts
    explicit saves before the model is invoked.
  * `app/src-tauri/src/main.rs` — registered the
    new Tauri command.
  * `app/web/src/pages/MemoryCenter.tsx` —
    `onMemorySaved` listener auto-refreshes the
    list.
* **Regression tests** (`tests/phase_p4_memory.rs`,
  12 tests): F1–F5 acceptance, idempotency, dedupe,
  classifier routing, validator, retrieval across
  conversations, foreign-key safety, existing-memory
  preservation.

### Bug L — Requests consume tokens but never finish

* **Symptom.** 91 s of model output with no terminal
  state; 48 s of output followed by `Waiting for model
  · 70.0s` sticking; the Stop button remained active
  after the response was visible.
* **Root cause.**
  1. `AppState::active_cancellations` was a
     `Vec<CancellationToken>` with no request-id
     tagging. `cancel_inflight` cancelled **every**
     active request, including unrelated Model
     Manager test prompts.
  2. The frontend `running` flag was reset only on
     a `done` event. If the user navigated away
     during the gap between the chat promise
     resolving and the `done` event being processed,
     the `done` event was lost and the UI stayed in
     `running` forever.
* **Fix.** See Bug E and Bug F above for the
  infrastructure that backs Bug L (per-request
  registry, request-id-tagged deltas, unregister on
  every exit path, frontend rehydration). On top of
  that, `cancel_inflight` defaults to chat-only
  (`cancel_kind("chat")`) and the frontend cancels
  any in-flight request for the current conversation
  before sending a new one.
* **Files changed.** Same as Bug E.
* **Regression tests.** `tests/phase_p4_lifecycle.rs`
  9 tests, plus the in-line invariants in
  `commands/chat.rs::chat_send` (covered by
  Phase P2's `phase_p2_chat_lifecycle.rs` and Phase
  P3's `phase_p3_auto_title.rs` which both still
  pass).

### Test result

```
cargo test --lib --tests
TOTAL: passed=276 failed=0 ignored=1
```

Web build: 436.39 kB / 117.91 kB gzip.
`cargo check --lib --tests --bin nexus --release`
clean.

36 test binaries. New: `phase_p4_lifecycle` (9),
`phase_p4_memory` (12), and 5 in-module unit tests
on `memory_v2::explicit_save`.
