# Phase N — Operations & Policy Surface

This phase finishes the operations surface that the previous phases left implicit.
Nothing here introduces a new model feature; it tightens the loop between the
Settings page, the agent executor, and the audit log so the policy the user
sees is the policy the Rust core actually enforces.

## What ships

### Agent allowlists

* `AppConfig::AgentConfig` now carries two new fields:
  * `file_allowlist: Vec<String>` — directories the agent's `file.read` and
    `file.list` tools may touch. Compared by canonical path prefix.
  * `shell_allowlist: Vec<String>` — command basenames the agent's
    `shell.run` tool may invoke. Exact basename match.
* They round-trip through `AppConfig::save` / `load` (TOML).
* The Settings page renders them as multi-line text editors; the Web side
  (`AppSettings.agent`) types them as `string[]` with `[]` defaults.
* The Agent page reads the persisted config via `settings.get()` and passes
  the allowlists to `agent_run_tool` so the executor's
  `policy::check_path_allowed` / `check_command_allowed` actually have a
  list to compare against. Previously the UI always passed `[]`, so the
  executor denied every file / shell tool even when the user expected it
  to be allowed.

### Audit log retention

* `audit::clear(pool)` — deletes every event, returns count removed.
* `audit::prune_before(pool, before_ms)` — deletes events older than the
  cutoff.
* `audit::prune_by_retention(pool, retention_days)` — convenience wrapper
  that computes the cutoff from `retention_days`. No-op when 0.
* Three new Tauri commands: `audit_clear`, `audit_prune(before_ms)`,
  `audit_prune_by_retention(retention_days)`. All return the row count.
* `AppConfig::AgentConfig` adds `audit_retention_days: u32` and
  `audit_prune_on_boot: bool`. The Settings page renders them as a
  number input + toggle. `AppState` calls `prune_by_retention` at boot
  when the toggle is on, and logs how many rows were removed.
* The System Monitor page now has a "Clear log" button with a confirm
  dialog that calls `audit_clear` and reloads the view.

## Files touched

* `app/src-tauri/src/config.rs` — `AgentConfig` adds the four new fields
  with `#[serde(default)]` and the matching `Default` arm.
* `app/src-tauri/src/audit.rs` — three new functions.
* `app/src-tauri/src/commands/audit_cmd.rs` — three new Tauri commands
  + 3 unit tests against a real migrated SQLite pool.
* `app/src-tauri/src/app_state.rs` — boot-time pruning.
* `app/src-tauri/src/main.rs` — register the new Tauri commands.
* `app/web/src/lib/settings.ts` — type + default for the new fields.
* `app/web/src/pages/Settings.tsx` — two new sections (file/shell
  allowlist, audit retention) and a small `<AllowlistEditor>` helper.
* `app/web/src/pages/Agent.tsx` — pull the allowlists from settings when
  invoking `agentRunTool`.
* `app/web/src/pages/Monitor.tsx` — "Clear log" button.
* `IMPLEMENTATION_PLAN.md` — Phase N entry.

## Tests that actually ran

`cargo test --lib --tests`:

```
test result: ok. 16 passed (lib)
test result: ok.  3 passed (main binary, includes new audit_cmd tests)
test result: ok. 81 passed (integration)
test result: ok.  6 passed, 1 ignored (phase_c: live_mempalace_health)
```

**100 passed, 1 ignored (MemPalace live), 0 failed.**

The three new tests in `commands/audit_cmd::tests` are:

* `clear_removes_all_events` — inserts three, clears, asserts empty.
* `prune_before_removes_only_old` — inserts one ancient + one recent row
  directly, prunes everything older than `now - 1000`, asserts only the
  recent row remains.
* `prune_by_retention_zero_is_noop` — `0` is a sentinel that means
  "keep forever" and must never delete anything.

`npm --prefix app/web run build` — green, 346 kB / 96 kB gzipped JS.
`cargo check --release` — green, only the same pre-existing warnings
(5 `unused` warnings in `kg/*` and one `unused_variable` in
`phase_f::tests`).

## Why this matters

Until this phase, the policy the Settings page advertised did not reach
the Rust core:

* The agent's `file.*` and `shell.run` tools could not run at all,
  because `agent_run_tool` was being called with empty allowlists.
* The audit log was permanent. A long-lived install would accumulate
  rows without a documented way to bound them.

Both of those are now correct, observable, and tested.
