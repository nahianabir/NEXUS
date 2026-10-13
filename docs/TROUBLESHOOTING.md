# NEXUS — Troubleshooting

## The app fails to start

1. **Data directory is locked.** NEXUS uses SQLite WAL. If a previous
   process crashed, the WAL file (`nexus.db-wal`, `nexus.db-shm`)
   may be stale. Quit the app, then in the data dir
   (`%APPDATA%/ai.nexus.workbench` on Windows,
   `~/Library/Application Support/ai.nexus.workbench` on macOS,
   `~/.local/share/ai.nexus.workbench` on Linux) delete the `-wal`
   and `-shm` files. The next launch will recover from the main DB.
2. **Migration error.** Open `%APPDATA%/ai.nexus.workbench/nexus.log`
   (or the platform equivalent). The migration runner logs the
   failing version. If the schema is genuinely incompatible with
   older code, copy `nexus.db` aside and let the app create a fresh
   one, then re-import what you need via the Vault.

## LM Studio is running but no model is detected

* LM Studio's default API is at `http://127.0.0.1:1234`. NEXUS
  expects that address; if you changed the port, edit the provider
  on the Models page.
* The "model probe" button does an `HTTP GET /v1/models`. If the
  endpoint returns 200 with at least one model, NEXUS will list it.
* The local server must be in "developer mode" (the toggle in the
  LM Studio developer tab). Otherwise `/v1/models` returns 404.

## Chat is slow or returns empty

* The first chat of a session loads a model into memory. On a 13B
  model on CPU this can take 30+ seconds.
* Check the Monitor page: if CPU is pegged at 100%, the model is
  decoding tokens; that is normal. If CPU is idle, the request
  probably never reached the backend.
* Increase `max_tokens` in the model profile if the reply is being
  truncated.

## Vault search returns nothing

* FTS5 requires the chunks to be indexed. Re-ingest the source.
  The Vault page shows the chunk count per document.
* FTS5 tokenises on whitespace and ASCII letters. Queries with
  punctuation (e.g. "C++") will not match. Try a single-word query
  first to confirm the index is healthy.
* Vector search requires an embedding profile and a backfill. If
  you have not run the backfill, only FTS5 is used.

## Agent run is "stuck" on pending

* "Ask" tools require explicit approval. Open the Agent page,
  click the run, and click "approve" on each pending invocation.
* "Restricted" tools require the underlying capability to be
  enabled in Settings (currently a no-op flag; restricted tools
  remain require-approval until that flag is set).

## Knowledge graph entities are not appearing

* The graph is *populated from approved memories* and from
  project seed actions. Auto-extraction of entities from arbitrary
  text is not enabled in this build; use the KG "create" form or
  seed a project to add nodes.
* Aliases are case-folded and punctuation-stripped. Two nodes that
  differ only in case (`Apple` vs `apple`) collapse to one.

## The Monitor page is missing GPU stats

* GPU telemetry depends on the OS exposing it. Windows exposes
  Intel Arc stats via the Performance Counters; if you do not see
  them, the WebView does not have permission or the driver does
  not report them. The Monitor page reports "unknown" instead of
  guessing — this is intentional.

## MemPalace sidecar will not start

* The Python sidecar is **not bundled** with NEXUS. See
  `docs/MEMPALACE_INTEGRATION.md` for the wire format and the
  expected process name.
* The "live" integration test is `#[ignore]`-gated precisely
  because of this.

## Database grows very large

* The `audit_events` table grows monotonically. There is no auto-prune
  in the current build. A future release will add a retention knob.
* The `messages` table accumulates with every chat. Old conversations
  can be archived (the archive flag hides them from the sidebar) and
  the underlying rows are kept for history.
* Run `VACUUM` from the sqlite3 CLI on `nexus.db` if you want to
  reclaim space after large deletes.

## Reset to factory defaults

1. Quit NEXUS.
2. Move the data dir aside (or rename it).
3. Restart NEXUS. The app recreates a fresh DB and seeds the model
   profile defaults.

## Where the data lives

| Platform | Path                                              |
|----------|---------------------------------------------------|
| Windows  | `%APPDATA%\ai.nexus.workbench\`                  |
| macOS    | `~/Library/Application Support/ai.nexus.workbench/`|
| Linux    | `~/.local/share/ai.nexus.workbench/`             |

The Command Center page also displays the resolved path.
