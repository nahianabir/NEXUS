// Research Studio. Phase G.
//
// Four tabs:
//   1. Overview — the project's question, description, and a list of
//      papers with metadata.
//   2. Papers — add/edit papers (manual metadata; the vault document
//      link is optional).
//   3. Claims — extracted findings, methods, limitations, questions.
//      Filterable by category and paper.
//   4. Matrix — pick or create a comparison matrix; rows are papers,
//      columns are criteria; cells are text/number/bool/rating.
//   5. Report — build a Markdown snapshot of the current evidence and
//      copy or save it.

import { useEffect, useMemo, useState } from "react";
import {
  type ComparisonMatrix,
  type MatrixCell,
  type MatrixColumn,
  type PaperClaim,
  type ResearchPaper,
  type ResearchProject,
  type ResearchReport,
  researchClaimAdd,
  researchClaimDelete,
  researchClaimList,
  researchMatrixCellSet,
  researchMatrixColumnAdd,
  researchMatrixColumnDelete,
  researchMatrixColumns,
  researchMatrixCreate,
  researchMatrixDelete,
  researchMatrixCells,
  researchMatrixList,
  researchPaperAdd,
  researchPaperDelete,
  researchPaperList,
  researchPaperUpdate,
  researchProjectCreate,
  researchProjectDelete,
  researchProjectList,
  researchProjectUpdate,
  researchReportBuild,
  researchReportDelete,
  researchReportList,
  researchReportSave,
} from "../lib/research";
import { usePromptDialog } from "../hooks/usePromptDialog";
import { MarkdownView } from "../components/MarkdownView";

type Tab = "overview" | "papers" | "claims" | "matrix" | "report";

const CLAIM_CATEGORIES = ["finding", "method", "limitation", "question", "fact"] as const;

