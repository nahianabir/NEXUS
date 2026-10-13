// Phase S.3 page. Three features in one route, switched by
// tabs: Opportunity Radar, Hypothesis Ledger, Codebase
// Archaeologist. Every action is a thin wrapper around a
// Tauri command; no in-memory shadow state.

import { useEffect, useMemo, useState } from "react";
import {
  type EvidenceCard,
  type Hypothesis,
  type HypothesisEvidence,
  type HypothesisStatus,
  type ImpactAction,
  type ImpactCommandRequest,
  type ImpactReport as ImpactReportT,
  type IndexSummary,
  type RadarFeedbackRequest,
  type RadarReport,
  type SerendipityReport,
  type Stance,
  type TribunalReport,
  type TribunalRequest,
  archaeologistHead,
  archaeologistIndex,
  evidenceTribunalRun,
  hypothesesAddEvidence,
  hypothesesCreate,
  hypothesesDecide,
  hypothesesDelete,
  hypothesesList,
  hypothesesListEvidence,
  impactSimulate,
  radarFeedbackUpsert,
  radarReport,
  serendipityReport,
} from "../lib/s3";
import { type Project, getActiveProject, projectsList } from "../lib/projects";
import { usePromptDialog } from "../hooks/usePromptDialog";

type Tab = "radar" | "hypotheses" | "archaeologist" | "serendipity" | "impact" | "tribunal";

export function OpportunityRadarPage() {
  const [tab, setTab] = useState<Tab>("radar");
  return (
    <div className="p-6 max-w-7xl space-y-4">
      <div className="panel p-4 flex items-center gap-2 flex-wrap">
        <button
          className={tab === "radar" ? "btn-primary" : "btn"}
          onClick={() => setTab("radar")}
        >Radar</button>
        <button
          className={tab === "hypotheses" ? "btn-primary" : "btn"}
          onClick={() => setTab("hypotheses")}
        >Hypotheses</button>
        <button
          className={tab === "serendipity" ? "btn-primary" : "btn"}
          onClick={() => setTab("serendipity")}
        >Serendipity</button>
        <button
          className={tab === "impact" ? "btn-primary" : "btn"}
          onClick={() => setTab("impact")}
        >Impact</button>
        <button
          className={tab === "archaeologist" ? "btn-primary" : "btn"}
          onClick={() => setTab("archaeologist")}
        >Archaeologist</button>
        <button
          className={tab === "tribunal" ? "btn-primary" : "btn"}
          onClick={() => setTab("tribunal")}
        >Tribunal</button>
      </div>
      {tab === "radar" && <RadarTab />}
      {tab === "hypotheses" && <HypothesesTab />}
      {tab === "serendipity" && <SerendipityTab />}
      {tab === "impact" && <ImpactTab />}
      {tab === "archaeologist" && <ArchaeologistTab />}
      {tab === "tribunal" && <TribunalTab />}
    </div>
  );
}

// ---------------- Radar ----------------

