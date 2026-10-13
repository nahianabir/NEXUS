# Phase M — Final Regression

## Test results

```
$ cargo test --lib --tests
test result: ok. 16 passed   (nexus-core lib unit tests)
test result: ok.  0 passed   (main)
test result: ok.  0 passed   (test common)
test result: ok.  1 passed   (live_chat)
test result: ok.  1 passed   (live_lmstudio)
test result: ok.  2 passed   (phase1)
test result: ok.  8 passed   (phase2)
test result: ok.  3 passed   (phase3)
test result: ok.  5 passed   (phase4)
test result: ok.  1 passed   (phase4_live)
test result: ok.  9 passed   (phase5)
test result: ok. 10 passed   (phase5b)
test result: ok.  6 passed; 1 ignored  (phase_c)
test result: ok.  8 passed   (phase_d)
test result: ok.  5 passed   (phase_e)
test result: ok.  8 passed   (phase_f)
test result: ok.  5 passed   (phase_g)
test result: ok.  2 passed   (phase_h)
test result: ok.  5 passed   (phase_i)
test result: ok.  2 passed   (phase_j)
```

**Totals: 97 tests passing, 1 ignored (MemPalace live).**

## Web build

```
$ npx tsc -p tsconfig.json --noEmit
exit 0

$ npx vite build
dist/index.html                   0.52 kB
dist/assets/index-*.css          25.18 kB
dist/assets/index-*.js          345.89 kB
built in ~1 s
```

## Release compile

```
$ cargo check --release
Finished `release` profile [optimized] target(s) in 1m 59s
```

The release build is clean (warnings only, no errors). `cargo tauri
build` is documented in `docs/BUILDING.md` but was not run end-to-end
in this environment because Tauri requires the WebView2 runtime at
build time; the bundle configuration has not changed.

## What was built since the previous release point

* **E2** Context inspector panel in the Assistant page, with grouped
  sources, per-item token counts, evidence panel for graph edges,
  open-source links to vault documents, graph entities, and memories.
* **F** Model router (capability registry, lifecycle state machine,
  decision log, resource-aware scheduling, hard prefer override).
* **G** Research Studio (projects, papers, claims, comparison
  matrices with text/number/bool/rating columns, Markdown reports).
* **H** Experiment tracking (experiments, runs with hyperparams JSON
  and metrics JSON, metric history chart).
* **I** Controlled agent (12 seeded tools across 6 categories, four
  permission levels, allowlisted paths and commands, full audit log).
* **J** Document intelligence (pipe and whitespace table extraction,
  CSV/TSV export, line diff).
* **K** Performance & UX (virtualised list component, audit panel
  on the Monitor page, no fabricated GPU metrics).
* **L** Packaging & release docs.

## New modules

* `nexus_core::research::{projects,papers,claims,matrices,reports,markdown}`
* `nexus_core::experiments::{experiments,runs}`
* `nexus_core::agent::{registry,runs,invocations,policy,executor}`
* `nexus_core::intelligence::{tables,csv,diff}`
* `nexus_core::router::{capabilities,lifecycle,policy,state}` (was
  already present; this release fixes the `prefer_profile_id`
  override and adds the inline lifecycle test).

## Schema

* Migration `0008_router.sql` — `router_capabilities`,
  `router_lifecycle`, `router_decisions`.
* Migration `0009_research.sql` — `research_projects`,
  `research_papers`, `paper_claims`, `comparison_matrices`,
  `matrix_columns`, `matrix_cells`, `research_reports`.
* Migration `0010_experiments.sql` — `experiments`,
  `experiment_runs`.
* Migration `0011_agent.sql` — `agent_tools`, `agent_runs`,
  `agent_invocations`.

## Known follow-ups (deliberately deferred)

* MLflow read-only sync is not implemented; the table is the only
  thing the schema promises.
* OCR (tesseract) is not bundled. The intent is a sidecar; it is
  not needed for the offline-first story.
* Retention / pruning of `audit_events` is not in this build.
* No on-device speaker embeddings, no live MLflow import, no
  llama-server / vLLM provider beyond what the provider-trait can
  accommodate.

## Cross-references

* `AUDIT.md` is the canonical security / privacy summary.
* `IMPLEMENTATION_PLAN.md` lists every checkbox with the phase that
  delivered it.
* `docs/PHASE*_SUMMARY.md` is the running per-phase changelog.