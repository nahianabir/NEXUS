// Project Time Machine — Phase S.1.
//
// This page reads `timeline_events` and shows a chronological,
// provenance-tagged view of a project's history. It is
// *read-only* against the source tables; the only writes are the
// idempotent inserts triggered by "Build timeline" and the
// user-driven "Add a milestone" action.

import { useEffect, useMemo, useState } from "react";
import {
  type Comparison,
  type IngestReport,
  type Reconstruction,
  type TimelineEvent,
  timelineCompare,
  timelineCount,
  timelineDecisions,
  timelineInsertManual,
  timelineIngestProject,
  timelineList,
  timelineReconstruct,
  timelineSuperseded,
} from "../lib/timeline";
import { type Project, projectsList, getActiveProject } from "../lib/projects";

type FilterState = {
  sourceKind: string;
  eventType: string;
  sinceMs: string;
  untilMs: string;
};

const DEFAULT_FILTER: FilterState = {
  sourceKind: "",
  eventType: "",
  sinceMs: "",
  untilMs: "",
};

const SOURCE_KINDS = [
  "audit",
  "task",
  "note",
  "memory",
  "experiment",
  "experiment_run",
  "kg_relationship",
  "agent_invocation",
  "research_paper",
  "research_report",
  "manual",
];

const EVENT_TYPES = [
  "created",
  "updated",
  "status_changed",
  "completed",
  "approved",
  "superseded",
  "archived",
  "opened",
  "invoked",
  "finished",
  "note",
  "manual",
];

function formatMs(ms: number): string {
  const d = new Date(ms);
  if (Number.isNaN(d.getTime())) return String(ms);
  return d.toLocaleString();
}

