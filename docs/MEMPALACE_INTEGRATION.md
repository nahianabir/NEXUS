# MemPalace integration

MemPalace is an **optional** backend for the NEXUS long-term memory
system. The application ships with a fully functional native memory
store (`memories_v2`); MemPalace can be enabled alongside it without
disrupting the existing user data.

## Status of this integration

* NEXUS exposes a typed adapter abstraction with three concrete
  backends: `native`, `mempalace`, `disabled`. The selected backend is
  stored in `MemoryBackendState` (see `app_state.rs`).
* The MemPalace backend talks to a Python sidecar over a small
  JSON-RPC surface (`health`, `search`, `store`, `delete`,
  `list_collections`). The sidecar contract is documented below.
* The official MemPalace repository at
  <https://github.com/MemPalace/mempalace> was **not reachable from
  this build environment** during the v0.1.0 release. The adapter
  was therefore implemented against the documented contract only.
  The live integration test is `#[ignore]`-gated; enable it with
  `cargo test --test phase_c -- --ignored` once MemPalace is
  installed and `MEMPALACE_BIN` is set.
* A native backend is always available, so the application boots and
  serves chat, projects, and research without MemPalace present.

## What the sidecar receives

Each call spawns a single process. NEXUS writes a single line of JSON
to the sidecar's stdin and reads the response from stdout.

### `health`

Request:
```json
{ "method": "health" }
```

Response (`ok: true`):
```json
{
  "ok": true,
  "version": "0.4.2",
  "message": "MemPalace ready"
}
```

Response (`ok: false`):
```json
{ "ok": false, "error": "index not loaded" }
```

### `search`

Request:
```json
{
  "method": "search",
  "query": "What is the default embedding model?",
  "project_id": "01H...",
  "limit": 10
}
```

Response (`ok: true`):
```json
{
  "ok": true,
  "value": [
    {
      "id": "mp-...",
      "backend": "mempalace",
      "project_id": "01H...",
      "workspace_id": null,
      "category": "decision",
      "content": "Default embedding model is nomic-embed-text-v1.5",
      "source_kind": "user",
      "source_id": null,
      "source_excerpt": null,
      "event_at": null,
      "valid_from": null,
      "valid_until": null,
      "created_at": 1731600000000,
      "updated_at": 1731600000000
    }
  ]
}
```

### `store`

Request:
```json
{
  "method": "store",
  "record": { "id": "", "backend": "native", "category": "project_fact", "content": "..." }
}
```

Response:
```json
{ "ok": true, "value": { "id": "mp-..." } }
```

### `delete`

```json
{ "method": "delete", "id": "mp-..." }
```

### `list_collections`

```json
{ "method": "list_collections" }
```

## Timeouts, retries, and backoff

* Per-call timeout defaults to 10 s (`MemPalaceConfig::timeout_ms`).
* One process per call. A failed call does not crash the host
  process; the adapter returns `AdapterError::Upstream`.
* The `BackendSet::search_fallback` helper in
  `mempalace::adapter` falls back to the native store on any
  failure, so the assistant never blocks on MemPalace being down.

## Privacy

* NEXUS only ever sends the fields listed in `AdapterMemory`. Source
  paths and excerpts are explicit opt-in (the user fills them in).
* The NEXUS `audit_events` table records every call (`backend.activate`,
  `backend.store`, `backend.delete`).
* No silent retries or background uploads. Every outbound call is
  user-initiated.

## Enabling the integration

1. Install MemPalace and verify it works on the command line.
2. Open NEXUS → **Settings** → **Memory backend** → **MemPalace**.
3. Set the binary path (e.g. `mempalace` on PATH or an absolute path
   to the wrapper).
4. Click **Activate**. The status panel reports `healthy` when the
   sidecar responds within the timeout.

## Verifying locally

* Run the live test:

  ```powershell
  $env:MEMPALACE_BIN = "C:\path\to\mempalace.exe"
  cargo test --test phase_c live_mempalace_health -- --ignored --nocapture
  ```

* Run the unit tests (no MemPalace required):

  ```powershell
  cargo test --test phase_c
  ```

## Known limitations

* NEXUS does not yet expose MemPalace-specific operations (timelines,
  related-entity graph from MemPalace). The adapter returns
  `AdapterError::Unsupported` for these and the assistant falls back to
  the native knowledge graph (Phase D) when it needs them.
* The adapter writes one process per call. A long-lived daemon mode
  will land in a later phase if the user base asks for it.
