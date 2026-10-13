// One-Click Work Resume — Phase S.2.
//
// Three views in one page: a list of saved checkpoints per
// project, a "create new checkpoint" form, and the resume
// summary that includes the drift report and the deterministic
// text rendering the Rust backend produces.
//
// Every action that mutates state calls a Tauri command and
// waits for the result. Resuming does NOT execute any side
// effects: it is a read of the source tables, an in-memory
// drift comparison, and a write of a `checkpoint_resumes` row.

import { useEffect, useMemo, useState } from "react";
import {
  type Checkpoint,
  type CreateCheckpointRequest,
  type ObservedFile,
  type ResumeResponse,
  checkpointsCreate,
  checkpointsDelete,
  checkpointsGet,
  checkpointsList,
  checkpointsListResumes,
  checkpointsResume,
  checkpointsSetStatus,
} from "../lib/checkpoints";
import { type Project, getActiveProject, projectsList } from "../lib/projects";

function formatMs(ms: number | null | undefined): string {
  if (!ms) return "—";
  const d = new Date(ms);
  if (Number.isNaN(d.getTime())) return String(ms);
  return d.toLocaleString();
}

function emptyCreate(projectId: string): CreateCheckpointRequest {
  return {
    project_id: projectId,
    task_id: null,
    title: "",
    goal: "",
    completed_work: "",
    current_state: "",
    next_action_text: "",
    observed_files: [],
    observed_tests: [],
    errors: [],
    decisions: [],
    constraints: [],
    pending_actions: [],
    evidence_refs: [],
    created_by: "user",
    model_hint: null,
  };
}