export function TimelinePage() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [activeProject, setActiveProject] = useState<Project | null>(null);
  const [projectId, setProjectId] = useState<string | null>(null);
  const [filter, setFilter] = useState<FilterState>(DEFAULT_FILTER);
  const [events, setEvents] = useState<TimelineEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<boolean>(false);
  const [ingestReport, setIngestReport] = useState<IngestReport | null>(null);
  const [reconstruction, setReconstruction] = useState<Reconstruction | null>(null);
  const [comparison, setComparison] = useState<Comparison | null>(null);
  const [supersededCount, setSupersededCount] = useState<{ memories: number; relationships: number }>({
    memories: 0,
    relationships: 0,
  });
  const [decisions, setDecisions] = useState<TimelineEvent[]>([]);
  const [milestoneSummary, setMilestoneSummary] = useState<string>("");
  const [milestoneAt, setMilestoneAt] = useState<string>("");
  const [count, setCount] = useState<number>(0);

  useEffect(() => {
    (async () => {
      try {
        const [ps, ap] = await Promise.all([projectsList(false), getActiveProject()]);
        setProjects(ps);
        setActiveProject(ap ?? null);
        const initial = ap?.id ?? ps[0]?.id ?? null;
        setProjectId(initial);
      } catch (e) {
        setError(String(e));
      }
    })();
  }, []);

  useEffect(() => {
    if (!projectId) {
      setEvents([]);
      setIngestReport(null);
      setReconstruction(null);
      setComparison(null);
      setSupersededCount({ memories: 0, relationships: 0 });
      setDecisions([]);
      setCount(0);
      return;
    }
    void refreshAll(projectId);
  }, [projectId]);

  async function refreshAll(pid: string) {
    setError(null);
    try {
      const sinceMs = filter.sinceMs ? Number(filter.sinceMs) : null;
      const untilMs = filter.untilMs ? Number(filter.untilMs) : null;
      const [evs, cn, dec, sup] = await Promise.all([
        timelineList({
          project_id: pid,
          source_kind: filter.sourceKind || null,
          event_type: filter.eventType || null,
          since_ms: sinceMs,
          until_ms: untilMs,
          limit: 200,
        }),
        timelineCount(pid),
        timelineDecisions(pid),
        timelineSuperseded(pid),
      ]);
      setEvents(evs);
      setCount(cn);
      setDecisions(dec);
      setSupersededCount({ memories: sup.memories.length, relationships: sup.kg_relationships.length });
    } catch (e) {
      setError(String(e));
    }
  }

  async function buildTimeline() {
    if (!projectId) return;
    setBusy(true);
    setError(null);
    try {
      const r = await timelineIngestProject(projectId);
      setIngestReport(r);
      await refreshAll(projectId);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function reconstruct() {
    if (!projectId) return;
    const at = filter.untilMs ? Number(filter.untilMs) : Date.now();
    setError(null);
    try {
      const r = await timelineReconstruct(projectId, at);
      setReconstruction(r);
    } catch (e) {
      setError(String(e));
    }
  }

  async function compare() {
    if (!projectId) return;
    const to = filter.untilMs ? Number(filter.untilMs) : Date.now();
    const from = filter.sinceMs ? Number(filter.sinceMs) : Math.max(0, to - 7 * 24 * 3600 * 1000);
    setError(null);
    try {
      const c = await timelineCompare(projectId, from, to);
      setComparison(c);
    } catch (e) {
      setError(String(e));
    }
  }

  async function addMilestone() {
    if (!projectId || !milestoneSummary.trim()) return;
    const at = milestoneAt ? Number(milestoneAt) : null;
    setBusy(true);
    setError(null);
    try {
      await timelineInsertManual({
        project_id: projectId,
        summary: milestoneSummary.trim(),
        event_at: at,
        detail: { source: "manual" },
      });
      setMilestoneSummary("");
      setMilestoneAt("");
      await refreshAll(projectId);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const grouped = useMemo(() => {
    const map = new Map<string, TimelineEvent[]>();
    for (const e of events) {
      const k = SOURCE_KINDS.includes(e.source_kind) ? e.source_kind : "other";
      const arr = map.get(k) ?? [];
      arr.push(e);
      map.set(k, arr);
    }
    return Array.from(map.entries()).sort(([a], [b]) => a.localeCompare(b));
  }, [events]);

  return (
    <div className="p-6 max-w-7xl space-y-6">
      <div className="panel p-5">
        <div className="flex items-start gap-3">
          <div className="flex-1">
            <div className="h-title">Project Time Machine</div>
            <p className="mt-1 text-sm text-slate-400">
              Replayable, provenance-tagged history of one project. The timeline is
              built from real records in tasks, notes, memories, experiments, the
              knowledge graph, agent invocations, and the audit log. It never
              modifies those records.
            </p>
          </div>
          <div className="text-right text-[11px] text-slate-500">
            {activeProject ? (
              <>
                <div>Active project</div>
                <div className="text-slate-300">{activeProject.name}</div>
              </>
            ) : (
              <div>No active project</div>
            )}
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
                <option key={p.id} value={p.id}>
                  {p.name}
                  {p.archived ? " (archived)" : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="text-xs text-slate-400">
            Source kind
            <select
              className="select w-full mt-1"
              value={filter.sourceKind}
              onChange={(e) => setFilter((f) => ({ ...f, sourceKind: e.target.value }))}
            >
              <option value="">All</option>
              {SOURCE_KINDS.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>
          <label className="text-xs text-slate-400">
            Event type
            <select
              className="select w-full mt-1"
              value={filter.eventType}
              onChange={(e) => setFilter((f) => ({ ...f, eventType: e.target.value }))}
            >
              <option value="">All</option>
              {EVENT_TYPES.map((k) => (
                <option key={k} value={k}>
                  {k}
                </option>
              ))}
            </select>
          </label>
          <label className="text-xs text-slate-400">
            Since (epoch ms)
            <input
              className="input w-full mt-1 font-mono"
              value={filter.sinceMs}
              placeholder="0"
              onChange={(e) => setFilter((f) => ({ ...f, sinceMs: e.target.value }))}
            />
          </label>
          <label className="text-xs text-slate-400">
            Until (epoch ms)
            <input
              className="input w-full mt-1 font-mono"
              value={filter.untilMs}
              placeholder={String(Date.now())}
              onChange={(e) => setFilter((f) => ({ ...f, untilMs: e.target.value }))}
            />
          </label>
          <div className="flex items-end gap-2">
            <button
              className="btn-primary"
              disabled={!projectId || busy}
              onClick={() => void buildTimeline()}
            >
              {busy ? "Building…" : "Build timeline"}
            </button>
            <button className="btn" disabled={!projectId} onClick={() => void refreshAll(projectId!)}>
              Refresh
            </button>
            <button className="btn" disabled={!projectId} onClick={() => void reconstruct()}>
              Reconstruct
            </button>
            <button className="btn" disabled={!projectId} onClick={() => void compare()}>
              Compare
            </button>
          </div>
        </div>

        {error && <div className="mt-3 text-sm text-rose-400">{error}</div>}

        {ingestReport && (
          <div className="mt-3 text-[12px] text-slate-400">
            Last ingest: <span className="font-mono">{ingestReport.run_id.slice(-8)}</span>{" "}
            · examined {ingestReport.examined} · inserted {ingestReport.inserted} · skipped{" "}
            {ingestReport.skipped_existing} (idempotent). Total in timeline:{" "}
            <span className="text-slate-200">{count}</span>.
          </div>
        )}

        <div className="mt-2 text-[11px] text-slate-500">
          Decisions on record: <span className="text-slate-200">{decisions.length}</span> · Superseded
          memories: <span className="text-slate-200">{supersededCount.memories}</span> · Superseded
          graph relationships:{" "}
          <span className="text-slate-200">{supersededCount.relationships}</span>
        </div>
      </div>

      {reconstruction && (
        <div className="panel p-5">
          <div className="h-subtitle">Reconstruction at {formatMs(reconstruction.at_ms)}</div>
          <p className="mt-1 text-[12px] text-slate-400">
            Read-only view of what we know was true at this moment. Items below the
            threshold are reported as "unknown" rather than guessed.
          </p>
          {reconstruction.unknowns.length > 0 && (
            <ul className="mt-2 list-disc pl-5 text-[12px] text-amber-300/80">
              {reconstruction.unknowns.map((u, i) => (
                <li key={i}>{u}</li>
              ))}
            </ul>
          )}
          <div className="mt-4 grid grid-cols-1 md:grid-cols-2 gap-3">
            {reconstruction.sections.map((s) => (
              <div key={s.label} className="rounded-lg border border-white/5 bg-ink-800/40 p-3">
                <div className="flex items-center gap-2">
                  <div className="text-sm text-slate-200">{s.label}</div>
                  <span
                    className={`chip ${
                      s.status === "known" ? "text-emerald-300 border-emerald-500/30" : "text-amber-300 border-amber-500/30"
                    }`}
                  >
                    {s.status}
                  </span>
                  <div className="ml-auto text-[10px] font-mono text-slate-500">{s.count}</div>
                </div>
                {s.note && <div className="mt-1 text-[11px] text-slate-500">{s.note}</div>}
                <ul className="mt-2 space-y-1 text-[12px] text-slate-300 max-h-40 overflow-y-auto">
                  {s.items.length === 0 && <li className="text-slate-500">No items.</li>}
                  {s.items.map((it) => (
                    <li key={it.id} className="flex items-center gap-2">
                      <span className="text-slate-500 font-mono text-[10px]">
                        {formatMs(it.at_ms)}
                      </span>
                      <span className="truncate">{it.label}</span>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        </div>
      )}

      {comparison && (
        <div className="panel p-5">
          <div className="h-subtitle">
            Compare {formatMs(comparison.from_ms)} → {formatMs(comparison.to_ms)}
          </div>
          <p className="mt-1 text-[12px] text-slate-400">
            {comparison.note} {comparison.added.length} event(s) in the window.
          </p>
          <ul className="mt-2 space-y-1 text-[12px] text-slate-300 max-h-40 overflow-y-auto">
            {comparison.added.length === 0 && <li className="text-slate-500">Nothing changed in this window.</li>}
            {comparison.added.map((e) => (
              <li key={e.id} className="flex items-center gap-2">
                <span className="text-slate-500 font-mono text-[10px]">{formatMs(e.event_at)}</span>
                <span className="chip">{e.source_kind}</span>
                <span className="truncate">{e.summary}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="panel p-5">
        <div className="h-subtitle">Events</div>
        {!projectId ? (
          <p className="mt-2 text-sm text-slate-500">Pick a project to begin.</p>
        ) : events.length === 0 ? (
          <p className="mt-2 text-sm text-slate-500">
            No events match the current filter. Click <span className="text-slate-300">Build timeline</span> to
            ingest from the source tables.
          </p>
        ) : (
          <div className="mt-3 space-y-3">
            {grouped.map(([kind, list]) => (
              <div key={kind}>
                <div className="text-[11px] uppercase tracking-wide text-slate-500">{kind}</div>
                <ul className="mt-1 divide-y divide-white/5 border border-white/5 rounded-lg">
                  {list.map((e) => (
                    <li key={e.id} className="p-3 hover:bg-white/5">
                      <div className="flex items-center gap-2">
                        <span className="chip">{e.event_type}</span>
                        <span className="text-[11px] text-slate-500 font-mono">
                          {formatMs(e.event_at)}
                        </span>
                        <span className="ml-auto text-[10px] text-slate-500">
                          imported {formatMs(e.imported_at)}
                        </span>
                      </div>
                      <div className="mt-1 text-sm text-slate-200">{e.summary}</div>
                      {e.source_label && (
                        <div className="mt-0.5 text-[11px] text-slate-500">{e.source_label}</div>
                      )}
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="panel p-5">
        <div className="h-subtitle">Add a milestone</div>
        <p className="mt-1 text-[12px] text-slate-400">
          Hand-written timeline entries are useful for context the system
          cannot infer: a real-world deadline, a meeting, a publication, a
          decision to revisit. They are scoped to this project and never
          replace any source record.
        </p>
        <div className="mt-3 grid grid-cols-1 md:grid-cols-3 gap-3">
          <input
            className="input md:col-span-2"
            placeholder="What happened?"
            value={milestoneSummary}
            onChange={(e) => setMilestoneSummary(e.target.value)}
          />
          <input
            className="input font-mono"
            placeholder="When (epoch ms, optional)"
            value={milestoneAt}
            onChange={(e) => setMilestoneAt(e.target.value)}
          />
        </div>
        <div className="mt-3">
          <button
            className="btn-primary"
            disabled={!projectId || busy || !milestoneSummary.trim()}
            onClick={() => void addMilestone()}
          >
            Add milestone
          </button>
        </div>
      </div>
    </div>
  );
}
