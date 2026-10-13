# NEXUS — Security Model

## 1. Threat model

The user is a single developer/researcher. The realistic threats are:

1. **Prompt injection** from documents, web content, snippets, or memory text that gets pulled into a prompt and tricks the LLM into calling tools or exfiltrating data.
2. **Filesystem abuse** by the LLM via a tool that is given too much permission.
3. **Path traversal** in any user-supplied identifier (file name, project id, source path).
4. **Process abuse** by the LLM via an unrestricted command-execution tool.
5. **SQL injection** from the LLM into ad-hoc queries.
6. **Outbound network leakage** when "online" features are enabled.
7. **Loss of data** through careless writes or schema migrations.

NEXUS assumes the LLM is **untrusted** even though it runs locally. All authorizations live in the Rust core.

## 2. Authorization layers

1. **Process boundary**: the WebView runs untrusted UI code; all sensitive work is in the Rust process. Tauri commands are typed and validated.
2. **Permission scope** (`permissions` table) is the source of truth. Default = `readonly`.
3. **Tool policy**: each tool has a `required_permission`. Before execution, the engine asserts the active scope covers that level.
4. **Argument validation**: each tool defines a JSON schema; arguments are parsed and validated before any side effect.
5. **Path validation**: every path is canonicalized and checked against an allowlisted root.
6. **Command allowlist**: the `run_command` tool resolves a registered command id; raw shell strings are not accepted.
7. **Approval gate**: any tool requiring `confirm` pauses and waits for a user decision in the UI.

## 3. Prompt injection defenses

* All retrieved text is concatenated into a *user-side* section of the prompt, clearly delimited and labeled.
* System prompts are immutable per session and are not influenced by tool results.
* The agent loop is bounded: max steps, max tool calls per step, hard timeout, hard token budget.
* Tool outputs are *not* re-injected as instructions; only their structured result fields enter the next prompt.
* A configurable "high-trust" mode (off by default) lets the user opt into letting the assistant compose `cmd` strings; even then the strings are validated against a regex allowlist.

## 4. Filesystem boundaries

* `Workspace` = a directory chosen by the user. Each workspace is registered in `sources` and stored as an absolute canonical path.
* Tools that read or write files take a `root_id` and a `relative_path`; the engine canonicalizes the result and refuses anything outside the root.
* Hidden files, `node_modules`, `.git`, `target`, and a configurable blocklist are skipped during ingestion and protected from writes.

## 5. Database

* All access through `rusqlite` with **parameterized** statements (no string interpolation).
* `PRAGMA foreign_keys = ON` at every connection.
* Migrations are forward-only and never destructive; if a migration fails the database is left in the previous version and the user is told.
* Backups: `app.data_dir/backups/YYYY-MM-DDTHHMMSS.sqlite` created automatically before destructive operations and on a configurable schedule.

## 6. Network

* The default backend base URL is `http://127.0.0.1:<port>`. The provider refuses to start if the URL is not loopback unless the user explicitly enables a non-loopback endpoint in **Settings → Advanced → Network**.
* CSP forbids remote scripts. Tauri config disables `dangerousRemoteUrlIpcAccess`.
* Optional integrations (MLflow, Hugging Face, OCR sidecar, research script sidecar) are gated behind a single "Allow optional online features" toggle, off by default. Each one logs its outbound request to `audit_events`.
* Environment variables passed to subprocesses are filtered; `*TOKEN*`, `*SECRET*`, `*KEY*` and known credential paths are redacted from logs and from command output.

## 7. Secrets

* No cloud API keys are required for core functionality. Where a user *does* add one (e.g. a private MLflow token), it is stored with `keyring` (Windows Credential Manager) via the `keyring` crate. The DB only stores a `secret_ref` key, not the value.

## 8. Agent execution

* Every run writes an `agent_runs` row and one `tool_executions` row per attempt.
* Cancellation is propagated to subprocesses (`tokio::process::Child::kill`) and to in-flight async tasks via `tokio_util::sync::CancellationToken`.
* The reflection step (a single confirmation prompt) is the only place where the model is asked to summarize the result; it cannot issue a tool call from inside reflection.
* Recursive agent loops are blocked: a sub-agent cannot spawn another sub-agent.
* Destructive operations (file deletion, dropping a project, overwriting a file) require `confirm` and surface the target path in the approval dialog.

## 9. Auditing

* `audit_events` is append-only.
* Required events: model loaded, model unloaded, tool executed, permission changed, file created, file modified, file deleted, project archived, memory approved, memory deleted, settings changed, network request issued.
* The user can export the entire audit log to JSON or NDJSON.

## 10. Failure containment

* A panic in any ingestion job leaves the document row in `failed`; the queue retries with backoff up to N attempts, then surfaces the failure in the UI.
* A panic in the Tauri command thread is logged and reported; the UI displays a non-fatal error banner.
* The DB has `WAL` mode enabled; a crash mid-write does not corrupt the database.
