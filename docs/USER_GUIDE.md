# NEXUS — User Guide

NEXUS is a local-first AI workbench. It bundles:

* a chat assistant grounded in your local knowledge vault
* a long-term memory system (Native v2, plus an optional MemPalace sidecar)
* a knowledge graph for entities, aliases, and relationships
* a Research Studio for literature reviews and comparison matrices
* an Experiment Tracking module with metric history charts
* a Controlled Agent with a permission-gated tool registry
* a Knowledge Vault that ingests files, chunks them, and indexes
  them with FTS5 and (optionally) embeddings
* Document Intelligence for table extraction, CSV/TSV export, and diffs

Everything is stored on your machine. No data leaves the host unless
you explicitly point a model profile at a remote endpoint.

## First-run checklist

1. **Start LM Studio** (or llama.cpp / Ollama). NEXUS expects at
   least one HTTP model server on `http://127.0.0.1:1234`. See
   <https://lmstudio.ai/> for installation.
2. **Open the Command Center**. It is the default landing page. You
   will see provider health and a "model probe" button; click it to
   have NEXUS list the models the running server is serving.
3. **Add a model profile**. The Models page converts a probed model
   into a profile with a role (`general` / `coding` / `embedding`),
   a context length, a sampling temperature, and a max-tokens cap.
4. **Point the Vault at a directory**. The Knowledge page accepts a
   folder or a single file. Folders are walked recursively; chunks
   are written to `documents` and FTS5; vectors are filled in by the
   embeddings backfill if you have an embedding profile configured.
5. **Ask the assistant something**. The Assistant page sends the
   active model's reply as a stream and, when the vault is enabled,
   prepends the most relevant chunks as citations.

## The five modules in plain English

### Knowledge

A search-first view of the vault. Free-text search uses FTS5; vector
search kicks in when an embedding profile is configured. The Knowledge
Graph tab shows the entities extracted from memory and research
projects; click a node to recenter, right-click an edge to see
evidence.

### Memory

Three things in one place:

* **Approved memories** — long-term facts the model can recall.
* **Candidates** — auto-extracted statements waiting for review.
* **Sessions** — end-of-conversation summaries you can save.

Use the search box to find an existing memory; click "Open source" to
jump to the conversation that produced it.

### Research Studio

A workspace per literature question. Add papers, extract claims,
build a comparison matrix, and generate a Markdown report. Reports
can be copied into a notes app or saved as a project snapshot.

### Experiments

A "what I tried" log. Create an experiment, then add runs with
hyperparameters, metrics, code revisions, and notes. The metric
history panel plots a single metric across runs; switch metrics in
the dropdown to see different views.

### Agent

A tool registry with four permission levels (`always`, `ask`,
`restricted`, `deny`). The page shows every registered tool and
every recorded invocation. "Ask" tools require explicit approval
before they execute. Every action is written to the audit log on the
Monitor page.

## Keyboard shortcuts

| Shortcut            | Action              |
|---------------------|---------------------|
| `Ctrl/Cmd + K`      | Open command palette|
| `Ctrl/Cmd + B`      | Toggle sidebar      |
| `Enter` in chat     | Send                |
| `Shift + Enter`     | Newline             |
| `Esc`               | Close palette       |

## How to read the Monitor page

The Monitor page samples CPU and RAM every 3 seconds. It does not
fabricate GPU numbers: if the host does not expose GPU telemetry,
the panel shows "unknown" rather than guessing.

The "Audit log" panel lists the most recent privileged actions
(agent tool runs, router decisions, model profile changes, vault
ingests). The list is virtualised, so even 10,000 events scroll
smoothly.

## Privacy

* No telemetry leaves the machine.
* Models run on the local GPU/CPU.
* File reads are allowlist-based (see `agent.policy`).
* The agent's "shell.run" tool is registered as `restricted` by
  default and is rejected by `policy::check_command_allowed` unless
  the command is in the explicit allowlist.
