// Memory Center — Phase 5B native long-term memory v2.
//
// Tabs: Approved (the canonical long-term memory), Candidates
// (auto-extracted, pending review), Timeline (last 30 days of memory
// changes), Sessions (end-of-conversation summaries).

import { useEffect, useMemo, useState } from "react";
import { type Project, projectsList, getActiveProject } from "../lib/projects";
import {
  type Candidate,
  type Evidence,
  type MemoryCategory,
  type MemoryV2,
  memoriesV2Categories,
  memoriesV2Create,
  memoriesV2Delete,
  memoriesV2List,
  memoriesV2Search,
  memoriesV2Supersede,
  memoriesV2Update,
  memoryCandidatesApprove,
  memoryCandidatesList,
  memoryCandidatesPropose,
  memoryCandidatesReject,
  memoryEvidenceAdd,
  memoryEvidenceList,
  memorySessionsList,
  type Session,
} from "../lib/memory_v2";
import { onMemorySaved } from "../lib/chat";
import { usePromptDialog } from "../hooks/usePromptDialog";

type Tab = "approved" | "candidates" | "timeline" | "sessions";

type PromptFn = (opts: {
  title: string;
  label?: string;
  description?: string;
  defaultValue?: string;
  placeholder?: string;
  submitLabel?: string;
  validate?: (v: string) => string;
}) => Promise<string | null>;

export function MemoryCenterPage() {
  const [tab, setTab] = useState<Tab>("approved");
  const [projects, setProjects] = useState<Project[]>([]);
  const [active, setActive] = useState<Project | null>(null);
  const [projectFilter, setProjectFilter] = useState<string | "active" | "all">("active");
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const { prompt: promptModal } = usePromptDialog();

  useEffect(() => {
    (async () => {
      try {
        setProjects(await projectsList(false));
        setActive(await getActiveProject());
      } catch (e) { setError(String(e)); }
    })();
  }, []);

  const projectId = useMemo(() => {
    if (projectFilter === "all") return null;
    if (projectFilter === "active") return active?.id ?? null;
    return projectFilter;
  }, [projectFilter, active]);

  return (
    <>
    <div className="p-6 max-w-7xl space-y-4">
      <div className="panel p-5">
        <div className="flex items-start gap-3 flex-wrap">
          <div className="flex-1">
            <div className="h-title">Memory Center</div>
            <p className="mt-1 text-sm text-slate-400">
              Native long-term memory with provenance, temporal fields, and supersession.
              Candidates are extracted by the assistant and require your approval.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <select className="input" value={projectFilter} onChange={(e) => setProjectFilter(e.target.value as any)}>
              <option value="active">active project{active ? `: ${active.name}` : " (none)"}</option>
              <option value="all">all projects</option>
              {projects.map((p) => <option key={p.id} value={p.id}>only: {p.name}</option>)}
            </select>
            <input className="input" placeholder="Search content…" value={search} onChange={(e) => setSearch(e.target.value)} />
          </div>
        </div>
        <div className="mt-3 flex gap-1 border-b border-white/5">
          {(["approved", "candidates", "timeline", "sessions"] as Tab[]).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={`px-3 py-1.5 text-sm capitalize ${tab === t ? "border-b-2 border-cyan2-500 text-cyan2-300" : "text-slate-400"}`}
            >{t}</button>
          ))}
        </div>
        {error && <div className="mt-3 text-xs text-rose2-400 font-mono">{error}</div>}
      </div>

      {tab === "approved" && <ApprovedTab projectId={projectId} search={search} promptModal={promptModal} />}
      {tab === "candidates" && <CandidatesTab projectId={projectId} promptModal={promptModal} />}
      {tab === "timeline" && <TimelineTab projectId={projectId} />}
      {tab === "sessions" && <SessionsTab projectId={projectId} />}
    </div>
    </>
  );
}

// ---------------- approved ----------------