export function ResearchPage() {
  const { prompt: promptModal, confirm: confirmModal } = usePromptDialog();
  const [projects, setProjects] = useState<ResearchProject[]>([]);
  const [activeId, setActiveId] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("overview");
  const [error, setError] = useState<string | null>(null);

  const [papers, setPapers] = useState<ResearchPaper[]>([]);
  const [claims, setClaims] = useState<PaperClaim[]>([]);
  const [matrices, setMatrices] = useState<ComparisonMatrix[]>([]);
  const [activeMatrix, setActiveMatrix] = useState<ComparisonMatrix | null>(null);
  const [matrixColumns, setMatrixColumns] = useState<MatrixColumn[]>([]);
  const [matrixCells, setMatrixCells] = useState<MatrixCell[]>([]);
  const [reports, setReports] = useState<ResearchReport[]>([]);
  const [reportPreview, setReportPreview] = useState<string | null>(null);

  const refresh = async (id: string | null) => {
    try {
      setProjects(await researchProjectList(null, null, 50));
      if (id) {
        setPapers(await researchPaperList(id));
        setClaims(await researchClaimList(id, null, null));
        setMatrices(await researchMatrixList(id));
        setReports(await researchReportList(id));
      } else {
        setPapers([]);
        setClaims([]);
        setMatrices([]);
        setReports([]);
      }
    } catch (e) { setError(String(e)); }
  };

  useEffect(() => { void refresh(null); }, []);

  useEffect(() => {
    if (!activeMatrix) { setMatrixColumns([]); setMatrixCells([]); return; }
    (async () => {
      try {
        setMatrixColumns(await researchMatrixColumns(activeMatrix.id));
        setMatrixCells(await researchMatrixCells(activeMatrix.id));
      } catch (e) { setError(String(e)); }
    })();
  }, [activeMatrix?.id]);

  const active = useMemo(() => projects.find((p) => p.id === activeId) ?? null, [projects, activeId]);

  const onNewProject = async () => {
    const title = await promptModal({
      title: "New research project",
      label: "Project title",
      defaultValue: "Untitled review",
      submitLabel: "Create project",
      validate: (v) => (v.trim() ? "" : "Title is required"),
    });
    if (!title) return;
    const created = await researchProjectCreate({
      id: "", workspace_id: null, title, question: "", description: null, status: "active", created_at: 0, updated_at: 0,
    });
    await refresh(null);
    setActiveId(created.id);
    setTab("overview");
  };

  const onUpdateProject = async (patch: Partial<ResearchProject>) => {
    if (!active) return;
    const updated = await researchProjectUpdate({ ...active, ...patch });
    await refresh(updated.id);
  };

  const onDeleteProject = async (id: string) => {
    const ok = await confirmModal({
      title: "Delete research project",
      description: "This deletes the project, all its papers, claims, matrices, and reports. This cannot be undone.",
      submitLabel: "Delete project",
      destructive: true,
    });
    if (!ok) return;
    await researchProjectDelete(id);
    setActiveId(null);
    await refresh(null);
  };

  return (
    <>
    <div className="grid h-full grid-cols-12 gap-4 p-4 min-h-0">
      <aside className="col-span-3 panel p-3 flex flex-col min-h-0">
        <div className="flex items-center justify-between">
          <div className="h-title">Research projects</div>
          <button className="btn btn-primary" onClick={onNewProject}>New</button>
        </div>
        <div className="mt-3 space-y-1 overflow-y-auto flex-1 min-h-0">
          {projects.length === 0 && <div className="text-xs text-slate-500 px-2">No projects yet. Click "New" to start one.</div>}
          {projects.map((p) => (
            <button
              key={p.id}
              className={`w-full text-left rounded-lg p-2 transition ${activeId === p.id ? "bg-cyan2-500/10 ring-1 ring-cyan2-500/30" : "hover:bg-white/5"}`}
              onClick={() => { setActiveId(p.id); setTab("overview"); }}
            >
              <div className="text-sm text-slate-100 truncate">{p.title}</div>
              <div className="text-[11px] text-slate-500 font-mono truncate">{p.question || "(no question)"}</div>
              <div className="mt-1 flex gap-1 text-[10px]">
                <span className="chip">{p.status}</span>
                <span className="kbd" onClick={(e) => { e.stopPropagation(); void onDeleteProject(p.id); }}>delete</span>
              </div>
            </button>
          ))}
        </div>
      </aside>

      <section className="col-span-9 panel p-4 flex flex-col min-h-0">
        {!active ? (
          <div className="text-sm text-slate-500 text-center py-12">
            Select or create a research project to start.
          </div>
        ) : (
          <>
            <div className="flex items-center gap-2">
              <div className="h-title flex-1 truncate">{active.title}</div>
              <div className="flex gap-1 text-[11px]">
                {(["overview", "papers", "claims", "matrix", "report"] as Tab[]).map((t) => (
                  <button
                    key={t}
                    className={`kbd ${tab === t ? "text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10" : ""}`}
                    onClick={() => setTab(t)}
                  >{t}</button>
                ))}
              </div>
            </div>
            {error && <div className="mt-2 text-[11px] text-rose2-400 font-mono">{error}</div>}
            <div className="mt-3 flex-1 min-h-0 overflow-y-auto pr-1">
              {tab === "overview" && <OverviewTab project={active} onUpdate={onUpdateProject} papers={papers} claims={claims} matrices={matrices} />}
              {tab === "papers" && <PapersTab projectId={active.id} papers={papers} onChange={async () => { await refresh(active.id); }} />}
              {tab === "claims" && <ClaimsTab projectId={active.id} papers={papers} claims={claims} onChange={async () => { await refresh(active.id); }} />}
              {tab === "matrix" && <MatrixTab projectId={active.id} papers={papers} matrices={matrices} activeMatrix={activeMatrix} setActiveMatrix={setActiveMatrix} columns={matrixColumns} cells={matrixCells} onChange={async () => { await refresh(active.id); }} />}
              {tab === "report" && <ReportTab projectId={active.id} reports={reports} preview={reportPreview} setPreview={setReportPreview} onChange={async () => { await refresh(active.id); }} />}
            </div>
          </>
        )}
      </section>
    </div>
    </>
  );
}