function RadarTab() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [, setActive] = useState<Project | null>(null);
  const [projectId, setProjectId] = useState<string | null>(null);
  const [report, setReport] = useState<RadarReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);

  useEffect(() => {
    (async () => {
      try {
        const [ps, ap] = await Promise.all([projectsList(false), getActiveProject()]);
        setProjects(ps);
        setActive(ap ?? null);
        const initial = ap?.id ?? ps[0]?.id ?? null;
        setProjectId(initial);
        if (initial) await runReport(initial);
      } catch (e) { setError(String(e)); }
    })();
  }, []);

  async function runReport(pid: string) {
    setBusy(true);
    setError(null);
    try {
      const r = await radarReport({ project_id: pid });
      setReport(r);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  return (
    <div className="panel p-5 space-y-4">
      <div className="flex items-center gap-3">
        <div className="flex-1">
          <div className="h-title">Opportunity Radar</div>
          <p className="mt-1 text-sm text-slate-400">
            What to look at next. Read-only â€” the radar never
            modifies the source tables.
          </p>
        </div>
        <select
          className="select"
          value={projectId ?? ""}
          onChange={(e) => {
            const v = e.target.value || null;
            setProjectId(v);
            if (v) void runReport(v);
          }}
        >
          {projects.length === 0 && <option value="">No projects</option>}
          {projects.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
        </select>
        <button className="btn" disabled={busy || !projectId} onClick={() => projectId && runReport(projectId)}>
          {busy ? "Scanningâ€¦" : "Scan"}
        </button>
      </div>
      {error && <div className="text-sm text-rose-400">{error}</div>}
      {report && (
        <>
          <Counts counts={report.counts} />
          <ul className="divide-y divide-white/5 border border-white/5 rounded-lg">
            {report.items.length === 0 && (
              <li className="p-3 text-sm text-slate-500">No opportunities detected.</li>
            )}
            {report.items.map((o, i) => (
              <li key={i} className="p-3 flex items-center gap-3">
                <span className="chip">{o.kind.replace(/_/g, " ")}</span>
                <div className="flex-1">
                  <div className="text-sm text-slate-200">{o.title}</div>
                  <div className="text-[12px] text-slate-400">{o.summary}</div>
                  <div className="text-[11px] text-cyan2-400 mt-1">{o.suggested_action}</div>
                </div>
                <div className="text-right">
                  <div className="text-[11px] text-slate-500 font-mono">score</div>
                  <div className="text-sm text-slate-200">{o.score.toFixed(1)}</div>
                  <div className="mt-1 flex gap-1">
                    <button className="text-[10px] px-2 py-0.5 rounded border border-white/10 hover:bg-white/5"
                      onClick={async () => {
                        try {
                          const req: RadarFeedbackRequest = { project_id: projectId ?? "", key: o.key, kind: "dismiss" };
                          await radarFeedbackUpsert(req);
                          if (projectId) await runReport(projectId);
                        } catch (e: unknown) { setError(String(e)); }
                      }}
                    >Dismiss</button>
                    <button className="text-[10px] px-2 py-0.5 rounded border border-white/10 hover:bg-white/5"
                      onClick={async () => {
                        try {
                          const req: RadarFeedbackRequest = { project_id: projectId ?? "", key: o.key, kind: "pin" };
                          await radarFeedbackUpsert(req);
                          if (projectId) await runReport(projectId);
                        } catch (e: unknown) { setError(String(e)); }
                      }}
                    >Pin</button>
                  </div>
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
      {!report && !error && <p className="text-sm text-slate-500">Pick a project to scan.</p>}
    </div>
  );
}

function Counts({ counts }: { counts: RadarReport["counts"] }) {
  const items: [string, number][] = [
    ["open tasks", counts.open_tasks],
    ["overdue tasks", counts.overdue_tasks],
    ["resumed checkpoints", counts.resumed_checkpoints],
    ["open hypotheses", counts.open_hypotheses],
    ["high-importance memories", counts.high_importance_memories],
  ];
  return (
    <div className="grid grid-cols-2 md:grid-cols-5 gap-2">
      {items.map(([k, v]) => (
        <div key={k} className="rounded border border-white/5 bg-ink-800/40 p-2">
          <div className="text-[10px] text-slate-500 font-mono">{k}</div>
          <div className="text-base text-slate-200">{v}</div>
        </div>
      ))}
    </div>
  );
}

// ---------------- Hypotheses ----------------

function HypothesesTab() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [, setActive] = useState<Project | null>(null);
  const [projectId, setProjectId] = useState<string | null>(null);
  const [items, setItems] = useState<Hypothesis[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);
  const { confirm: confirmModal } = usePromptDialog();
  const [draft, setDraft] = useState<{ title: string; statement: string; prediction: string; confidence: number } | null>(null);

  const [selected, setSelected] = useState<Hypothesis | null>(null);
  const [evidence, setEvidence] = useState<HypothesisEvidence[]>([]);

  useEffect(() => {
    (async () => {
      try {
        const [ps, ap] = await Promise.all([projectsList(false), getActiveProject()]);
        setProjects(ps);
        setActive(ap ?? null);
        const initial = ap?.id ?? ps[0]?.id ?? null;
        setProjectId(initial);
        if (initial) await refresh(initial);
      } catch (e) { setError(String(e)); }
    })();
  }, []);

  async function refresh(pid: string) {
    setBusy(true);
    setError(null);
    try {
      const list = await hypothesesList({ project_id: pid, limit: 100 });
      setItems(list);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function saveDraft() {
    if (!projectId || !draft || !draft.title.trim() || !draft.statement.trim()) {
      setError("Title and statement are required.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await hypothesesCreate({
        project_id: projectId,
        title: draft.title.trim(),
        statement: draft.statement.trim(),
        prediction: draft.prediction.trim() || null,
        confidence: draft.confidence,
        evidence_for: [],
        evidence_against: [],
        memory_id: null,
      });
      setDraft(null);
      await refresh(projectId);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function decide(id: string, d: HypothesisStatus) {
    setBusy(true);
    setError(null);
    try {
      await hypothesesDecide(id, d, null);
      if (projectId) await refresh(projectId);
      if (selected?.id === id) setSelected(null);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function remove(id: string) {
    const ok = await confirmModal({
      title: "Delete hypothesis",
      description: "Delete this hypothesis? The source records are not affected.",
      submitLabel: "Delete hypothesis",
      destructive: true,
    });
    if (!ok) return;
    setBusy(true);
    setError(null);
    try {
      await hypothesesDelete(id);
      if (selected?.id === id) setSelected(null);
      if (projectId) await refresh(projectId);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function openSelected(h: Hypothesis) {
    setSelected(h);
    setEvidence([]);
    try {
      const evs = await hypothesesListEvidence(h.id);
      setEvidence(evs);
    } catch (e) { setError(String(e)); }
  }

  return (
    <div className="panel p-5 space-y-4">
      <div className="flex items-center gap-3">
        <div className="flex-1">
          <div className="h-title">Hypothesis Ledger</div>
          <p className="mt-1 text-sm text-slate-400">
            Track claims you can confirm or refute. The ledger
            is fully additive; deleting a hypothesis never
            touches the source records.
          </p>
        </div>
        <select
          className="select"
          value={projectId ?? ""}
          onChange={(e) => {
            const v = e.target.value || null;
            setProjectId(v);
            if (v) void refresh(v);
          }}
        >
          {projects.length === 0 && <option value="">No projects</option>}
          {projects.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
        </select>
        <button className="btn-primary" disabled={!projectId} onClick={() => setDraft({ title: "", statement: "", prediction: "", confidence: 0.5 })}>
          New hypothesis
        </button>
      </div>

      {error && <div className="text-sm text-rose-400">{error}</div>}

      {draft && (
        <div className="panel p-3 space-y-2">
          <div className="text-sm text-slate-200">New hypothesis</div>
          <input
            className="input w-full"
            placeholder="Title"
            value={draft.title}
            onChange={(e) => setDraft({ ...draft, title: e.target.value })}
          />
          <textarea
            className="input w-full min-h-[4rem]"
            placeholder="Statement (the claim)"
            value={draft.statement}
            onChange={(e) => setDraft({ ...draft, statement: e.target.value })}
          />
          <textarea
            className="input w-full min-h-[3rem]"
            placeholder="Prediction (optional)"
            value={draft.prediction}
            onChange={(e) => setDraft({ ...draft, prediction: e.target.value })}
          />
          <label className="text-xs text-slate-400 block">
            Confidence: <span className="text-slate-200 font-mono">{draft.confidence.toFixed(2)}</span>
            <input
              type="range" min={0} max={1} step={0.05}
              className="w-full mt-1"
              value={draft.confidence}
              onChange={(e) => setDraft({ ...draft, confidence: parseFloat(e.target.value) })}
            />
          </label>
          <div className="flex gap-2">
            <button className="btn-primary" disabled={busy} onClick={saveDraft}>Save</button>
            <button className="btn" onClick={() => setDraft(null)}>Cancel</button>
          </div>
        </div>
      )}

      {selected && (
        <SelectedHypothesis
          h={selected}
          evidence={evidence}
          onDecide={(d) => decide(selected.id, d)}
          onDelete={() => remove(selected.id)}
          onClose={() => setSelected(null)}
        />
      )}

      <ul className="divide-y divide-white/5 border border-white/5 rounded-lg">
        {items.length === 0 && (
          <li className="p-3 text-sm text-slate-500">No hypotheses for this project yet.</li>
        )}
        {items.map((h) => (
          <li key={h.id} className="p-3 flex items-center gap-3">
            <span className={`chip ${statusChipClass(h.status)}`}>{h.status}</span>
            <div className="flex-1 min-w-0">
              <div className="text-sm text-slate-200 truncate">{h.title}</div>
              <div className="text-[12px] text-slate-400 line-clamp-2">{h.statement}</div>
            </div>
            <div className="text-right text-[11px] text-slate-500 font-mono">conf {h.confidence.toFixed(2)}</div>
            <button className="btn" onClick={() => openSelected(h)}>Open</button>
            {h.status === "open" && (
              <>
                <button className="btn" onClick={() => decide(h.id, "confirmed")}>Confirm</button>
                <button className="btn" onClick={() => decide(h.id, "refuted")}>Refute</button>
              </>
            )}
            <button className="btn" onClick={() => remove(h.id)}>Delete</button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function statusChipClass(s: string): string {
  if (s === "confirmed") return "text-emerald-300 border-emerald-400/40 bg-emerald-400/10";
  if (s === "refuted") return "text-rose-300 border-rose-400/40 bg-rose-400/10";
  if (s === "abandoned") return "text-slate-300 border-white/10";
  return "text-cyan-300 border-cyan-400/30 bg-cyan-400/10";
}

function SelectedHypothesis({ h, evidence, onDecide, onDelete, onClose }: {
  h: Hypothesis;
  evidence: HypothesisEvidence[];
  onDecide: (s: HypothesisStatus) => void;
  onDelete: () => void;
  onClose: () => void;
}) {
  const [summary, setSummary] = useState<string>("");
  const [weight, setWeight] = useState<number>(0.5);
  const [stance, setStance] = useState<Stance>("for");
  const [busy, setBusy] = useState<boolean>(false);
  const [err, setErr] = useState<string | null>(null);

  async function add() {
    if (!summary.trim()) return;
    setBusy(true);
    setErr(null);
    try {
      await hypothesesAddEvidence({
        hypothesis_id: h.id,
        kind: "observation",
        summary: summary.trim(),
        weight,
        source_kind: null,
        source_id: null,
        stance,
      });
      setSummary("");
    } catch (e) { setErr(String(e)); }
    finally { setBusy(false); }
  }

  return (
    <div className="panel p-3 space-y-2">
      <div className="flex items-center gap-2">
        <div className="flex-1">
          <div className="h-subtitle">{h.title}</div>
          <div className="text-[12px] text-slate-400">{h.statement}</div>
          {h.prediction && <div className="text-[12px] text-cyan-300">Prediction: {h.prediction}</div>}
        </div>
        <span className={`chip ${statusChipClass(h.status)}`}>{h.status}</span>
        <button className="btn" onClick={onClose}>Close</button>
      </div>
      <div className="text-xs text-slate-500">Evidence ({evidence.length})</div>
      <ul className="text-[12px] text-slate-300 space-y-1">
        {evidence.length === 0 && <li className="text-slate-500">No evidence recorded yet.</li>}
        {evidence.map((e) => (
          <li key={e.id} className="flex items-center gap-2">
            <span className="chip text-[10px]">{e.stance}</span>
            <span className="font-mono text-[10px] text-slate-500">w {e.weight.toFixed(2)}</span>
            <span className="flex-1">{e.summary}</span>
          </li>
        ))}
      </ul>
      {h.status === "open" && (
        <div className="flex flex-wrap items-end gap-2">
          <input
            className="input flex-1 min-w-[20rem]"
            placeholder="Add a piece of evidence"
            value={summary}
            onChange={(e) => setSummary(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); void add(); } }}
          />
          <label className="text-[10px] text-slate-500">
            stance
            <select className="select ml-1" value={stance} onChange={(e) => setStance(e.target.value as Stance)}>
              <option value="for">for</option>
              <option value="against">against</option>
            </select>
          </label>
          <label className="text-[10px] text-slate-500">
            weight {weight.toFixed(2)}
            <input
              type="range" min={0} max={1} step={0.05}
              className="ml-2 align-middle"
              value={weight}
              onChange={(e) => setWeight(parseFloat(e.target.value))}
            />
          </label>
          <button className="btn" disabled={busy} onClick={add}>Add</button>
        </div>
      )}
      {err && <div className="text-[12px] text-rose-400">{err}</div>}
      <div className="flex gap-2 pt-2">
        {h.status === "open" && (
          <>
            <button className="btn" onClick={() => onDecide("confirmed")}>Confirm</button>
            <button className="btn" onClick={() => onDecide("refuted")}>Refute</button>
            <button className="btn" onClick={() => onDecide("abandoned")}>Abandon</button>
          </>
        )}
        <button className="btn" onClick={onDelete}>Delete</button>
      </div>
    </div>
  );
}

// ---------------- Archaeologist ----------------

function ArchaeologistTab() {
  const [root, setRoot] = useState<string>(String.raw`D:\NEXUS\app\src-tauri\src`);
  const [index, setIndex] = useState<IndexSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);
  const [query, setQuery] = useState<string>("");
  const [openFile, setOpenFile] = useState<{ path: string; head: string; total_lines: number; size_bytes: number; sha256: string | null; truncated: boolean } | null>(null);

  async function scan() {
    setBusy(true);
    setError(null);
    try {
      const r = await archaeologistIndex({ root });
      setIndex(r);
      setOpenFile(null);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  const filtered = useMemo(() => {
    if (!index) return [];
    if (!query.trim()) return index.files;
    const q = query.toLowerCase();
    return index.files.filter((f) => f.path.toLowerCase().includes(q));
  }, [index, query]);

  async function open(p: string) {
    setError(null);
    try {
      const r = await archaeologistHead(root, p, 200);
      setOpenFile({
        path: p,
        head: r.head,
        total_lines: r.total_lines,
        size_bytes: r.size_bytes,
        sha256: r.sha256,
        truncated: r.truncated,
      });
    } catch (e) { setError(String(e)); }
  }

  return (
    <div className="panel p-5 space-y-4">
      <div>
        <div className="h-title">Codebase Archaeologist</div>
        <p className="mt-1 text-sm text-slate-400">
          Read-only filesystem walk. The archaeologist never
          modifies the tree. Excludes <code>.git</code>,
          <code>node_modules</code>, <code>target</code>,
          <code>dist</code>, and a few other build-output
          directories by default.
        </p>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-[1fr_auto] gap-2">
        <input
          className="input font-mono"
          value={root}
          onChange={(e) => setRoot(e.target.value)}
        />
        <button className="btn-primary" disabled={busy} onClick={scan}>
          {busy ? "Walkingâ€¦" : "Index"}
        </button>
      </div>
      {error && <div className="text-sm text-rose-400">{error}</div>}
      {index && (
        <div className="grid grid-cols-1 md:grid-cols-5 gap-2">
          {Object.entries(index.by_kind).map(([k, v]) => (
            <div key={k} className="rounded border border-white/5 bg-ink-800/40 p-2">
              <div className="text-[10px] text-slate-500 font-mono">{k}</div>
              <div className="text-base text-slate-200">{v}</div>
            </div>
          ))}
        </div>
      )}
      {index && (
        <input
          className="input w-full"
          placeholder="Filter by name (case-insensitive)"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      )}
      {index && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
          <ul className="divide-y divide-white/5 border border-white/5 rounded-lg max-h-[28rem] overflow-y-auto">
            {filtered.slice(0, 500).map((f) => (
              <li key={f.path} className="p-2 flex items-center gap-2">
                <span className="chip text-[10px]">{f.kind}</span>
                <button
                  className="text-left text-[12px] font-mono text-slate-200 flex-1 truncate hover:underline"
                  onClick={() => open(f.path)}
                >{f.path}</button>
                <span className="text-[10px] text-slate-500 font-mono">{f.size_bytes}b</span>
              </li>
            ))}
            {filtered.length > 500 && (
              <li className="p-2 text-[12px] text-slate-500">â€¦and {filtered.length - 500} more</li>
            )}
          </ul>
          <div className="border border-white/5 rounded-lg p-2 max-h-[28rem] overflow-y-auto">
            {openFile ? (
              <>
                <div className="text-[11px] text-slate-500 font-mono">
                  {openFile.path} Â· {openFile.size_bytes}b Â· {openFile.total_lines} lines
                  {openFile.truncated && " Â· truncated"}
                </div>
                {openFile.sha256 && (
                  <div className="text-[10px] text-slate-500 font-mono break-all">
                    sha256 {openFile.sha256}
                  </div>
                )}
                <pre className="mt-2 whitespace-pre-wrap text-[12px] font-mono text-slate-200">{openFile.head}</pre>
              </>
            ) : (
              <p className="text-[12px] text-slate-500">Click a file to view its first 200 lines.</p>
            )}
          </div>
        </div>
      )}
    </div>
  );
}


// ---------------- Serendipity ----------------

function SerendipityTab() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [, setActive] = useState<Project | null>(null);
  const [projectId, setProjectId] = useState<string | null>(null);
  const [report, setReport] = useState<SerendipityReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);

  useEffect(() => {
    (async () => {
      try {
        const [ps, ap] = await Promise.all([projectsList(false), getActiveProject()]);
        setProjects(ps);
        setActive(ap ?? null);
        const initial = ap?.id ?? ps[0]?.id ?? null;
        setProjectId(initial);
        if (initial) await runReport(initial);
      } catch (e) { setError(String(e)); }
    })();
  }, []);

  async function runReport(pid: string) {
    setBusy(true);
    setError(null);
    try {
      const r = await serendipityReport({ project_id: pid });
      setReport(r);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  return (
    <div className="panel p-5 space-y-4">
      <div className="flex items-center gap-3">
        <div className="flex-1">
          <div className="h-title">Serendipity</div>
          <p className="mt-1 text-sm text-slate-400">
            Things you have not looked at in a while, plus
            cross-project notes that share trigrams. Read-only.
          </p>
        </div>
        <select
          className="select"
          value={projectId ?? ""}
          onChange={(e) => {
            const v = e.target.value || null;
            setProjectId(v);
            if (v) void runReport(v);
          }}
        >
          {projects.length === 0 && <option value="">No projects</option>}
          {projects.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
        </select>
        <button className="btn" disabled={busy || !projectId} onClick={() => projectId && runReport(projectId)}>
          {busy ? "Scanningâ€¦" : "Rescan"}
        </button>
      </div>
      {error && <div className="text-sm text-rose-400">{error}</div>}
      {report && (
        <ul className="divide-y divide-white/5 border border-white/5 rounded-lg">
          {report.items.length === 0 && (
            <li className="p-3 text-sm text-slate-500">Nothing dormant. Try lowering the threshold.</li>
          )}
          {report.items.map((it) => (
            <li key={`${it.ref_kind}-${it.id}`} className="p-3 flex items-center gap-3">
              <span className="chip">{it.kind.replace(/_/g, " ")}</span>
              <div className="flex-1 min-w-0">
                <div className="text-sm text-slate-200 truncate">{it.title}</div>
                <div className="text-[12px] text-slate-400">
                  {it.ref_kind} {it.id} Â· dormant {it.days_dormant.toFixed(1)} day(s)
                </div>
              </div>
              <div className="text-right">
                <div className="text-[11px] text-slate-500 font-mono">surprise</div>
                <div className="text-sm text-slate-200">{it.score.toFixed(2)}</div>
              </div>
            </li>
          ))}
        </ul>
      )}
      {report && report.cross_project.length > 0 && (
        <div>
          <div className="h-subtitle">Cross-project</div>
          <ul className="mt-2 space-y-2">
            {report.cross_project.map((h, i) => (
              <li key={i} className="rounded border border-white/5 bg-ink-800/40 p-2 text-[12px]">
                <div className="text-slate-300">
                  <span className="font-mono">{h.left.project_id}</span> â†” <span className="font-mono">{h.right.project_id}</span>
                  {" Â· "}
                  <span className="text-slate-500">{h.shared_count} shared trigram(s)</span>
                </div>
                <div className="text-slate-400 mt-1">
                  &quot;{h.left.title}&quot; â†” &quot;{h.right.title}&quot;
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

// ---------------- Impact ----------------

function ImpactTab() {
  const [action, setAction] = useState<ImpactAction>("delete_note");
  const [targetId, setTargetId] = useState<string>("");
  const [report, setReport] = useState<ImpactReportT | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);

  async function run() {
    if (!targetId.trim()) { setError("Target id is required."); return; }
    setBusy(true);
    setError(null);
    try {
      const req: ImpactCommandRequest = { action, target_id: targetId.trim() };
      const r = await impactSimulate(req);
      setReport(r);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  return (
    <div className="panel p-5 space-y-4">
      <div>
        <div className="h-title">Impact Simulator</div>
        <p className="mt-1 text-sm text-slate-400">
          Dry-run a destructive action. The simulator never
          writes to any table; it walks the existing rows and
          reports what would be affected.
        </p>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-[14rem_1fr_auto] gap-2">
        <select
          className="select"
          value={action}
          onChange={(e) => setAction(e.target.value as ImpactAction)}
        >
          <option value="delete_note">delete_note</option>
          <option value="delete_memory">delete_memory</option>
          <option value="delete_task">delete_task</option>
          <option value="delete_checkpoint">delete_checkpoint</option>
          <option value="delete_hypothesis">delete_hypothesis</option>
          <option value="delete_project">delete_project</option>
          <option value="complete_task">complete_task</option>
        </select>
        <input
          className="input font-mono"
          placeholder="target id"
          value={targetId}
          onChange={(e) => setTargetId(e.target.value)}
        />
        <button className="btn-primary" disabled={busy} onClick={run}>
          {busy ? "Simulatingâ€¦" : "Simulate"}
        </button>
      </div>
      {error && <div className="text-sm text-rose-400">{error}</div>}
      {report && (
        <div className="space-y-3">
          <div className="flex items-center gap-3">
            <span className={`chip ${report.destructive ? "text-rose-300 border-rose-400/40 bg-rose-400/10" : "text-emerald-300 border-emerald-400/40 bg-emerald-400/10"}`}>
              {report.destructive ? "destructive" : "non-destructive"}
            </span>
            <div className="text-[12px] text-slate-400 font-mono">
              {report.action} on {report.target_id}
            </div>
          </div>
          {report.warnings.length > 0 && (
            <ul className="text-[12px] text-amber2-300 space-y-1">
              {report.warnings.map((w, i) => <li key={i}>âš  {w}</li>)}
            </ul>
          )}
          <div>
            <div className="h-subtitle">Counts</div>
            <ul className="mt-2 grid grid-cols-1 md:grid-cols-3 gap-2">
              {report.counts.map((c, i) => (
                <li key={i} className="rounded border border-white/5 bg-ink-800/40 p-2">
                  <div className="text-[10px] text-slate-500 font-mono">{c.table}</div>
                  <div className="text-base text-slate-200">{c.count}</div>
                </li>
              ))}
            </ul>
          </div>
          {report.affected.length > 0 && (
            <div>
              <div className="h-subtitle">Affected (sample)</div>
              <ul className="mt-2 divide-y divide-white/5 border border-white/5 rounded-lg">
                {report.affected.map((a, i) => (
                  <li key={i} className="p-2 text-[12px] flex items-center gap-2">
                    <span className="chip text-[10px]">{a.table}</span>
                    <span className="font-mono text-[10px] text-slate-500">{a.id}</span>
                    <span className="text-slate-200 truncate flex-1">{a.title}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </div>
  );
}




// ---------------- Tribunal ----------------

function TribunalTab() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectId, setProjectId] = useState<string | null>(null);
  const [question, setQuestion] = useState<string>("");
  const [report, setReport] = useState<TribunalReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);

  useEffect(() => {
    (async () => {
      try {
        const list = await projectsList();
        setProjects(list);
        const active = await getActiveProject();
        const id = active?.id ?? list[0]?.id ?? null;
        setProjectId(id);
      } catch (e: unknown) {
        setError(String(e));
      }
    })();
  }, []);

  async function run() {
    if (!question.trim()) { setError("Question is required."); return; }
    setBusy(true);
    setError(null);
    try {
      const req: TribunalRequest = { question: question.trim(), project_id: projectId, repo_root: null, limit_per_source: 8 };
      const r = await evidenceTribunalRun(req);
      setReport(r);
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-3">
      <div className="panel p-3 space-y-2">
        <div className="flex gap-2">
          <select
            className="input flex-1"
            value={projectId ?? ""}
            onChange={(e) => setProjectId(e.target.value || null)}
          >
            <option value="">All projects</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>{p.name}</option>
            ))}
          </select>
        </div>
        <textarea
          className="input w-full"
          rows={3}
          placeholder="Ask the tribunal â€” it pulls evidence from notes, memories, timeline events, and hypotheses."
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
        />
        <div className="flex gap-2 items-center">
          <button className="btn-primary" onClick={run} disabled={busy}>
            {busy ? "Runningâ€¦" : "Run tribunal"}
          </button>
          {error && <span className="text-red-400 text-[12px]">{error}</span>}
        </div>
      </div>
      {report && (
        <div className="panel p-3 space-y-3">
          <div className="flex flex-wrap gap-2 text-[12px]">
            <span className="chip">supports: {report.counts.supports}</span>
            <span className="chip">contradicts: {report.counts.contradicts}</span>
            <span className="chip">partial: {report.counts.partial}</span>
            <span className="chip">unjudged: {report.counts.unjudged}</span>
          </div>
          {report.notes.length > 0 && (
            <ul className="text-[12px] text-slate-400 list-disc pl-5 space-y-1">
              {report.notes.map((n, i) => (<li key={i}>{n}</li>))}
            </ul>
          )}
          <div className="space-y-2">
            {report.cards.map((c: EvidenceCard, i: number) => (
              <div key={i} className="rounded-lg border border-white/5 p-2 space-y-1">
                <div className="flex items-center gap-2 text-[12px]">
                  <span className="chip">{c.source_kind}</span>
                  <span className={`chip ${c.stance === "supports" ? "bg-emerald-900/30" : c.stance === "contradicts" ? "bg-rose-900/30" : c.stance === "partial" ? "bg-amber-900/30" : ""}`}>
                    {c.stance}
                  </span>
                  <span className="text-slate-500 font-mono text-[10px]">conf {c.confidence.toFixed(2)}</span>
                </div>
                <div className="text-slate-200 text-[12px] whitespace-pre-wrap">{c.snippet}</div>
                {c.reasons.length > 0 && (
                  <ul className="text-[11px] text-slate-500 list-disc pl-5">
                    {c.reasons.map((r, j) => (<li key={j}>{r}</li>))}
                  </ul>
                )}
                {c.href && (
                  <div className="text-[10px] text-slate-500 font-mono">{c.href}</div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

