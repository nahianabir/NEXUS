# Phase 4 — Knowledge Vault

## What ships

Phase 4 turns the previously-shadowed `ingestion` module into a real product
surface. The Knowledge Vault is a content-addressed, FTS5 + sqlite-vec
hybrid index that the Assistant can cite from.

### Rust core (`src-tauri/src/`)

* `ingestion/extractors.rs` — one extractor per file type. Already shipped in
  Phase 1 scaffolding; the implementation covers TXT / MD / RST / LOG,
  JSON, CSV / TSV, HTML, code (line-numbered), IPYNB, PDF (`pdf-extract`),
  DOCX (XML walk), XLSX (`calamine`).
* `ingestion/chunker.rs` — paragraph-aware chunker with a 500 / 80
  target/overlap default. Pure, tested.
* `ingestion/hash.rs` — SHA-256 streaming hash for content dedup.
* `ingestion/pipeline.rs` — `register_source`, `insert_document`,
  `insert_chunk`, `get_chunk`, `list_sources`, `list_documents`,
  `delete_source`, `delete_document`, plus the synchronous `ingest_source`
  used by the unit tests.
* `ingestion/orchestrator.rs` *(new)* — async background job that:
  1. registers the source,
  2. walks the path (skipping `.git`, `node_modules`, `target`, …),
  3. SHA-256 hashes every file,
  4. skips anything already in the vault,
  5. extracts via the right extractor,
  6. chunks the text,
  7. persists documents + chunks,
  8. embeds each chunk via the active provider's `/v1/embeddings` (when the
     user opted in),
  9. emits `nexus:vault.progress` events for the UI,
  10. returns an `IngestJobResult` summary.
* `ingestion/vector.rs` — sqlite-vec upsert / search over a fixed 384-dim
  index (compatible with `text-embedding-nomic-embed-text-v1.5`).
* `ingestion/search.rs` — FTS5 with token escaping + Reciprocal Rank Fusion
  across FTS5 and the vector index.
* `ingestion/citation.rs` *(new)* — `Citation` value type plus
  `persist_citations` / `list_for_message` for the `message_sources` table.
* `commands/vault.rs` *(new)* — 10 Tauri commands:
  `vault_list_sources`, `vault_register_source`, `vault_delete_source`,
  `vault_list_documents`, `vault_get_document`, `vault_delete_document`,
  `vault_ingest_source` (spawns the orchestrator; returns the job id
  immediately; progress flows on `nexus:vault.progress`/`done`/`error`),
  `vault_search` (hybrid retrieval), `vault_list_citations`,
  `vault_embed_backfill`, `vault_stats`. Plus three helpers (`build_citations_from_hits`,
  `build_context_block`, `save_citations`) that the chat command uses to
  attach a context block to the model prompt and persist the citations on
  the assistant message.
* `commands/chat.rs` — extended: `SendRequest` now has `use_vault` and
  `vault_limit`. When the user opts in, `chat_send` runs a hybrid search
  over the latest user message, builds a context block, prepends it to the
  system prompt, and saves the citation rows on the assistant message
  after the stream finishes. A new `nexus:chat.citations` event surfaces
  the live citation list to the UI.

### Web (`app/web/src/`)

* `lib/vault.ts` *(new)* — typed wrappers for the 10 vault commands and
  four event listeners.
* `lib/chat.ts` — adds `ChatCitation`, `ChatCitationsEvent`,
  `ChatSendOptions`; the `chatSend` helper takes `{ useVault, vaultLimit }`.
* `pages/Knowledge.tsx` *(new)* — three panels (Sources, Search,
  Documents) plus a stats bar, a live progress bar, and a backfill
  button. Sources are registered through the Tauri dialog plugin
  (file or directory). Live progress arrives on the `nexus:vault.*`
  events; the page re-fetches sources / docs / stats on every "done".
* `pages/Assistant.tsx` — added a "use vault" toggle next to the
  composer and a per-message `CitationsPanel` that lists each citation
  (chunk id, document title, score, preview). Citations arrive on
  `nexus:chat.citations`, and the page also re-loads persisted citations
  from the DB after a chat completes (so a reload still shows them).

