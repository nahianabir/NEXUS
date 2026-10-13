# NEXUS — Backup and Recovery

Everything NEXUS stores lives in a single SQLite database
(`nexus.db`) plus a small set of WAL sidecar files (`-wal`, `-shm`).
There is no server, no remote sync, and no implicit cloud copy.
This page describes how to back up and restore that file safely.

## What to back up

* `<data_dir>/nexus.db`
* `<data_dir>/nexus.db-wal`
* `<data_dir>/nexus.db-shm`
* `<data_dir>/app_settings.json` (Tauri's per-app config; usually
  not user-edited, but capture it for symmetry)
* `<data_dir>/logs/` (debugging, not strictly required for recovery)

`<data_dir>` is the path shown on the Command Center page. The
defaults are:

| Platform | Path                                              |
|----------|---------------------------------------------------|
| Windows  | `%APPDATA%\ai.nexus.workbench\`                  |
| macOS    | `~/Library/Application Support/ai.nexus.workbench/`|
| Linux    | `~/.local/share/ai.nexus.workbench/`             |

## Hot backup (safe while the app is running)

SQLite in WAL mode supports backup via the `sqlite3 .backup`
command or by copying the file directly while the app is running
(the WAL guarantees a consistent snapshot). The simplest approach:

```sh
sqlite3 nexus.db ".backup '/path/to/backup/nexus-YYYY-MM-DD.db'"
```

This produces a transactionally-consistent copy. Repeat on a
schedule (cron, Task Scheduler, launchd).

## Cold backup (app is not running)

If you prefer the simplicity of a file copy:

1. Quit NEXUS completely.
2. Copy `nexus.db` (and, optionally, the `-wal` and `-shm`
   sidecars) to a safe location.
3. The copy is a complete snapshot; no further work is needed.

The `-wal` file is only relevant when the app is running. Once the
app exits cleanly, SQLite checkpoints the WAL back into the main
file and you can back up just `nexus.db`.

## Restore

1. Quit NEXUS.
2. Replace `nexus.db` (and the sidecars, if you backed them up) in
   the data dir.
3. Restart NEXUS. The migration runner will see the schema as
   already applied and skip ahead.

If the restored database is from an older version of NEXUS, the
migrations will be applied in order on the next launch. If it is
from a newer version, NEXUS will refuse to start; in that case
revert to the matching NEXUS binary or wait for an upgrade.

## Disaster scenarios

* **WAL corruption (crash mid-write).** Delete `nexus.db-wal`
  and `nexus.db-shm`; the main DB is still consistent.
* **Main DB corruption.** Restore from backup. If you have no
  backup, try `sqlite3 nexus.db ".recover" > recovered.sql` and
  inspect what came back.
* **Schema drift after a botched migration.** The migration runner
  records applied versions in `schema_migrations`. To force a
  re-run, delete the row for the broken version **only** if you
  know what you are doing; otherwise restore the pre-migration
  backup.
* **Lost file path.** Search for `nexus.db` under the data dir; the
  Command Center page prints the exact resolved location.

## What is NOT in the database

* The bundled model registry (the four default profiles) is
  written by `ensure_default_profiles` on every boot. If you
  delete the DB you lose any custom profiles, but the defaults
  reappear.
* The memory v2 dedupe index is rebuilt from `content_hash`
  values; no special recovery is needed.
* The knowledge graph is rebuilt from memories; missing rows are
  recreated the next time the source memories are loaded.

## Cross-device migration

The data dir is portable. To move NEXUS to a new machine:

1. Quit NEXUS.
2. Copy the data dir to the new machine's equivalent location
   (paths differ per platform; see the table above).
3. Install NEXUS on the new machine (run the installer or copy
   the binary; the model server software must match).
4. Launch NEXUS; it will detect the existing DB and continue.

## Encryption at rest

NEXUS does not encrypt the SQLite file. If your threat model
requires encryption at rest, put the entire data dir inside an
encrypted volume (BitLocker, FileVault, LUKS) and back up the
encrypted container as a single artefact. Do not attempt to
encrypt individual rows; SQLite's WAL would not be encrypted
and you would lose crash-safety.
