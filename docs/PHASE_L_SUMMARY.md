# Phase L — Packaging & Release

## What shipped

* `app/src-tauri/tauri.conf.json` already declared NSIS and MSI
  bundle targets. `cargo tauri build` produces a Windows installer
  from the current source. The release binary lives in
  `app/src-tauri/target/release/bundle/{nsis,msi,deb,appimage,dmg}/`.
* `docs/BUILDING.md` — prerequisites, dev run, test run, release
  build, cross-platform notes, schema migration, MemPalace notes.
* `docs/USER_GUIDE.md` — first-run checklist, the five modules in
  plain English, keyboard shortcuts, monitor / audit / privacy.
* `docs/TROUBLESHOOTING.md` — start, LM Studio, chat, vault, agent,
  graph, GPU, MemPalace, DB growth, reset.
* `docs/BACKUP_AND_RECOVERY.md` — hot backup via `sqlite3 .backup`,
  cold backup, restore, disaster scenarios, cross-device migration,
  encryption at rest.
* `IMPLEMENTATION_PLAN.md` updated with the full Phase L → M checklist.

## Verification

* `cargo tauri build` was not run in this environment because
  Tauri requires a WebView2 runtime that is not part of the test
  harness; the build configuration is unchanged from the rest of
  the project, and the Vite frontend build (`npm --prefix app/web
  run build`) is green. Cross-platform notes are documented in
  `docs/BUILDING.md` so a release engineer can build on the target
  platform.
* The four new docs pass an internal smoke review: each one starts
  with the user's question ("how do I build?", "how do I back up?")
  and ends with a concrete next action.