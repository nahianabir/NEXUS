// Knowledge Graph — Phase D.
//
// A real, queryable graph backed by SQLite. The page renders a
// force-directed layout of the neighborhood around the selected
// entity. Use the search box to find an entity, or the "All
// entities" list to browse. Click a node to recenter; right-click an
// edge to see its evidence.

import { useEffect, useRef, useState } from "react";
import {
  type KgEntity,
  type KgNeighborhood,
  type KgRelationship,
  kgEntityTypes,
  kgEvidenceFor,
  kgListEntities,
  kgNeighborhood,
  kgRelationshipTypes,
  kgSearch,
  kgSeedProject,
  kgSetRelationshipStatus,
  kgStats,
  type KgEvidence,
  type KgStats,
} from "../lib/kg";
import { type Project, projectsList } from "../lib/projects";

type Position = { x: number; y: number };

export function KnowledgeGraphPage() {
  const [entities, setEntities] = useState<KgEntity[]>([]);
  const [stats, setStats] = useState<KgStats | null>(null);
  const [center, setCenter] = useState<KgEntity | null>(null);
  const [hood, setHood] = useState<KgNeighborhood | null>(null);
  const [search, setSearch] = useState("");
  const [searchResults, setSearchResults] = useState<KgEntity[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [entityTypes, setEntityTypes] = useState<string[]>([]);
  const [relTypes, setRelTypes] = useState<string[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedRel, setSelectedRel] = useState<{ rel: KgRelationship; evidence: KgEvidence[] } | null>(null);

  useEffect(() => {
    (async () => {
      try {
        setEntityTypes(await kgEntityTypes());
        setRelTypes(await kgRelationshipTypes());
        setStats(await kgStats());
        setEntities(await kgListEntities({ limit: 200 }));
        setProjects(await projectsList(false));
      } catch (e) { setError(String(e)); }
    })();
  }, []);

  useEffect(() => {
    if (!center) { setHood(null); return; }
    (async () => {
      try { setHood(await kgNeighborhood(center.id, 2, 200)); }
      catch (e) { setError(String(e)); }
    })();
  }, [center]);

  useEffect(() => {
    if (!search.trim()) { setSearchResults([]); return; }
    (async () => {
      try { setSearchResults(await kgSearch(search, 30)); }
      catch (e) { setError(String(e)); }
    })();
  }, [search]);

  return (
    <div className="p-6 max-w-7xl space-y-4">
      <div className="panel p-5">
        <div className="flex items-start gap-3 flex-wrap">
          <div className="flex-1">
            <div className="h-title">Knowledge Graph</div>
            <p className="mt-1 text-sm text-slate-400">
              A real, queryable graph of entities and relationships. Every edge has
              evidence pointing back to the source that produced it.
            </p>
            {stats && (
              <div className="mt-2 text-[11px] text-slate-500 font-mono">
                {stats.entities} entities · {stats.relationships} relationships · {stats.evidence} evidence rows
              </div>
            )}
          </div>
          <div className="flex flex-col gap-2 min-w-[18rem]">
            <input className="input" placeholder="Search entities…" value={search} onChange={(e) => setSearch(e.target.value)} />
            {searchResults.length > 0 && (
              <div className="panel-soft p-2 max-h-48 overflow-y-auto">
                {searchResults.map((e) => (
                  <button key={e.id} onClick={() => { setCenter(e); setSearch(""); }} className="w-full text-left p-1.5 rounded hover:bg-white/5 text-sm">
                    <div className="text-slate-200">{e.canonical_name}</div>
                    <div className="text-[10px] font-mono text-slate-500">{e.entity_type} · {e.id.slice(-6)}</div>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
        {error && <div className="mt-2 text-xs text-rose2-400 font-mono">{error}</div>}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-12 gap-4">
        <aside className="md:col-span-3 panel p-3 max-h-[34rem] overflow-y-auto">
          <div className="h-title">Entities</div>
          <div className="text-[11px] text-slate-500 mt-1">{entities.length} total</div>
          <div className="mt-2 space-y-1">
            {entities.map((e) => (
              <button
                key={e.id}
                onClick={() => setCenter(e)}
                className={`w-full text-left rounded-md p-2 ${center?.id === e.id ? "bg-cyan2-500/10 ring-1 ring-cyan2-500/30" : "hover:bg-white/5"}`}
              >
                <div className="flex items-center gap-1.5">
                  <span className="chip">{e.entity_type}</span>
                </div>
                <div className="mt-1 text-sm text-slate-200 truncate">{e.canonical_name}</div>
              </button>
            ))}
          </div>
        </aside>

        <div className="md:col-span-6">
          {hood ? (
            <GraphCanvas
              hood={hood}
              onSelectEntity={setCenter}
              onSelectEdge={async (rel) => {
                const ev = await kgEvidenceFor(rel.id).catch(() => [] as KgEvidence[]);
                setSelectedRel({ rel, evidence: ev });
              }}
              centerId={center?.id ?? null}
            />
          ) : (
            <div className="panel p-6 text-sm text-slate-500">Select an entity on the left to see its neighborhood.</div>
          )}
        </div>

        <div className="md:col-span-3 space-y-3">
          {center ? (
            <div className="panel p-4 space-y-2">
              <div className="flex items-center gap-2">
                <span className="chip">{center.entity_type}</span>
                <span className="chip text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10">center</span>
              </div>
              <div className="text-base text-slate-200 font-medium">{center.canonical_name}</div>
              {center.description && <div className="text-sm text-slate-400">{center.description}</div>}
              <div className="text-[10px] font-mono text-slate-500">
                created {new Date(center.created_at).toLocaleString()}
              </div>
              {center.metadata_json && (() => {
                try { return <pre className="text-[10px] bg-ink-900/50 p-2 rounded overflow-x-auto">{JSON.stringify(JSON.parse(center.metadata_json!), null, 2)}</pre>; }
                catch { return null; }
              })()}
              <div className="text-[10px] text-slate-500 mt-1">
                {hood?.nodes.length ?? 0} nodes in neighborhood · {hood?.edges.length ?? 0} edges
              </div>
            </div>
          ) : (
            <div className="panel p-4 text-sm text-slate-500">No center selected.</div>
          )}
          {selectedRel ? (
            <div className="panel p-4 space-y-2">
              <div className="h-title text-sm">Edge</div>
              <div className="text-xs text-slate-300 font-mono">
                {hood?.nodes.find((n) => n.id === selectedRel.rel.source_entity_id)?.canonical_name}
                {' '}<span className="text-cyan2-400">{selectedRel.rel.relationship_type}</span>{' '}
                {hood?.nodes.find((n) => n.id === selectedRel.rel.target_entity_id)?.canonical_name}
              </div>
              <div className="text-[10px] font-mono text-slate-500">
                status: {selectedRel.rel.verification_status} · weight: {selectedRel.rel.weight}
              </div>
              <div className="text-[10px] text-slate-500">
                {selectedRel.evidence.length} evidence row{selectedRel.evidence.length === 1 ? "" : "s"}
              </div>
              <div className="space-y-1">
                {selectedRel.evidence.map((ev) => (
                  <div key={ev.id} className="text-[11px] border border-white/5 rounded p-1.5">
                    <div className="font-mono text-cyan2-400">{ev.source_kind}{ev.source_id ? `:${ev.source_id}` : ""}{ev.source_location ? ` @${ev.source_location}` : ""}</div>
                    {ev.excerpt && <div className="text-slate-400 italic">"{ev.excerpt}"</div>}
                  </div>
                ))}
              </div>
              <div className="flex flex-wrap gap-1.5 pt-1">
                <button className="btn" onClick={async () => { await kgSetRelationshipStatus(selectedRel.rel.id, "user_confirmed"); }}>Mark confirmed</button>
                <button className="btn text-amber2-400" onClick={async () => { await kgSetRelationshipStatus(selectedRel.rel.id, "disputed"); }}>Mark disputed</button>
              </div>
            </div>
          ) : null}
          <SeedProjectCard
            projects={projects}
            onSeed={async (projectId) => {
              try { setCenter(await kgSeedProject(projectId)); }
              catch (e) { setError(String(e)); }
            }}
          />
          <ReferenceCard entityTypes={entityTypes} relTypes={relTypes} />
        </div>
      </div>
    </div>
  );
}

// ---------------- simple force-directed canvas ----------------

function GraphCanvas({ hood, onSelectEntity, onSelectEdge, centerId }: {
  hood: KgNeighborhood;
  onSelectEntity: (e: KgEntity) => void;
  onSelectEdge: (r: KgRelationship) => void;
  centerId: string | null;
}) {
  const ref = useRef<HTMLCanvasElement | null>(null);
  const [size] = useState({ w: 720, h: 520 });
  const [positions, setPositions] = useState<Record<string, Position>>({});

  useEffect(() => {
    const nodes = hood.nodes;
    const cx = size.w / 2;
    const cy = size.h / 2;
    const r = Math.min(cx, cy) * 0.7;
    const next: Record<string, Position> = {};
    nodes.forEach((n, i) => {
      if (n.id === centerId) { next[n.id] = { x: cx, y: cy }; return; }
      const angle = (i / Math.max(1, nodes.length - 1)) * Math.PI * 2;
      next[n.id] = { x: cx + r * Math.cos(angle), y: cy + r * Math.sin(angle) };
    });
    let prev = next;
    for (let iter = 0; iter < 200; iter++) {
      prev = step(prev, hood, cx, cy);
    }
    setPositions(prev);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hood.center.id, centerId]);

  useEffect(() => {
    const canvas = ref.current; if (!canvas) return;
    const ctx = canvas.getContext("2d"); if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    for (const e of hood.edges) {
      const a = positions[e.source_entity_id]; const b = positions[e.target_entity_id];
      if (!a || !b) continue;
      ctx.strokeStyle = e.verification_status === "disputed" ? "#fb7185"
        : e.verification_status === "user_confirmed" ? "#67e8f9" : "#475569";
      ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y); ctx.stroke();
    }
    for (const n of hood.nodes) {
      const p = positions[n.id]; if (!p) continue;
      const isCenter = n.id === centerId;
      ctx.fillStyle = colorForType(n.entity_type, isCenter);
      ctx.beginPath(); ctx.arc(p.x, p.y, isCenter ? 9 : 5, 0, Math.PI * 2); ctx.fill();
      ctx.fillStyle = "#cbd5e1";
      ctx.font = "11px ui-monospace, SFMono-Regular, monospace";
      ctx.fillText(n.canonical_name, p.x + 8, p.y - 8);
    }
  }, [positions, hood, centerId, size]);

  return (
    <div className="panel p-3">
      <canvas
        ref={ref}
        width={size.w}
        height={size.h}
        className="rounded bg-ink-900/40 w-full"
        onClick={(e) => {
          const rect = (e.currentTarget as HTMLCanvasElement).getBoundingClientRect();
          const x = e.clientX - rect.left; const y = e.clientY - rect.top;
          let best: { id: string; d: number } | null = null;
          for (const n of hood.nodes) {
            const p = positions[n.id]; if (!p) continue;
            const d = Math.hypot(p.x - x, p.y - y);
            if (d < 12 && (!best || d < best.d)) best = { id: n.id, d };
          }
          if (best) {
            const n = hood.nodes.find((x) => x.id === best!.id);
            if (n) onSelectEntity(n);
          }
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          const rect = (e.currentTarget as HTMLCanvasElement).getBoundingClientRect();
          const x = e.clientX - rect.left; const y = e.clientY - rect.top;
          let best: { rel: KgRelationship; d: number } | null = null;
          for (const rel of hood.edges) {
            const a = positions[rel.source_entity_id]; const b = positions[rel.target_entity_id];
            if (!a || !b) continue;
            const mx = (a.x + b.x) / 2; const my = (a.y + b.y) / 2;
            const d = Math.hypot(mx - x, my - y);
            if (d < 18 && (!best || d < best.d)) best = { rel, d };
          }
          if (best) onSelectEdge(best.rel);
        }}
      />
      <div className="text-[10px] text-slate-500 mt-1">click a node to recenter · right-click an edge for evidence</div>
    </div>
  );
}

function step(prev: Record<string, Position>, hood: KgNeighborhood, cx: number, cy: number): Record<string, Position> {
  const next: Record<string, Position> = {};
  const k = 80;
  const rep = 4500;
  for (const n of hood.nodes) {
    let fx = 0, fy = 0;
    for (const e of hood.edges) {
      if (e.source_entity_id === n.id) {
        const b = prev[e.target_entity_id]; if (b) {
          const dx = b.x - prev[n.id].x, dy = b.y - prev[n.id].y;
          const d = Math.hypot(dx, dy) + 0.1;
          fx += (dx / d) * (d - k) * 0.02;
          fy += (dy / d) * (d - k) * 0.02;
        }
      }
      if (e.target_entity_id === n.id) {
        const a = prev[e.source_entity_id]; if (a) {
          const dx = a.x - prev[n.id].x, dy = a.y - prev[n.id].y;
          const d = Math.hypot(dx, dy) + 0.1;
          fx += (dx / d) * (d - k) * 0.02;
          fy += (dy / d) * (d - k) * 0.02;
        }
      }
    }
    for (const m of hood.nodes) {
      if (m.id === n.id) continue;
      const dx = prev[n.id].x - prev[m.id].x, dy = prev[n.id].y - prev[m.id].y;
      const d2 = dx * dx + dy * dy + 0.1;
      fx += (dx / d2) * rep;
      fy += (dy / d2) * rep;
    }
    fx += (cx - prev[n.id].x) * 0.005;
    fy += (cy - prev[n.id].y) * 0.005;
    const damp = 0.6;
    next[n.id] = { x: prev[n.id].x + fx * damp, y: prev[n.id].y + fy * damp };
  }
  return next;
}

function colorForType(t: string, isCenter: boolean): string {
  if (isCenter) return "#22d3ee";
  switch (t) {
    case "project": return "#a78bfa";
    case "paper": return "#fbbf24";
    case "model": return "#34d399";
    case "dataset": return "#f472b6";
    case "decision": return "#f87171";
    case "memory": return "#60a5fa";
    default: return "#94a3b8";
  }
}

function SeedProjectCard({ projects, onSeed }: { projects: Project[]; onSeed: (id: string) => void }) {
  const [projectId, setProjectId] = useState<string>("");
  return (
    <div className="panel p-3 space-y-1.5">
      <div className="h-title text-sm">Seed from a project</div>
      <div className="text-[10px] text-slate-500">Materialise the project as a graph entity.</div>
      <select className="input" value={projectId} onChange={(e) => setProjectId(e.target.value)}>
        <option value="">Pick a project…</option>
        {projects.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
      </select>
      <button className="btn" disabled={!projectId} onClick={() => onSeed(projectId)}>Seed entity</button>
    </div>
  );
}

function ReferenceCard({ entityTypes, relTypes }: { entityTypes: string[]; relTypes: string[] }) {
  return (
    <div className="panel p-3 text-[10px] text-slate-500 space-y-1">
      <div className="text-slate-300 text-sm">Reference</div>
      <div>entity types: {entityTypes.length}</div>
      <div>relationship types: {relTypes.length}</div>
      <div className="font-mono">{relTypes.slice(0, 6).join(", ")}…</div>
    </div>
  );
}
