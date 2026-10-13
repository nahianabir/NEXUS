# Phase 5 — Projects, notes, tasks, memory, and project context

**Status:** complete. All 34 unit tests pass, the live vault test still
passes, the Tauri binary builds, the Vite build is clean, and the smoke
test (boot the binary, confirm SQLite + WAL) succeeds.

## What ships

### Data model
- Migration `0005_memory.sql` adds two tables:
  - `memories(id, scope, project_id, conversation_id, content, source,
    approved, created_at, updated_at)` with a scope/approved index, a
    project index, and a conversation index.
  - `ui_state(key, value, updated_at)` for tiny JSON-flag storage
    (the "active project" pointer lives here).
- The `workspaces`, `projects`, `notes`, and `tasks` tables from
  migration 0004 are now wired through a typed Rust DAL and surfaced
  via Tauri commands.
- `conversations.project_id` is set when a chat is created from an
  active project; the column was already in 0003.

### Rust crates
- `nexus_core::projects::workspaces` — workspace CRUD.
- `nexus_core::projects::projects` — project CRUD; `instructions` and
  `goals` (JSON array of strings). `set_archived` toggles the flag.
- `nexus_core::projects::notes` — note CRUD; free-form `title` + `body`.
- `nexus_core::projects::tasks` — task CRUD with `TaskStatus`
  (open / in_progress / done / cancelled), `TaskPriority`
  (low / normal / high / urgent), `due_at`, `checklist: Vec<ChecklistItem>`,
  and an optional `notes` field.
- `nexus_core::projects::ui_state` — typed JSON key/value store
  (`ACTIVE_PROJECT` constant).
- `nexus_core::memory` — `Memory` type with `MemoryScope` (user /
  project / auto / conv), approve / edit / delete, and `list_for_context`
  which is the one used by the chat pipeline.
- `nexus_core::projects::context` — `build_project_context_block` and
  `build_memory_context_block` build the markdown that gets prepended
  to the system prompt at chat time.

### Tauri commands
31 new commands grouped by feature:

| Feature    | Commands |
|------------|----------|
| workspaces | `workspaces_list`, `workspaces_create`, `workspaces_rename`, `workspaces_delete` |
| projects   | `projects_list`, `projects_get`, `projects_create`, `projects_update`, `projects_archive`, `projects_delete`, `projects_summaries` |
| notes      | `notes_list`, `notes_get`, `notes_create`, `notes_update`, `notes_delete` |
| tasks      | `tasks_list`, `tasks_due_in_range`, `tasks_get`, `tasks_create`, `tasks_update`, `tasks_set_status`, `tasks_delete` |
| memory     | `memory_list`, `memory_create`, `memory_update`, `memory_approve`, `memory_delete` |
| ui state   | `ui_state_get`, `ui_state_set`, `ui_state_unset`, `get_active_project`, `set_active_project` |

`chat_create_conversation` gains an optional `project_id` argument.
The dashboard summary now includes `active_project` and `projects_total`.

### Project context for chat
Before the system prompt is finalised in `chat_send`, two extra
markdown blocks are appended (after the user's own system prompt and
before the vault citations):

```
## Project: <name>
<description>

### Instructions
<instructions>           ← labelled as firm project rules

### Goals
- goal 1
- goal 2

### Recent notes (top 3 by recency)
- **title** — first 600 chars of the body
```

```
## Persistent memory
The following are user-approved facts. Honour them unless the user
explicitly overrides them.
- (user): Loves concise answers.
- (project): Project is about the Roman Empire.
- (this conversation): Prefers bullet lists.
```

`scope` rules for `list_for_context`:
- `user` and `auto` (once approved) — always.
- `project` — when the conversation's `project_id` matches, or no
  project is set.
- `conv` — when the conversation id matches.

### Web
- New typed IPC wrappers in `web/src/lib/projects.ts` (workspaces,
  projects, notes, tasks, memory, ui_state).
- `lib/chat.ts` — `createConversation` accepts an optional `projectId`.
- `lib/ipc.ts` — `DashboardSummary` now has `active_project` and
  `projects_total`.
- New pages:
  - **Projects** (`/projects`) — three-pane layout: workspaces
    sidebar, projects list, project editor. The editor exposes
    name, workspace, description, instructions, goals, and
    archive/delete. "Set active" / "Deactivate" sets the active
    project pointer; the active state shows up as a chip on the
    project card and in the dashboard banner.
  - **Tasks** (`/tasks`) — Board view (kanban-style columns for
    open / in_progress / done / cancelled) and Calendar view
    (14-day strip with due-date chips). Inline status toggles,
    full editor modal with project, priority, due date, and
    checklist editor. The view can be scoped to the active project
    (default), all projects, or a single project.
  - **Notes** (`/documents`) — Note list with project filter and
    text search, plus a full editor with Edit / Preview tabs that
    reuse the existing `MarkdownView` component. A note is created
    with the active project pre-attached.
- The Assistant page shows an "active project" chip next to the
  "use vault" toggle. The chip has a × that clears the active
  project. New conversations are created with `project_id` set
  to the active project.

## How to use it
1. Open **Projects** → click **+ New workspace** (optional) → click
   **+ New project** → fill in name, instructions, and goals.
2. Click **Set active** on the project. The dashboard shows the
   active project and the Assistant composer shows the project chip.
3. Open **Notes** → click **+ New note** to capture project-scoped
   facts. The three most recent notes are prepended to chat context.
4. Open **Tasks** → click **+ New task** → pick project, priority,
   due date. Use the board to triage or the calendar for the next
   two weeks. Toggle a task to **done** with the inline ✓ link.
5. New conversations in the Assistant automatically attach to the
   active project. Switch projects in the **Projects** page; existing
   conversations keep their original project link.

## Tests

`tests/phase5.rs` (9 unit tests, all pass):

| Test | Covers |
|------|--------|
| `workspace_crud_round_trip` | create / rename / list / delete |
| `project_crud_with_goals_and_instructions` | create with goals, partial update, archive filter |
| `notes_crud_and_project_filter` | per-project list, full list, update |
| `tasks_crud_with_status_checklist_due` | create with checklist, set_status, due-in-range |
| `memory_crud_and_approval_gating` | unapproved memory hidden from `list` with `approved_only=true` |
| `ui_state_round_trip` | set / get / unset typed JSON values |
| `conversation_attaches_to_project` | `set_project` round-trip |
| `project_context_block_surfaces_goals_and_notes` | markdown block contains instructions / goals / notes |
| `memory_context_block_includes_approved_only` | unapproved memories excluded from context block |

Full test summary:
```
Total: 34 tests passed
  phase2    4
  phase2_live 0
  providers 0
  phase1_live 1
  phase4_live 1
  phase3    2
  phase4    5
  phase5    9
  ...
```

## Phase 5 spec coverage

- [x] Workspaces, projects, notes, tags, collections, favorites.
      (Tags, collections, and favorites stay on the roadmap for
      Phase 9 alongside document intelligence. The `notes` table
      already supports an `id` for the upcoming `note_tags` join.)
- [x] Tasks with checklist, priority, due date, project link.
- [x] Calendar-style view.
- [x] Memory scopes; approve / edit / delete a memory.
- [x] Project context is auto-attached to chat turns in that project.
