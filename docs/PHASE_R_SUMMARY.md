# Phase R — Final Release

NEXUS 1.0.0 is built. This phase finalises the release pipeline
and the polish needed to ship a real installer.

## What ships

### Versioning

* `app/src-tauri/Cargo.toml` — `version = "1.0.0"`.
* `app/src-tauri/tauri.conf.json` — `version: 1.0.0`.
* `app/src-tauri/src/main.rs` — banner reflects the version.

### Release artifacts

`cargo tauri build` (run from `app/src-tauri/`) produces three
artifacts:

| Artifact                                                                | Size     |
|-------------------------------------------------------------------------|----------|
| `app/src-tauri/target/release/bundle/nsis/NEXUS_1.0.0_x64-setup.exe`   | 5.31 MiB |
| `app/src-tauri/target/release/bundle/msi/NEXUS_1.0.0_x64_en-US.msi`     | 7.39 MiB |
| `app/src-tauri/target/release/nexus.exe`                                | 16.47 MiB|

Both installers are per-machine; the NSIS dialog lets the user pick
"Just me" to install without admin rights.

### Top-level docs

* `CHANGELOG.md` — Keep-a-Changelog format, lists every feature
  that landed in 1.0.0.
* `RELEASE_NOTES.md` — end-user release notes, first-run checklist,
  install + backup pointers, and a "what is not in this release"
  honesty list.
* `AUDIT.md` — refreshed totals (104 / 104 + 1 ignored), new
  release-installer block, new phase rows.

### About panel

* New "About" section at the bottom of the Settings page, reading
  the live `app_info` Tauri command. Shows:
  * Product name + version (chip).
  * Data directory (monospaced, full path, word-wraps).
  * Schema version.
  * Build target (windows / darwin / linux).
  * Link to the user and building docs.

### Scripts

* `scripts/dev.ps1` — installs web deps if missing, then launches
  `cargo tauri dev`.
* `scripts/check.ps1` — runs the full CI check: Vite build + cargo
  test + cargo check --release. Respects `NEXUS_SKIP_WEB=1` and
  `NEXUS_SKIP_RELEASE=1` for faster local iterations.
* `scripts/release.ps1` — runs `cargo tauri build`. Respects
  `NEXUS_SKIP_WEB=1` (skip Vite) and `NEXUS_NO_BUNDLE=1` (only build
  the .exe, no installers).

### Phase N integration tests

`app/src-tauri/tests/phase_n.rs` exercises the boot path used by the
real Tauri binary:

* `app_state_initialize_migrates_and_persists_config` — `AppState::initialize`
  migrates the schema, writes the default `config.toml`, and the
  resulting `schema_migrations` version is `>= 11`.
* `audit_log_round_trip` — `audit::log` + `audit::recent` work
  through the real pool.
* `boot_time_audit_prune_removes_old_events` — sets
  `audit_retention_days = 30` and `audit_prune_on_boot = true`,
  inserts an "ancient" event directly, re-initialises the state,
  and asserts the ancient row is pruned but the `app.boot` event
  remains.
* `file_and_shell_allowlists_round_trip` — the new agent allowlists
  survive save / load / initialise.

These four tests run against the same `AppState::initialize` that
the binary calls in `main.rs::setup`, so the verification mirrors
the production boot path.

## Tests that actually ran

```
$ cargo test --lib --tests
   16 (lib) + 3 (main binary) + 85 (integration) = 104 passed
   1 ignored (MemPalace live)
   0 failed
```

Across 21 test binaries.

```
$ npx vite build
   dist/index.html                   0.52 kB
   dist/assets/index-*.css          25.21 kB
   dist/assets/index-*.js          346.32 kB
   ✓ built in 1.01s
```

```
$ cargo tauri build
   Compiling nexus v1.0.0
    Finished `release` profile [optimized] target(s) in 3m 23s
    Finished 2 bundles at:
       NEXUS_1.0.0_x64-setup.exe       (5.31 MiB)
       NEXUS_1.0.0_x64_en-US.msi        (7.39 MiB)
```

## What this release does not include

* **Code signing** — Tauri supports signing via
  `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The distributors set those
  up; nothing about the build prevents it.
* **Auto-update** — Tauri 2 supports an `updater` plugin. The user
  can wire it into the existing release pipeline when needed; the
  current release ships without it because the spec did not require
  it and the data is local-first anyway.
* **MemPalace live integration** — still gated behind
  `#[ignore]`. The native backend covers the common path; see
  `docs/MEMPALACE_INTEGRATION.md`.

## Known minor warnings (carry-over, non-blocking)

The 5 `unused_*` warnings in `kg/{evidence,graph,queries}.rs` and
the `unused_variable: s` in `phase_f::tests` were intentionally
left in place — they are pre-existing, not caused by this phase,
and the `cargo fix` suggestion is to add `_`-prefixes that would
just be cosmetic.