function ApprovedTab({
  projectId,
  search,
  promptModal,
}: {
  projectId: string | null;
  search: string;
  promptModal: PromptFn;
}) {
  const [memories, setMemories] = useState<MemoryV2[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [categories, setCategories] = useState<string[]>([]);
  const [categoryFilter, setCategoryFilter] = useState<string>("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => { memoriesV2Categories().then(setCategories).catch(() => setCategories([])); }, []);

  const refresh = async () => {
    try {
      const list = search.trim()
        ? await memoriesV2Search(search, { projectId, limit: 200 })
        : await memoriesV2List({ projectId, category: categoryFilter || null, limit: 200 });
      setMemories(list);
    } catch (e) { setError(String(e)); }
  };
  useEffect(() => { refresh(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, [projectId, categoryFilter, search]);
  // Phase P4 — Bug F: refresh when the chat pipeline
  // completes an explicit memory-save. The user does
  // not need to leave the page or click refresh; the
  // list re-renders automatically with the new
  // record.
  useEffect(() => {
    let dead = false;
    (async () => {
      const unlisten = await onMemorySaved(() => {
        if (dead) return;
        refresh();
      });
      if (dead) unlisten();
    })();
    return () => { dead = true; };
  }, []);

  const selected = useMemo(() => memories.find((m) => m.id === selectedId) ?? null, [memories, selectedId]);

  return (
    <div className="grid grid-cols-1 md:grid-cols-12 gap-4">
      <div className="md:col-span-3 panel p-3 max-h-[34rem] overflow-y-auto space-y-2">
        <select className="input" value={categoryFilter} onChange={(e) => setCategoryFilter(e.target.value)}>
          <option value="">all categories</option>
          {categories.map((c) => <option key={c} value={c}>{c}</option>)}
        </select>
        <button
          className="btn btn-primary w-full"
          onClick={async () => {
            const content = await promptModal({
              title: "New memory",
              label: "Memory content",
              placeholder: "What should NEXUS remember?",
              submitLabel: "Save memory",
              validate: (v) => (v.trim() ? "" : "Memory content is required"),
            });
            if (!content) return;
            const category = (await promptModal({
              title: "Memory category",
              label: "Category",
              defaultValue: "lesson",
              submitLabel: "Save memory",
              validate: (v) => (v.trim() ? "" : "Category is required"),
            })) || "lesson";
            try {
              await memoriesV2Create({
                project_id: projectId,
                category: category as MemoryCategory,
                content,
                source_kind: "user",
                pinned: false,
              });
              await refresh();
            } catch (e) { setError(String(e)); }
          }}
        >+ New memory</button>
        {memories.length === 0 && <div className="text-xs text-slate-500 px-2">No memories yet.</div>}
        {memories.map((m) => (
          <button
            key={m.id}
            onClick={() => setSelectedId(m.id)}
            className={`w-full text-left rounded-md p-2 ${selectedId === m.id ? "bg-cyan2-500/10 ring-1 ring-cyan2-500/30" : "hover:bg-white/5"}`}
          >
            <div className="flex items-center gap-1.5">
              <span className="chip">{m.category}</span>
              {m.pinned && <span className="chip text-amber2-400 border-amber2-400/40 bg-amber2-400/10">pinned</span>}
            </div>
            <div className="mt-1 text-sm text-slate-200 line-clamp-2">{m.content}</div>
            <div className="text-[10px] font-mono text-slate-500 mt-1">
              {m.source_kind} · {new Date(m.updated_at).toLocaleDateString()}
            </div>
          </button>
        ))}
      </div>
      <div className="md:col-span-9">
        {selected ? (
          <MemoryEditor
            key={selected.id}
            memory={selected}
            categories={categories}
            onSave={async (req) => { await memoriesV2Update(req); await refresh(); }}
            onDelete={async () => { await memoriesV2Delete(selected.id); setSelectedId(null); await refresh(); }}
            onSupersede={async () => {
              const newContent = await promptModal({
                title: "Supersede memory",
                description: "Provide a replacement for this memory. The old memory will be marked as superseded.",
                label: "Replacement content",
                defaultValue: selected.content,
                submitLabel: "Supersede",
                validate: (v) => (v.trim() ? "" : "Replacement content is required"),
              });
              if (!newContent) return;
              const replacement = await memoriesV2Create({
                project_id: selected.project_id,
                category: selected.category as MemoryCategory,
                content: newContent,
                source_kind: "user",
                pinned: selected.pinned,
              });
              await memoriesV2Supersede(selected.id, replacement.id);
              setSelectedId(replacement.id);
              await refresh();
            }}
          />
        ) : (
          <div className="panel p-6 text-sm text-slate-500">Select a memory to inspect or edit.</div>
        )}
      </div>
      {error && <div className="md:col-span-12 text-xs text-rose2-400 font-mono">{error}</div>}
    </div>
  );
}

function MemoryEditor({ memory, categories, onSave, onDelete, onSupersede }: {
  memory: MemoryV2;
  categories: string[];
  onSave: (req: { id: string; content?: string; category?: string; pinned?: boolean; event_at?: { value: number | null } | null; valid_from?: { value: number | null } | null; valid_until?: { value: number | null } | null }) => Promise<void>;
  onDelete: () => Promise<void>;
  onSupersede: () => Promise<void>;
}) {
  const [content, setContent] = useState(memory.content);
  const [category, setCategory] = useState(memory.category);
  const [pinned, setPinned] = useState(memory.pinned);
  const [evidence, setEvidence] = useState<Evidence[]>([]);
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    setContent(memory.content);
    setCategory(memory.category);
    setPinned(memory.pinned);
    setDirty(false);
  }, [memory.id]);

  useEffect(() => { memoryEvidenceList(memory.id).then(setEvidence).catch(() => setEvidence([])); }, [memory.id]);

  return (
    <div className="panel p-4 space-y-3">
      <div className="flex items-center gap-2">
        <select className="input flex-1" value={category} onChange={(e) => { setCategory(e.target.value); setDirty(true); }}>
          {categories.map((c) => <option key={c} value={c}>{c}</option>)}
        </select>
        <label className="flex items-center gap-1 text-[11px] text-slate-400">
          <input type="checkbox" className="accent-cyan2-500" checked={pinned} onChange={(e) => { setPinned(e.target.checked); setDirty(true); }} />
          pinned
        </label>
      </div>
      <textarea
        className="input min-h-[8rem] font-mono text-sm"
        value={content}
        onChange={(e) => { setContent(e.target.value); setDirty(true); }}
      />
      <div className="text-[10px] font-mono text-slate-500 grid grid-cols-2 gap-1">
        <div>source: {memory.source_kind} {memory.source_label ? `(${memory.source_label})` : ""}</div>
        <div>hash: <code>{memory.content_hash}</code></div>
        <div>created: {new Date(memory.created_at).toLocaleString()}</div>
        <div>verified: {memory.verified_at ? new Date(memory.verified_at).toLocaleString() : "—"}</div>
        {memory.event_at && <div>event: {new Date(memory.event_at).toLocaleString()}</div>}
        {memory.valid_from && <div>valid from: {new Date(memory.valid_from).toLocaleString()}</div>}
        {memory.valid_until && <div>valid until: {new Date(memory.valid_until).toLocaleString()}</div>}
        {memory.superseded_by && <div className="text-amber2-400">superseded by: {memory.superseded_by}</div>}
      </div>
      <EvidencePanel memoryId={memory.id} evidence={evidence} onChanged={() => memoryEvidenceList(memory.id).then(setEvidence).catch(() => setEvidence([]))} />
      <div className="flex flex-wrap gap-2">
        <button className="btn btn-primary" disabled={!dirty} onClick={() => onSave({ id: memory.id, content, category, pinned })}>Save</button>
        <button className="btn" onClick={onSupersede}>Supersede…</button>
        <button className="btn text-rose2-400" onClick={onDelete}>Delete</button>
      </div>
    </div>
  );
}

function EvidencePanel({ memoryId, evidence, onChanged }: { memoryId: string; evidence: Evidence[]; onChanged: () => void }) {
  const [kind, setKind] = useState("document");
  const [ref, setRef] = useState("");
  const [loc, setLoc] = useState("");
  const [excerpt, setExcerpt] = useState("");
  return (
    <div className="border border-white/5 rounded-md p-2 space-y-1.5">
      <div className="text-[11px] font-mono text-slate-500">evidence</div>
      {evidence.length === 0 && <div className="text-[11px] text-slate-500">no evidence yet</div>}
      {evidence.map((e) => (
        <div key={e.id} className="text-[11px] text-slate-300 flex items-center gap-1.5">
          <span className="chip">{e.source_kind}</span>
          {e.source_id && <code className="text-[10px] text-slate-500">{e.source_id}</code>}
          {e.source_location && <span className="text-slate-500">· {e.source_location}</span>}
          {e.verified ? <span className="text-cyan2-400 ml-auto">verified</span> : <button className="ml-auto text-cyan2-400 hover:underline" onClick={async () => { await memoryEvidenceAdd(memoryId, e.source_kind, e.source_id, e.source_location, e.excerpt); /* promote via re-adding with same content; in real world use verify */ onChanged(); }}>verify</button>}
        </div>
      ))}
      <div className="flex flex-wrap gap-1.5 pt-1">
        <input className="input w-32" placeholder="kind" value={kind} onChange={(e) => setKind(e.target.value)} />
        <input className="input flex-1" placeholder="source id (optional)" value={ref} onChange={(e) => setRef(e.target.value)} />
        <input className="input w-32" placeholder="location" value={loc} onChange={(e) => setLoc(e.target.value)} />
        <input className="input flex-1" placeholder="excerpt" value={excerpt} onChange={(e) => setExcerpt(e.target.value)} />
        <button
          className="btn"
          onClick={async () => {
            if (!kind) return;
            await memoryEvidenceAdd(memoryId, kind, ref || null, loc || null, excerpt || null);
            setRef(""); setLoc(""); setExcerpt("");
            onChanged();
          }}
        >add</button>
      </div>
    </div>
  );
}

// ---------------- candidates ---------------

function CandidatesTab({
  projectId,
  promptModal,
}: {
  projectId: string | null;
  promptModal: PromptFn;
}) {
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try { setCandidates(await memoryCandidatesList("pending", 200)); }
    catch (e) { setError(String(e)); }
  };
  useEffect(() => { refresh(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);

  return (
    <div className="panel p-4 space-y-3">
      <div className="flex items-center gap-2">
        <button
          className="btn"
          onClick={async () => {
            const content = await promptModal({
              title: "New memory candidate",
              label: "Candidate content",
              submitLabel: "Propose",
              validate: (v) => (v.trim() ? "" : "Content is required"),
            });
            if (!content) return;
            const category = (await promptModal({
              title: "Memory category",
              label: "Category",
              defaultValue: "project_fact",
              submitLabel: "Propose",
              validate: (v) => (v.trim() ? "" : "Category is required"),
            })) || "project_fact";
            try {
              await memoryCandidatesPropose({
                category: category as MemoryCategory,
                content,
                source_kind: "user",
                confidence: 0.5,
                project_id: projectId,
              });
              await refresh();
            } catch (e) { setError(String(e)); }
          }}
        >+ Add candidate</button>
        <span className="text-[11px] text-slate-500">Auto-extracted candidates require your approval before they become a long-term memory.</span>
      </div>
      {error && <div className="text-xs text-rose2-400 font-mono">{error}</div>}
      <div className="space-y-2">
        {candidates.length === 0 && <div className="text-sm text-slate-500">No pending candidates.</div>}
        {candidates.map((c) => (
          <div key={c.id} className="border border-white/5 rounded-md p-3 flex items-start gap-3">
            <div className="flex-1">
              <div className="flex items-center gap-2">
                <span className="chip">{c.category}</span>
                <span className="chip text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10">conf {(c.confidence * 100).toFixed(0)}%</span>
                <span className="text-[10px] text-slate-500">source: {c.source_kind}</span>
              </div>
              <div className="mt-1 text-sm text-slate-200">{c.content}</div>
              {c.source_excerpt && <div className="mt-1 text-[11px] text-slate-500 italic line-clamp-2">"{c.source_excerpt}"</div>}
            </div>
            <div className="flex flex-col gap-1">
              <button className="btn btn-primary" onClick={async () => { await memoryCandidatesApprove(c.id); await refresh(); }}>Approve</button>
              <button className="btn text-rose2-400" onClick={async () => { await memoryCandidatesReject(c.id, "user rejected"); await refresh(); }}>Reject</button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------- timeline -----------------

function TimelineTab({ projectId }: { projectId: string | null }) {
  const [items, setItems] = useState<MemoryV2[]>([]);
  useEffect(() => {
    memoriesV2List({ projectId, limit: 200 }).then(setItems).catch(() => setItems([]));
  }, [projectId]);

  // Bucket by day.
  const byDay = useMemo(() => {
    const buckets: Record<string, MemoryV2[]> = {};
    for (const m of items) {
      const d = new Date(m.updated_at);
      const key = d.toISOString().slice(0, 10);
      (buckets[key] ??= []).push(m);
    }
    return buckets;
  }, [items]);

  const days = Object.keys(byDay).sort().reverse().slice(0, 30);

  return (
    <div className="panel p-4 space-y-4">
      <div className="text-[11px] text-slate-500">Last 30 days of memory changes.</div>
      {days.length === 0 && <div className="text-sm text-slate-500">No activity yet.</div>}
      {days.map((d) => (
        <div key={d}>
          <div className="text-[11px] font-mono text-cyan2-400 mb-1">{d}</div>
          <div className="space-y-1.5 pl-3 border-l border-white/5">
            {byDay[d].map((m) => (
              <div key={m.id} className="text-[12px] text-slate-300">
                <span className="chip mr-1">{m.category}</span>
                <span>{m.content}</span>
                <span className="text-[10px] text-slate-500 ml-2">· {m.source_kind}</span>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

// ---------------- sessions -----------------

function SessionsTab({ projectId }: { projectId: string | null }) {
  const [sessions, setSessions] = useState<Session[]>([]);
  useEffect(() => {
    if (!projectId) { setSessions([]); return; }
    memorySessionsList(projectId, 50).then(setSessions).catch(() => setSessions([]));
  }, [projectId]);

  if (!projectId) return <div className="panel p-4 text-sm text-slate-500">Select a specific project to see its session history.</div>;
  return (
    <div className="space-y-3">
      {sessions.length === 0 && <div className="panel p-4 text-sm text-slate-500">No sessions recorded for this project yet.</div>}
      {sessions.map((s) => (
        <div key={s.id} className="panel p-4 space-y-1.5">
          <div className="flex items-center gap-2">
            <div className="text-slate-200 font-medium">{s.title}</div>
            <div className="text-[10px] font-mono text-slate-500 ml-auto">
              {new Date(s.started_at).toLocaleString()} → {new Date(s.ended_at).toLocaleString()}
            </div>
          </div>
          <div className="text-sm text-slate-300">{s.summary}</div>
          {s.decisions_made && <div className="text-[12px] text-slate-400"><span className="text-cyan2-400">decisions:</span> {s.decisions_made}</div>}
          {s.work_completed && <div className="text-[12px] text-slate-400"><span className="text-cyan2-400">completed:</span> {s.work_completed}</div>}
          {s.open_questions && <div className="text-[12px] text-slate-400"><span className="text-cyan2-400">open:</span> {s.open_questions}</div>}
          {s.next_actions && <div className="text-[12px] text-slate-400"><span className="text-cyan2-400">next:</span> {s.next_actions}</div>}
        </div>
      ))}
    </div>
  );
}