function OverviewTab({ project, onUpdate, papers, claims, matrices }: { project: ResearchProject; onUpdate: (p: Partial<ResearchProject>) => void; papers: ResearchPaper[]; claims: PaperClaim[]; matrices: ComparisonMatrix[]; }) {
  return (
    <div className="space-y-3 text-sm text-slate-300">
      <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
        <div className="panel-soft p-3 text-center">
          <div className="text-2xl text-cyan2-400 font-semibold">{papers.length}</div>
          <div className="text-[11px] text-slate-500 uppercase tracking-wide">papers</div>
        </div>
        <div className="panel-soft p-3 text-center">
          <div className="text-2xl text-cyan2-400 font-semibold">{claims.length}</div>
          <div className="text-[11px] text-slate-500 uppercase tracking-wide">claims</div>
        </div>
        <div className="panel-soft p-3 text-center">
          <div className="text-2xl text-cyan2-400 font-semibold">{matrices.length}</div>
          <div className="text-[11px] text-slate-500 uppercase tracking-wide">matrices</div>
        </div>
      </div>
      <div className="panel-soft p-3 space-y-2">
        <label className="block">
          <div className="text-[11px] text-slate-500 uppercase tracking-wide mb-1">Title</div>
          <input className="input" value={project.title} onChange={(e) => onUpdate({ title: e.target.value })} />
        </label>
        <label className="block">
          <div className="text-[11px] text-slate-500 uppercase tracking-wide mb-1">Research question</div>
          <textarea className="input min-h-[3rem]" value={project.question} onChange={(e) => onUpdate({ question: e.target.value })} />
        </label>
        <label className="block">
          <div className="text-[11px] text-slate-500 uppercase tracking-wide mb-1">Description</div>
          <textarea className="input min-h-[3rem]" value={project.description ?? ""} onChange={(e) => onUpdate({ description: e.target.value })} />
        </label>
      </div>
    </div>
  );
}

