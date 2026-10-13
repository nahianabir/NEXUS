// Experiments. Phase H.
//
// One sidebar list of experiments; right pane shows the active
// experiment's runs. Each run has a hyperparams/metrics JSON editor
// (we use a plain textarea and parse it; the user is expected to
// enter valid JSON). The chart on the run view is a small SVG line
// chart of selected metric values.

import { useEffect, useMemo, useState } from "react";
import {
  type Experiment,
  type ExperimentRun,
  type MetricPoint,
  experimentCreate,
  experimentDelete,
  experimentList,
  experimentMetricHistory,
  experimentRunCreate,
  experimentRunDelete,
  experimentRunList,
  experimentRunUpdate,
  experimentUpdate,
} from "../lib/experiments";
import { usePromptDialog } from "../hooks/usePromptDialog";

const STATUSES = ["active", "paused", "done", "archived"] as const;
const RUN_STATUSES = ["running", "completed", "failed", "cancelled"] as const;

export function ExperimentsPage() {
  const { prompt: promptModal, confirm: confirmModal } = usePromptDialog();
  const [experiments, setExperiments] = useState<Experiment[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [runs, setRuns] = useState<ExperimentRun[]>([]);
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const [metricHistory, setMetricHistory] = useState<MetricPoint[]>([]);
  const [selectedMetric, setSelectedMetric] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async (id: string | null) => {
    try {
      setExperiments(await experimentList(null, null, 50));
      if (id) {
        const r = await experimentRunList(id);
        setRuns(r);
        if (r.length > 0 && !r.find((x) => x.id === activeRunId)) setActiveRunId(r[0].id);
        const hist = await experimentMetricHistory(id);
        setMetricHistory(hist);
        if (hist.length > 0 && !selectedMetric) setSelectedMetric(hist[0].metric);
      } else {
        setRuns([]);
        setMetricHistory([]);
      }
    } catch (e) { setError(String(e)); }
  };

  useEffect(() => { void refresh(null); }, []);

  useEffect(() => { void refresh(activeId); }, [activeId]);

  const active = useMemo(() => experiments.find((e) => e.id === activeId) ?? null, [experiments, activeId]);
  const activeRun = useMemo(() => runs.find((r) => r.id === activeRunId) ?? null, [runs, activeRunId]);

  const onNewExperiment = async () => {
    const name = await promptModal({
      title: "New experiment",
      label: "Name",
      defaultValue: "Untitled experiment",
      submitLabel: "Create experiment",
      validate: (v) => (v.trim() ? "" : "Name is required"),
    });
    if (!name) return;
    const created = await experimentCreate({
      id: "", project_id: null, name, description: null, status: "active", tags: [], created_at: 0, updated_at: 0,
    });
    await refresh(null);
    setActiveId(created.id);
  };

  const onUpdateExperiment = async (patch: Partial<Experiment>) => {
    if (!active) return;
    const updated = await experimentUpdate({ ...active, ...patch });
    await refresh(updated.id);
  };

  const onDeleteExperiment = async (id: string) => {
    const ok = await confirmModal({
      title: "Delete experiment",
      description: "Delete this experiment and all of its runs? This cannot be undone.",
      submitLabel: "Delete experiment",
      destructive: true,
    });
    if (!ok) return;
    await experimentDelete(id);
    setActiveId(null);
    setActiveRunId(null);
    await refresh(null);
  };

  const onNewRun = async () => {
    if (!active) return;
    const name = await promptModal({
      title: "New run",
      label: "Name",
      defaultValue: `run ${runs.length + 1}`,
      submitLabel: "Create run",
      validate: (v) => (v.trim() ? "" : "Name is required"),
    });
    if (!name) return;
    const run = await experimentRunCreate({
      id: "",
      experiment_id: active.id,
      name,
      status: "running",
      hyperparams: {},
      metrics: {},
      code_revision: null,
      code_path: null,
      hardware: null,
      notes: null,
      started_at: Date.now(),
      ended_at: null,
      created_at: 0,
      updated_at: 0,
    });
    await refresh(active.id);
    setActiveRunId(run.id);
  };

  const onSaveRun = async (patch: Partial<ExperimentRun>) => {
    if (!activeRun || !active) return;
    const updated = await experimentRunUpdate({ ...activeRun, ...patch });
    await refresh(active.id);
    setActiveRunId(updated.id);
  };

  const onDeleteRun = async (id: string) => {
    const ok = await confirmModal({
      title: "Delete run",
      description: "Delete this run? Metric history will be removed.",
      submitLabel: "Delete run",
      destructive: true,
    });
    if (!ok) return;
    await experimentRunDelete(id);
    if (activeRunId === id) setActiveRunId(null);
    if (active) await refresh(active.id);
  };

  return (
    <div className="grid h-full grid-cols-12 gap-4 p-4 min-h-0">
      <aside className="col-span-3 panel p-3 flex flex-col min-h-0">
        <div className="flex items-center justify-between">
          <div className="h-title">Experiments</div>
          <button className="btn btn-primary" onClick={onNewExperiment}>New</button>
        </div>
        <div className="mt-3 space-y-1 overflow-y-auto flex-1 min-h-0">
          {experiments.length === 0 && <div className="text-xs text-slate-500 px-2">No experiments yet.</div>}
          {experiments.map((e) => (
            <button
              key={e.id}
              className={`w-full text-left rounded-lg p-2 transition ${activeId === e.id ? "bg-cyan2-500/10 ring-1 ring-cyan2-500/30" : "hover:bg-white/5"}`}
              onClick={() => setActiveId(e.id)}
            >
              <div className="text-sm text-slate-100 truncate">{e.name}</div>
              <div className="text-[11px] text-slate-500 font-mono truncate">{e.description || "(no description)"}</div>
              <div className="mt-1 flex gap-1 text-[10px]">
                <span className="chip">{e.status}</span>
                <span className="kbd" onClick={(ev) => { ev.stopPropagation(); void onDeleteExperiment(e.id); }}>delete</span>
              </div>
            </button>
          ))}
        </div>
      </aside>

      <section className="col-span-9 panel p-4 flex flex-col min-h-0">
        {!active ? (
          <div className="text-sm text-slate-500 text-center py-12">Select or create an experiment.</div>
        ) : (
          <>
            <div className="flex items-center gap-2">
              <div className="h-title flex-1 truncate">{active.name}</div>
              <select className="input" value={active.status} onChange={(e) => onUpdateExperiment({ status: e.target.value })}>
                {STATUSES.map((s) => <option key={s} value={s}>{s}</option>)}
              </select>
            </div>
            <div className="mt-2 grid grid-cols-1 md:grid-cols-2 gap-2">
              <input className="input" value={active.name} onChange={(e) => onUpdateExperiment({ name: e.target.value })} />
              <input className="input" placeholder="Tags (comma separated)" defaultValue={active.tags.join(", ")} onBlur={(e) => onUpdateExperiment({ tags: e.target.value.split(",").map((s) => s.trim()).filter(Boolean) })} />
            </div>
            <textarea className="input mt-2 min-h-[3rem]" placeholder="Description" value={active.description ?? ""} onChange={(e) => onUpdateExperiment({ description: e.target.value })} />

            <div className="mt-4 flex items-center gap-2">
              <div className="h-sub">Runs</div>
              <button className="btn btn-primary" onClick={onNewRun}>New run</button>
              {metricHistory.length > 0 && (
                <select className="input ml-auto" value={selectedMetric ?? ""} onChange={(e) => setSelectedMetric(e.target.value)}>
                  {[...new Set(metricHistory.map((p) => p.metric))].map((m) => <option key={m} value={m}>{m}</option>)}
                </select>
              )}
            </div>
            <div className="mt-2 flex gap-2 min-h-0">
              <div className="w-64 overflow-y-auto pr-1 space-y-1">
                {runs.length === 0 && <div className="text-xs text-slate-500">No runs yet.</div>}
                {runs.map((r) => (
                  <button
                    key={r.id}
                    className={`w-full text-left rounded p-2 text-[12px] border ${activeRunId === r.id ? "bg-cyan2-500/10 border-cyan2-500/30" : "border-white/5 hover:bg-white/5"}`}
                    onClick={() => setActiveRunId(r.id)}
                  >
                    <div className="flex items-center gap-1">
                      <span className="flex-1 truncate text-slate-200">{r.name}</span>
                      <span className="chip">{r.status}</span>
                    </div>
                    <div className="text-[10px] text-slate-500 font-mono">{new Date(r.created_at).toLocaleString()}</div>
                  </button>
                ))}
              </div>
              <div className="flex-1 min-w-0">
                {!activeRun ? <div className="text-xs text-slate-500">Select a run to edit.</div> : (
                  <RunEditor
                    run={activeRun}
                    onSave={onSaveRun}
                    onDelete={() => onDeleteRun(activeRun.id)}
                    metricHistory={metricHistory.filter((p) => p.run_id === activeRun.id && (selectedMetric ? p.metric === selectedMetric : true))}
                    selectedMetric={selectedMetric}
                  />
                )}
              </div>
            </div>
            {error && <div className="mt-2 text-[11px] text-rose2-400 font-mono">{error}</div>}
          </>
        )}
      </section>
    </div>
  );
}

function RunEditor({ run, onSave, onDelete, metricHistory, selectedMetric }: { run: ExperimentRun; onSave: (p: Partial<ExperimentRun>) => void; onDelete: () => void; metricHistory: MetricPoint[]; selectedMetric: string | null; }) {
  const [hp, setHp] = useState(JSON.stringify(run.hyperparams ?? {}, null, 2));
  const [m, setM] = useState(JSON.stringify(run.metrics ?? {}, null, 2));
  const [hpErr, setHpErr] = useState<string | null>(null);
  const [mErr, setMErr] = useState<string | null>(null);
  useEffect(() => { setHp(JSON.stringify(run.hyperparams ?? {}, null, 2)); setM(JSON.stringify(run.metrics ?? {}, null, 2)); }, [run.id]);
  const commit = () => {
    let hpv: unknown = {};
    let mv: unknown = {};
    try { hpv = JSON.parse(hp); setHpErr(null); } catch (e) { setHpErr(String(e)); return; }
    try { mv = JSON.parse(m); setMErr(null); } catch (e) { setMErr(String(e)); return; }
    onSave({ hyperparams: hpv as never, metrics: mv as never });
  };
  return (
    <div className="panel-soft p-3 space-y-2 text-[12px]">
      <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
        <input className="input" placeholder="Run name" defaultValue={run.name} onBlur={(e) => onSave({ name: e.target.value })} />
        <select className="input" value={run.status} onChange={(e) => onSave({ status: e.target.value })}>
          {RUN_STATUSES.map((s) => <option key={s} value={s}>{s}</option>)}
        </select>
        <input className="input" placeholder="Hardware (GPU, CPU)" defaultValue={run.hardware ?? ""} onBlur={(e) => onSave({ hardware: e.target.value })} />
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        <input className="input" placeholder="Code revision (git sha)" defaultValue={run.code_revision ?? ""} onBlur={(e) => onSave({ code_revision: e.target.value })} />
        <input className="input" placeholder="Code path" defaultValue={run.code_path ?? ""} onBlur={(e) => onSave({ code_path: e.target.value })} />
      </div>
      <textarea className="input" placeholder="Notes" defaultValue={run.notes ?? ""} onBlur={(e) => onSave({ notes: e.target.value })} />
      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
        <div>
          <div className="text-[10px] text-slate-500 uppercase tracking-wide">hyperparams (JSON)</div>
          <textarea className="input font-mono text-[11px] min-h-[8rem]" value={hp} onChange={(e) => setHp(e.target.value)} onBlur={commit} />
          {hpErr && <div className="text-[10px] text-rose2-400 font-mono">{hpErr}</div>}
        </div>
        <div>
          <div className="text-[10px] text-slate-500 uppercase tracking-wide">metrics (JSON)</div>
          <textarea className="input font-mono text-[11px] min-h-[8rem]" value={m} onChange={(e) => setM(e.target.value)} onBlur={commit} />
          {mErr && <div className="text-[10px] text-rose2-400 font-mono">{mErr}</div>}
        </div>
      </div>
      {metricHistory.length > 0 && <MiniChart points={metricHistory} label={selectedMetric ?? ""} />}
      <div className="flex items-center gap-2">
        <button className="btn btn-primary" onClick={commit}>Save run</button>
        <span className="kbd ml-auto" onClick={onDelete}>delete run</span>
      </div>
    </div>
  );
}

function MiniChart({ points, label }: { points: MetricPoint[]; label: string }) {
  if (points.length === 0) return null;
  // Order by run name for stable x axis. (We don't have run timestamps
  // in MetricPoint; name-based order is good enough.)
  const sorted = [...points].sort((a, b) => a.run_name.localeCompare(b.run_name));
  const w = 480;
  const h = 120;
  const pad = 24;
  const values = sorted.map((p) => p.value);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const xStep = (w - pad * 2) / Math.max(1, sorted.length - 1);
  const path = sorted.map((p, i) => {
    const x = pad + i * xStep;
    const y = h - pad - ((p.value - min) / range) * (h - pad * 2);
    return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(" ");
  return (
    <div className="panel-soft p-2">
      <div className="text-[10px] text-slate-500 uppercase tracking-wide">metric: {label}</div>
      <svg viewBox={`0 0 ${w} ${h}`} className="w-full h-32">
        <path d={path} fill="none" stroke="#67e0ff" strokeWidth="1.5" />
        {sorted.map((p, i) => {
          const x = pad + i * xStep;
          const y = h - pad - ((p.value - min) / range) * (h - pad * 2);
          return <circle key={i} cx={x} cy={y} r={2.5} fill="#67e0ff"><title>{p.run_name}: {p.value}</title></circle>;
        })}
        <text x={pad} y={h - 4} fontSize="10" fill="#94a3b8">{sorted[0].run_name}</text>
        <text x={w - pad} y={h - 4} fontSize="10" textAnchor="end" fill="#94a3b8">{sorted[sorted.length - 1].run_name}</text>
        <text x={4} y={12} fontSize="10" fill="#94a3b8">{max}</text>
        <text x={4} y={h - pad} fontSize="10" fill="#94a3b8">{min}</text>
      </svg>
    </div>
  );
}