export function CheckpointsPage() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [active, setActive] = useState<Project | null>(null);
  const [projectId, setProjectId] = useState<string | null>(null);
  const [items, setItems] = useState<Checkpoint[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);
  const [creating, setCreating] = useState<boolean>(false);
  const [draft, setDraft] = useState<CreateCheckpointRequest | null>(null);
  const [resuming, setResuming] = useState<ResumeResponse | null>(null);
  const [resumesFor, setResumesFor] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const [ps, ap] = await Promise.all([projectsList(false), getActiveProject()]);
        setProjects(ps);
        setActive(ap ?? null);
        const initial = ap?.id ?? ps[0]?.id ?? null;
        setProjectId(initial);
      } catch (e) { setError(String(e)); }
    })();
  }, []);

  useEffect(() => {
    if (!projectId) { setItems([]); return; }
    void refresh(projectId);
  }, [projectId]);

  async function refresh(pid: string) {
    setError(null);
    try {
      const list = await checkpointsList({ project_id: pid, limit: 100 });
      setItems(list);
    } catch (e) { setError(String(e)); }
  }

  function startCreate() {
    if (!projectId) return;
    setDraft(emptyCreate(projectId));
    setCreating(true);
    setResuming(null);
  }

  async function saveCreate() {
    if (!draft || !draft.title.trim()) { setError("Title is required."); return; }
    setBusy(true);
    setError(null);
    try {
      await checkpointsCreate(draft);
      setCreating(false);
      setDraft(null);
      if (projectId) await refresh(projectId);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function doResume(id: string) {
    setBusy(true);
    setError(null);
    try {
      const r = await checkpointsResume(id, null);
      setResuming(r);
      if (projectId) await refresh(projectId);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function doArchive(id: string) {
    setBusy(true);
    setError(null);
    try {
      await checkpointsSetStatus(id, "archived");
      if (projectId) await refresh(projectId);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function doDelete(id: string) {
    setBusy(true);
    setError(null);
    try {
      await checkpointsDelete(id);
      setResuming(null);
      if (projectId) await refresh(projectId);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function openResumes(id: string) {
    setResumesFor(id);
    setResuming(null);
  }

  const activeProjectName = useMemo(() => {
    return projects.find((p) => p.id === projectId)?.name ?? "—";
  }, [projects, projectId]);

  return (
    <div className="p-6 max-w-7xl space-y-6">
      <div className="panel p-5">
        <div className="flex items-start gap-3">
          <div className="flex-1">
            <div className="h-title">Work Resume</div>
            <p className="mt-1 text-sm text-slate-400">
              Save a structured checkpoint of where work stopped. Resuming later
              surfaces the goal, the current state, the observed files, the
              observed test results, the open errors, the decisions on record,
              and the pending actions — plus a drift report that lists what
              changed since the checkpoint was created. The checkpoint never
              modifies the source tables.
            </p>
          </div>
          <div className="text-right text-[11px] text-slate-500">
            {active && <><div>Active</div><div className="text-slate-300">{active.name}</div></>}
          </div>
        </div>
        <div className="mt-4 grid grid-cols-1 md:grid-cols-3 gap-3">
          <label className="text-xs text-slate-400">
            Project
            <select
              className="select w-full mt-1"
              value={projectId ?? ""}
              onChange={(e) => setProjectId(e.target.value || null)}
            >
              {projects.length === 0 && <option value="">No projects</option>}
              {projects.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
          </label>
          <div className="md:col-span-2 flex items-end gap-2">
            <button className="btn-primary" disabled={!projectId || busy} onClick={startCreate}>
              New checkpoint
            </button>
            <button className="btn" disabled={!projectId || busy} onClick={() => projectId && void refresh(projectId)}>
              Refresh
            </button>
          </div>
        </div>
        {error && <div className="mt-3 text-sm text-rose-400">{error}</div>}
      </div>

      {creating && draft && (
        <CheckpointDraft
          draft={draft}
          setDraft={setDraft}
          onCancel={() => { setCreating(false); setDraft(null); }}
          onSave={saveCreate}
          busy={busy}
        />
      )}

      {resuming && (
        <div className="panel p-5">
          <div className="flex items-start gap-3">
            <div className="flex-1">
              <div className="h-subtitle">Resume: {resuming.checkpoint.title}</div>
              <p className="mt-1 text-[12px] text-slate-400">
                The summary below is rendered by the Rust backend. The drift
                report tells you what changed since the checkpoint was
                created.
              </p>
            </div>
            <button className="btn" onClick={() => setResuming(null)}>Close</button>
          </div>
          <pre className="mt-3 whitespace-pre-wrap text-[12px] text-slate-200 font-mono bg-ink-800/60 rounded p-3 border border-white/5">
{resuming.summary}
          </pre>
          {resuming.drift_report.items.length > 0 ? (
            <div className="mt-3">
              <div className="h-subtitle">Drift items</div>
              <ul className="mt-2 space-y-1 text-[12px]">
                {resuming.drift_report.items.map((it, i) => (
                  <li key={i} className="flex items-start gap-2">
                    <span className={`chip ${severityChipClass(it.severity)}`}>{it.severity}</span>
                    <span className="text-slate-300">{it.summary}</span>
                  </li>
                ))}
              </ul>
            </div>
          ) : (
            <p className="mt-3 text-[12px] text-slate-500">No drift detected since the checkpoint was saved.</p>
          )}
        </div>
      )}

      <div className="panel p-5">
        <div className="h-subtitle">Checkpoints for {activeProjectName}</div>
        {!projectId ? (
          <p className="mt-2 text-sm text-slate-500">Pick a project to begin.</p>
        ) : items.length === 0 ? (
          <p className="mt-2 text-sm text-slate-500">
            No checkpoints yet. Click <span className="text-slate-300">New checkpoint</span> to capture the current state.
          </p>
        ) : (
          <ul className="mt-3 divide-y divide-white/5 border border-white/5 rounded-lg">
            {items.map((c) => (
              <li key={c.id} className="p-3">
                <div className="flex items-center gap-2">
                  <div className="text-sm text-slate-200 flex-1">{c.title}</div>
                  <span className="chip">{c.status}</span>
                  <span className="text-[10px] text-slate-500 font-mono">{formatMs(c.created_at)}</span>
                </div>
                {c.goal && <div className="mt-1 text-[12px] text-slate-400">Goal: {c.goal}</div>}
                {c.next_action_text && (
                  <div className="mt-1 text-[12px] text-cyan2-400">Next: {c.next_action_text}</div>
                )}
                <div className="mt-1 text-[10px] text-slate-500 font-mono">
                  {c.observed_files.length > 0 && <span>{c.observed_files.length} file(s) · </span>}
                  {c.observed_tests.length > 0 && <span>{c.observed_tests.length} test(s) · </span>}
                  {c.decisions.length > 0 && <span>{c.decisions.length} decision(s) · </span>}
                  {c.errors.length > 0 && <span>{c.errors.length} error(s) · </span>}
                  {c.pending_actions.length > 0 && <span>{c.pending_actions.length} pending action(s) · </span>}
                  {c.evidence_refs.length > 0 && <span>{c.evidence_refs.length} evidence ref(s)</span>}
                </div>
                <div className="mt-2 flex flex-wrap gap-2" onClick={(e) => e.stopPropagation()}>
                  <button className="btn-primary" disabled={busy} onClick={() => void doResume(c.id)}>
                    Resume
                  </button>
                  <button className="btn" disabled={busy} onClick={() => void openResumes(c.id)}>
                    History
                  </button>
                  {c.status !== "archived" && (
                    <button className="btn" disabled={busy} onClick={() => void doArchive(c.id)}>
                      Archive
                    </button>
                  )}
                  <button className="btn" disabled={busy} onClick={() => void doDelete(c.id)}>
                    Delete
                  </button>
                </div>
                {resumesFor === c.id && <ResumesList checkpointId={c.id} />}
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function severityChipClass(s: string): string {
  if (s === "conflict") return "text-rose2-400 border-rose2-400/40 bg-rose2-400/10";
  if (s === "warn") return "text-amber2-400 border-amber2-400/40 bg-amber2-400/10";
  return "text-slate-300 border-white/10";
}

function CheckpointDraft({
  draft, setDraft, onCancel, onSave, busy,
}: {
  draft: CreateCheckpointRequest;
  setDraft: (d: CreateCheckpointRequest) => void;
  onCancel: () => void;
  onSave: () => void;
  busy: boolean;
}) {
  const [filePath, setFilePath] = useState<string>("");
  const [testName, setTestName] = useState<string>("");
  const [decisionSummary, setDecisionSummary] = useState<string>("");
  const [pendingText, setPendingText] = useState<string>("");

  function addFile() {
    if (!filePath.trim()) return;
    const next: ObservedFile = { path: filePath.trim(), label: null, sha256: null, size_bytes: null, mtime_ms: null };
    setDraft({ ...draft, observed_files: [...draft.observed_files, next] });
    setFilePath("");
  }

  function addTest() {
    if (!testName.trim()) return;
    setDraft({
      ...draft,
      observed_tests: [...draft.observed_tests, { name: testName.trim(), outcome: "passed", note: null, at_ms: Date.now() }],
    });
    setTestName("");
  }

  function addDecision() {
    if (!decisionSummary.trim()) return;
    setDraft({
      ...draft,
      decisions: [...draft.decisions, { summary: decisionSummary.trim(), rationale: null, memory_id: null }],
    });
    setDecisionSummary("");
  }

  function addPending() {
    if (!pendingText.trim()) return;
    setDraft({
      ...draft,
      pending_actions: [...draft.pending_actions, { text: pendingText.trim(), done: false }],
    });
    setPendingText("");
  }

  return (
    <div className="panel p-5 space-y-3">
      <div className="h-subtitle">New checkpoint</div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        <label className="text-xs text-slate-400 md:col-span-2">
          Title <span className="text-rose2-400">*</span>
          <input
            className="input w-full mt-1"
            value={draft.title}
            onChange={(e) => setDraft({ ...draft, title: e.target.value })}
            placeholder="e.g. After S.2 schema work"
          />
        </label>
        <label className="text-xs text-slate-400">
          Goal
          <textarea
            className="input w-full mt-1 min-h-[3.5rem]"
            value={draft.goal ?? ""}
            onChange={(e) => setDraft({ ...draft, goal: e.target.value })}
          />
        </label>
        <label className="text-xs text-slate-400">
          Completed work
          <textarea
            className="input w-full mt-1 min-h-[3.5rem]"
            value={draft.completed_work ?? ""}
            onChange={(e) => setDraft({ ...draft, completed_work: e.target.value })}
          />
        </label>
        <label className="text-xs text-slate-400">
          Current state
          <textarea
            className="input w-full mt-1 min-h-[3.5rem]"
            value={draft.current_state ?? ""}
            onChange={(e) => setDraft({ ...draft, current_state: e.target.value })}
          />
        </label>
        <label className="text-xs text-slate-400">
          Recommended next step
          <textarea
            className="input w-full mt-1 min-h-[3.5rem]"
            value={draft.next_action_text ?? ""}
            onChange={(e) => setDraft({ ...draft, next_action_text: e.target.value })}
          />
        </label>
      </div>

      <div>
        <div className="text-xs text-slate-400">Observed files (paths only — never copied)</div>
        <div className="mt-1 flex gap-2">
          <input
            className="input flex-1 font-mono"
            value={filePath}
            onChange={(e) => setFilePath(e.target.value)}
            placeholder="D:\NEXUS\app\src-tauri\src\lib.rs"
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addFile(); } }}
          />
          <button className="btn" onClick={addFile}>Add</button>
        </div>
        {draft.observed_files.length > 0 && (
          <ul className="mt-2 space-y-1 text-[12px] font-mono text-slate-300">
            {draft.observed_files.map((f, i) => (
              <li key={i} className="flex items-center gap-2">
                <span className="truncate flex-1">{f.path}</span>
                <button
                  className="text-slate-500 hover:text-rose2-400"
                  onClick={() => setDraft({
                    ...draft,
                    observed_files: draft.observed_files.filter((_, j) => j !== i),
                  })}
                >remove</button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div>
        <div className="text-xs text-slate-400">Observed tests</div>
        <div className="mt-1 flex gap-2">
          <input
            className="input flex-1"
            value={testName}
            onChange={(e) => setTestName(e.target.value)}
            placeholder="phase_s2_crud"
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addTest(); } }}
          />
          <button className="btn" onClick={addTest}>Add</button>
        </div>
        {draft.observed_tests.length > 0 && (
          <ul className="mt-2 space-y-1 text-[12px] text-slate-300">
            {draft.observed_tests.map((t, i) => (
              <li key={i} className="flex items-center gap-2">
                <span className="chip">{t.outcome}</span>
                <span className="font-mono">{t.name}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div>
        <div className="text-xs text-slate-400">Decisions</div>
        <div className="mt-1 flex gap-2">
          <input
            className="input flex-1"
            value={decisionSummary}
            onChange={(e) => setDecisionSummary(e.target.value)}
            placeholder="Use filesystem mtimes for v1 drift."
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addDecision(); } }}
          />
          <button className="btn" onClick={addDecision}>Add</button>
        </div>
        {draft.decisions.length > 0 && (
          <ul className="mt-2 space-y-1 text-[12px] text-slate-300">
            {draft.decisions.map((d, i) => (
              <li key={i}>· {d.summary}</li>
            ))}
          </ul>
        )}
      </div>

      <div>
        <div className="text-xs text-slate-400">Pending actions</div>
        <div className="mt-1 flex gap-2">
          <input
            className="input flex-1"
            value={pendingText}
            onChange={(e) => setPendingText(e.target.value)}
            placeholder="Write the resume summary test."
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addPending(); } }}
          />
          <button className="btn" onClick={addPending}>Add</button>
        </div>
        {draft.pending_actions.length > 0 && (
          <ul className="mt-2 space-y-1 text-[12px] text-slate-300">
            {draft.pending_actions.map((p, i) => (
              <li key={i}>· {p.text}</li>
            ))}
          </ul>
        )}
      </div>

      <div className="flex items-center gap-2">
        <button className="btn-primary" disabled={busy || !draft.title.trim()} onClick={onSave}>
          {busy ? "Saving…" : "Save checkpoint"}
        </button>
        <button className="btn" onClick={onCancel}>Cancel</button>
      </div>
    </div>
  );
}

function ResumesList({ checkpointId }: { checkpointId: string }) {
  const [records, setRecords] = useState<ResumeResponse[]>([]);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    (async () => {
      try {
        const recs = await checkpointsListResumes(checkpointId);
        // We only have ResumeRecord here, but the page holds the
        // most recent ResumeResponse in state. Fetch each
        // record's drift report through the same Tauri call.
        const full: ResumeResponse[] = [];
        for (const r of recs) {
          // We don't have a `resume` for past records. Use the
          // checkpoint_get for the row and synthesise a minimal
          // response with the stored drift report.
          const cp = await checkpointsGet(checkpointId).catch(() => null);
          if (cp) {
            full.push({
              checkpoint: cp,
              drift_report: r.drift_report,
              summary: "",
              resume_record_id: r.id,
            });
          }
        }
        setRecords(full);
      } catch (e) { setErr(String(e)); }
    })();
  }, [checkpointId]);
  if (err) return <div className="mt-2 text-[12px] text-rose-400">{err}</div>;
  if (records.length === 0) {
    return <p className="mt-2 text-[12px] text-slate-500">No resume attempts recorded yet.</p>;
  }
  return (
    <ul className="mt-2 space-y-1 text-[12px] text-slate-300">
      {records.map((r) => (
        <li key={r.resume_record_id} className="font-mono">
          {formatMs(r.drift_report.generated_at)} · {r.drift_report.items.length} drift item(s) · {r.drift_report.files_now.length} file(s) re-checked
        </li>
      ))}
    </ul>
  );
}