function PapersTab({ projectId, papers, onChange }: { projectId: string; papers: ResearchPaper[]; onChange: () => Promise<void> }) {
  const { confirm: confirmModal } = usePromptDialog();
  const [draft, setDraft] = useState<Partial<ResearchPaper>>({});
  const onAdd = async () => {
    if (!draft.title?.trim()) return;
    await researchPaperAdd({
      id: "",
      research_project_id: projectId,
      document_id: null,
      title: draft.title!,
      authors: draft.authors ?? null,
      venue: draft.venue ?? null,
      year: draft.year ?? null,
      doi: draft.doi ?? null,
      url: draft.url ?? null,
      notes: draft.notes ?? null,
      position: papers.length,
      created_at: 0,
    });
    setDraft({});
    await onChange();
  };
  const onEdit = async (p: ResearchPaper, patch: Partial<ResearchPaper>) => {
    await researchPaperUpdate({ ...p, ...patch });
    await onChange();
  };
  const onDelete = async (id: string) => {
    const ok = await confirmModal({
      title: "Remove paper",
      description: "Remove this paper from the project? The paper record itself is preserved; only the project link is removed.",
      submitLabel: "Remove",
      destructive: true,
    });
    if (!ok) return;
    await researchPaperDelete(id);
    await onChange();
  };
  return (
    <>
    <div className="space-y-3">
      <div className="panel-soft p-3 space-y-2">
        <div className="text-[11px] text-slate-500 uppercase tracking-wide">Add a paper</div>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
          <input className="input" placeholder="Title" value={draft.title ?? ""} onChange={(e) => setDraft({ ...draft, title: e.target.value })} />
          <input className="input" placeholder="Authors" value={draft.authors ?? ""} onChange={(e) => setDraft({ ...draft, authors: e.target.value })} />
          <input className="input" placeholder="Venue" value={draft.venue ?? ""} onChange={(e) => setDraft({ ...draft, venue: e.target.value })} />
          <input className="input" type="number" placeholder="Year" value={draft.year ?? ""} onChange={(e) => setDraft({ ...draft, year: e.target.value ? Number(e.target.value) : null })} />
          <input className="input" placeholder="DOI" value={draft.doi ?? ""} onChange={(e) => setDraft({ ...draft, doi: e.target.value })} />
          <input className="input" placeholder="URL" value={draft.url ?? ""} onChange={(e) => setDraft({ ...draft, url: e.target.value })} />
        </div>
        <textarea className="input" placeholder="Notes" value={draft.notes ?? ""} onChange={(e) => setDraft({ ...draft, notes: e.target.value })} />
        <button className="btn btn-primary" onClick={onAdd} disabled={!draft.title?.trim()}>Add paper</button>
      </div>
      <div className="space-y-1.5">
        {papers.length === 0 && <div className="text-xs text-slate-500">No papers yet.</div>}
        {papers.map((p) => (
          <div key={p.id} className="panel-soft p-3 text-[12px] text-slate-300 space-y-1">
            <div className="flex items-center gap-2">
              <input className="input flex-1" defaultValue={p.title} onBlur={(e) => { if (e.target.value !== p.title) void onEdit(p, { title: e.target.value }); }} />
              <span className="kbd" onClick={() => { void onDelete(p.id); }}>delete</span>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-2 text-[11px]">
              <input className="input" placeholder="Authors" defaultValue={p.authors ?? ""} onBlur={(e) => { if (e.target.value !== (p.authors ?? "")) void onEdit(p, { authors: e.target.value }); }} />
              <input className="input" placeholder="Venue" defaultValue={p.venue ?? ""} onBlur={(e) => { if (e.target.value !== (p.venue ?? "")) void onEdit(p, { venue: e.target.value }); }} />
              <input className="input" type="number" placeholder="Year" defaultValue={p.year ?? ""} onBlur={(e) => { const n = e.target.value ? Number(e.target.value) : null; if (n !== p.year) void onEdit(p, { year: n }); }} />
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2 text-[11px]">
              <input className="input" placeholder="DOI" defaultValue={p.doi ?? ""} onBlur={(e) => { if (e.target.value !== (p.doi ?? "")) void onEdit(p, { doi: e.target.value }); }} />
              <input className="input" placeholder="URL" defaultValue={p.url ?? ""} onBlur={(e) => { if (e.target.value !== (p.url ?? "")) void onEdit(p, { url: e.target.value }); }} />
            </div>
            <textarea className="input" placeholder="Notes" defaultValue={p.notes ?? ""} onBlur={(e) => { if (e.target.value !== (p.notes ?? "")) void onEdit(p, { notes: e.target.value }); }} />
          </div>
        ))}
      </div>
    </div>
    </>
  );
}