### Tests

* `tests/phase4.rs` — five unit tests that were in place from the
  Phase 1 scaffolding (chunker splits long text, hash is stable,
  extractor handles every supported type, FTS5 finds a known phrase,
  ingest skips a duplicate by hash). Fixed two long-standing bugs in
  the existing assertions (the chunker test used a single long paragraph
  with no newlines; the extractor test compared MIME `text/markdown` to
  the actual `text/plain` returned by the markdown extractor).
* `tests/phase4_live.rs` *(new)* — gated on `NEXUS_LIVE_TEST=1`. Creates
  a temp dir with three `.md` fixtures, runs the orchestrator, asserts
  FTS5 retrieval, asserts hybrid retrieval, asserts zero hits for a
  nonsense query. Passes: 3 files → 3 chunks → 1 FTS hit, 1 hybrid hit,
  0 hits for "zzznotawordzzz".

## Migration 0004

`migrations/0004_vault.sql` adds `workspaces`, `projects`, `collections`,
`sources`, `documents`, `document_chunks`, the `chunks_fts` virtual table
with `porter unicode61` tokenisation, the `chunks_ai` / `ad` / `au`
triggers that keep the FTS5 mirror in sync, the `chunk_vectors` vec0
table at 384 dims, `tags`, `document_tags`, `notes`, and `tasks`. Future
phases (5, 6, 7) will surface the workspace / project / task tables;
the vault needs only `sources`, `documents`, `document_chunks`, and the
two virtual tables.

## Linker workaround

`db/mod.rs` previously redeclared `sqlite_vec_init` as a private
`unsafe extern "C"`. The rlib's `#[link(name = "sqlite_vec0")]` directive
attached to the `sqlite-vec` crate's re-export of that symbol was being
shadowed, and the bin lost `-lsqlite_vec0` from its link line. Fixed by
calling `sqlite_vec::sqlite3_vec_init()` directly, which preserves the
`#[link]` attribute and makes the bin's link line deterministic. The
build.rs is now back to its original one-liner.

## Verification

* `cargo test --tests --lib` (with `NEXUS_LIVE_TEST=1`): **25 passed, 0 failed**
  - 4 lib tests
  - 2 `phase1` tests
  - 8 `phase2` tests (frame parser, providers, config)
  - 3 `phase3` tests (conversations, message updates, profile resolution)
  - 5 `phase4` tests (chunker, hash, extractor, FTS, dedup)
  - 2 live integration tests (`live_chat`, `live_lmstudio`)
  - 1 new live test (`live_vault_ingest_and_search`)
* `npm run build` (tsc + vite): **success**, 230 KB JS, 22.5 KB CSS.
* `cargo build` (debug): **success**, binary at `target/debug/nexus.exe`.
* Phase 1 smoke test (`smoke1.ps1`): **PASS** — binary boots, working
  set 49.7 MB, WAL present, clean shutdown.
* Live vault test (`live_vault.ps1`): 3 files → 3 chunks, 1 FTS hit on
  "tau neutrino", 1 hybrid hit on "potassium bananas", 0 hits on
  nonsense query.

## Hand-off

The Vault is ready to feed the rest of the application. Specifically:

* `chat_send` already retrieves top-K context and persists citations
  when the user flips the "use vault" toggle in the Assistant.
* `vault_stats` is wired into the dashboard's source/doc counts.
* Migration 0004 created `workspaces`, `projects`, `tasks`, `notes`,
  `papers`, `paper_claims`, etc.; the next phases (5–9) just add the
  Rust functions that write to them.

Suggested next phases:

* **Phase 5 — Projects, notes, tasks, memory**: surface the four tables
  Phase 4 already created; add `commands::projects`, `commands::notes`,
  `commands::tasks`, and a calendar view.
* **Phase 6 — Research Studio**: PDF import → `papers` row → claim
  extraction via the LLM → `paper_claims` rows.
* **Phase 7 — Experiments**: experiment + run records that the Vault's
  retrieval metrics can be compared against (e.g., vector-only vs
  hybrid vs FTS-only).