function ClaimsTab({ projectId, papers, claims, onChange }: { projectId: string; papers: ResearchPaper[]; claims: PaperClaim[]; onChange: () => Promise<void> }) {
  const { confirm: confirmModal } = usePromptDialog();
  const [draft, setDraft] = useState<{ claim: string; category: string; paper_id: string | null; evidence: string; confidence: number }>({ claim: "", category: "finding", paper_id: null, evidence: "", confidence: 0.5 });
  const onAdd = async () => {
    if (!draft.claim.trim()) return;
    await researchClaimAdd({
      id: "",
      research_project_id: projectId,
      paper_id: draft.paper_id,
      document_id: null,
      claim: draft.claim,
      category: draft.category,
      evidence: draft.evidence || null,
      confidence: draft.confidence,
      kg_entity_id: null,
      created_at: 0,
    });
    setDraft({ claim: "", category: "finding", paper_id: null, evidence: "", confidence: 0.5 });
    await onChange();
  };
  const onDelete = async (id: string) => {
    const ok = await confirmModal({
      title: "Delete claim",
      description: "Remove this claim from the project? This cannot be undone.",
      submitLabel: "Delete claim",
      destructive: true,
    });
    if (ok) { await researchClaimDelete(id); await onChange(); }
  };
  const grouped = useMemo(() => {
    const m = new Map<string, PaperClaim[]>();
    for (const c of claims) {
      const list = m.get(c.category) ?? [];
      list.push(c);
      m.set(c.category, list);
    }
    return m;
  }, [claims]);
  return (
    <>
    <div className="space-y-3">
      <div className="panel-soft p-3 space-y-2">
        <div className="text-[11px] text-slate-500 uppercase tracking-wide">Add a claim</div>
        <textarea className="input min-h-[3rem]" placeholder="Claim" value={draft.claim} onChange={(e) => setDraft({ ...draft, claim: e.target.value })} />
        <div className="grid grid-cols-1 md:grid-cols-3 gap-2">
          <select className="input" value={draft.category} onChange={(e) => setDraft({ ...draft, category: e.target.value })}>
            {CLAIM_CATEGORIES.map((c) => <option key={c} value={c}>{c}</option>)}
          </select>
          <select className="input" value={draft.paper_id ?? ""} onChange={(e) => setDraft({ ...draft, paper_id: e.target.value || null })}>
            <option value="">(no specific paper)</option>
            {papers.map((p) => <option key={p.id} value={p.id}>{p.title}</option>)}
          </select>
          <input className="input" type="number" min={0} max={1} step={0.1} value={draft.confidence} onChange={(e) => setDraft({ ...draft, confidence: Number(e.target.value) })} />
        </div>
        <input className="input" placeholder="Evidence (optional quote / page / section)" value={draft.evidence} onChange={(e) => setDraft({ ...draft, evidence: e.target.value })} />
        <button className="btn btn-primary" onClick={onAdd} disabled={!draft.claim.trim()}>Add claim</button>
      </div>
      <div className="space-y-2">
        {claims.length === 0 && <div className="text-xs text-slate-500">No claims yet.</div>}
        {Array.from(grouped.entries()).map(([cat, list]) => (
          <div key={cat} className="panel-soft p-2">
            <div className="text-[11px] text-slate-500 uppercase tracking-wide mb-1">{cat} <span className="text-slate-600">({list.length})</span></div>
            <ul className="space-y-1 text-[12px]">
              {list.map((c) => {
                const p = papers.find((p) => p.id === c.paper_id);
                return (
                  <li key={c.id} className="border border-white/5 rounded p-2">
                    <div className="flex items-start gap-2">
                      <div className="flex-1">
                        <div className="text-slate-200">{c.claim}</div>
                        {c.evidence && <div className="mt-1 text-[11px] text-slate-500 italic">{c.evidence}</div>}
                        <div className="mt-1 text-[10px] font-mono text-slate-500">
                          {p ? `paper: ${p.title}` : "(no paper)"}
                          {` · conf ${c.confidence.toFixed(2)}`}
                        </div>
                      </div>
                      <span className="kbd" onClick={() => { void onDelete(c.id); }}>delete</span>
                    </div>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </div>
    </div>
    </>
  );
}

function MatrixTab({ projectId, papers, matrices, activeMatrix, setActiveMatrix, columns, cells, onChange }: { projectId: string; papers: ResearchPaper[]; matrices: ComparisonMatrix[]; activeMatrix: ComparisonMatrix | null; setActiveMatrix: (m: ComparisonMatrix | null) => void; columns: MatrixColumn[]; cells: MatrixCell[]; onChange: () => Promise<void> }) {
  const { prompt: promptModal, confirm: confirmModal } = usePromptDialog();
  const onCreate = async () => {
    const title = await promptModal({
      title: "New comparison matrix",
      label: "Matrix title",
      defaultValue: "Methods comparison",
      submitLabel: "Create matrix",
      validate: (v) => (v.trim() ? "" : "Title is required"),
    });
    if (!title) return;
    const m = await researchMatrixCreate({ id: "", research_project_id: projectId, title, description: null, created_at: 0 });
    await onChange();
    setActiveMatrix(m);
  };
  const onDeleteMatrix = async (id: string) => {
    const ok = await confirmModal({
      title: "Delete matrix",
      description: "Delete this matrix and all its cells? This cannot be undone.",
      submitLabel: "Delete matrix",
      destructive: true,
    });
    if (!ok) return;
    await researchMatrixDelete(id);
    if (activeMatrix?.id === id) setActiveMatrix(null);
    await onChange();
  };
  const onAddColumn = async () => {
    if (!activeMatrix) return;
    const label = await promptModal({
      title: "New column",
      label: "Column label",
      defaultValue: "Sample size",
      submitLabel: "Next",
      validate: (v) => (v.trim() ? "" : "Label is required"),
    });
    if (!label) return;
    const kind = (await promptModal({
      title: "Column kind",
      label: "Kind",
      defaultValue: "text",
      submitLabel: "Add column",
      validate: (v) => (["text", "number", "bool", "rating"].includes(v.trim()) ? "" : "Must be one of: text, number, bool, rating"),
    })) ?? "text";
    if (!["text", "number", "bool", "rating"].includes(kind)) return;
    await researchMatrixColumnAdd({ id: "", matrix_id: activeMatrix.id, label, kind, position: columns.length });
    await onChange();
  };
  const onDeleteColumn = async (id: string) => {
    const ok = await confirmModal({
      title: "Delete column",
      description: "Delete this column? Existing cell values for this column will be removed.",
      submitLabel: "Delete column",
      destructive: true,
    });
    if (!ok) return;
    await researchMatrixColumnDelete(id);
    await onChange();
  };
  const onSetCell = async (paperId: string, col: MatrixColumn, raw: string) => {
    if (!activeMatrix) return;
    let value_text: string | null = null;
    let value_number: number | null = null;
    let value_bool: boolean | null = null;
    if (col.kind === "number" || col.kind === "rating") {
      const n = Number(raw);
      value_number = isNaN(n) ? null : n;
    } else if (col.kind === "bool") {
      value_bool = raw === "true" || raw === "yes" || raw === "1";
    } else {
      value_text = raw || null;
    }
    await researchMatrixCellSet({
      id: "", matrix_id: activeMatrix.id, paper_id: paperId, column_id: col.id,
      value_text, value_number, value_bool, updated_at: 0,
    });
    await onChange();
  };
  return (
    <div className="space-y-3">
      <div className="panel-soft p-2 flex flex-wrap items-center gap-2">
        <button className="btn btn-primary" onClick={onCreate}>New matrix</button>
        <select className="input" value={activeMatrix?.id ?? ""} onChange={(e) => setActiveMatrix(matrices.find((m) => m.id === e.target.value) ?? null)}>
          <option value="">(select matrix)</option>
          {matrices.map((m) => <option key={m.id} value={m.id}>{m.title}</option>)}
        </select>
        {activeMatrix && <span className="kbd" onClick={() => { void onDeleteMatrix(activeMatrix.id); }}>delete matrix</span>}
        {activeMatrix && <span className="kbd" onClick={onAddColumn}>add column</span>}
        {activeMatrix && <span className="ml-auto text-[11px] text-slate-500">{columns.length} columns · {cells.length} cells</span>}
      </div>
      {activeMatrix && papers.length > 0 && columns.length > 0 ? (
        <div className="overflow-x-auto">
          <table className="min-w-full text-[12px]">
            <thead>
              <tr className="text-left text-slate-500">
                <th className="px-2 py-1">Paper</th>
                {columns.map((c) => (
                  <th key={c.id} className="px-2 py-1">
                    <div className="flex items-center gap-1">
                      <span>{c.label}</span>
                      <span className="text-slate-600 font-mono">[{c.kind}]</span>
                      <span className="kbd" onClick={() => { void onDeleteColumn(c.id); }}>×</span>
                    </div>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {papers.map((p) => (
                <tr key={p.id} className="border-t border-white/5">
                  <td className="px-2 py-1 text-slate-200 max-w-xs truncate" title={p.title}>{p.title}</td>
                  {columns.map((c) => {
                    const cell = cells.find((x) => x.paper_id === p.id && x.column_id === c.id);
                    const initial = cell?.value_text
                      ?? (cell?.value_number != null ? String(cell.value_number) : "")
                      ?? (cell?.value_bool != null ? (cell.value_bool ? "yes" : "no") : "");
                    return (
                      <td key={c.id} className="px-2 py-1">
                        {c.kind === "bool" ? (
                          <select className="input" defaultValue={cell?.value_bool == null ? "" : cell.value_bool ? "yes" : "no"} onChange={(e) => { void onSetCell(p.id, c, e.target.value); }}>
                            <option value="">(unset)</option>
                            <option value="yes">yes</option>
                            <option value="no">no</option>
                          </select>
                        ) : (
                          <input
                            className="input"
                            defaultValue={initial}
                            placeholder={c.kind === "rating" ? "0..5" : c.kind === "number" ? "0" : ""}
                            onBlur={(e) => { if (e.target.value !== initial) void onSetCell(p.id, c, e.target.value); }}
                          />
                        )}
                      </td>
                    );
                  })}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="text-xs text-slate-500">{activeMatrix ? "Add at least one paper and one column to render the matrix." : "Select a matrix above to begin."}</div>
      )}
    </div>
  );
}

function ReportTab({ projectId, reports, preview, setPreview, onChange }: { projectId: string; reports: ResearchReport[]; preview: string | null; setPreview: (s: string | null) => void; onChange: () => Promise<void> }) {
  const { confirm: confirmModal } = usePromptDialog();
  const [title, setTitle] = useState("Snapshot");
  const onBuild = async () => {
    const md = await researchReportBuild(projectId);
    setPreview(md);
  };
  const onSave = async () => {
    if (!preview) return;
    const id = crypto.randomUUID();
    await researchReportSave({ id, research_project_id: projectId, title: title || "Snapshot", body_md: preview, paper_ids: [], claim_ids: [], created_at: 0, updated_at: 0 });
    await onChange();
  };
  const onDelete = async (id: string) => {
    const ok = await confirmModal({
      title: "Delete report",
      description: "Delete this saved report? This cannot be undone.",
      submitLabel: "Delete report",
      destructive: true,
    });
    if (!ok) return;
    await researchReportDelete(id);
    await onChange();
  };
  return (
    <>
    <div className="space-y-3">
      <div className="panel-soft p-3 flex flex-wrap items-center gap-2">
        <input className="input flex-1" value={title} onChange={(e) => setTitle(e.target.value)} />
        <button className="btn btn-primary" onClick={onBuild}>Build Markdown</button>
        <button className="btn" onClick={onSave} disabled={!preview}>Save</button>
      </div>
      {preview && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          <div className="panel-soft p-3">
            <div className="h-sub mb-1">Markdown source</div>
            <pre className="text-[11px] text-slate-400 whitespace-pre-wrap max-h-[28rem] overflow-y-auto">{preview}</pre>
          </div>
          <div className="panel-soft p-3">
            <div className="h-sub mb-1">Preview</div>
            <div className="prose prose-invert max-w-none text-sm"><MarkdownView text={preview} /></div>
          </div>
        </div>
      )}
      <div>
        <div className="h-sub mb-1">Saved reports</div>
        {reports.length === 0 && <div className="text-xs text-slate-500">No saved reports yet.</div>}
        <ul className="space-y-1.5">
          {reports.map((r) => (
            <li key={r.id} className="panel-soft p-2 text-[12px] flex items-center gap-2">
              <span className="text-slate-200 flex-1 truncate">{r.title}</span>
              <span className="font-mono text-[10px] text-slate-500">{new Date(r.updated_at).toLocaleString()}</span>
              <span className="kbd" onClick={() => setPreview(r.body_md)}>view</span>
              <span className="kbd" onClick={() => { void onDelete(r.id); }}>delete</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
    </>
  );
}