// Notes — Phase 5.
//
// A free-form note editor attached to the active project (or none). The
// most recent notes for the active project are prepended to every chat
// turn as part of the project context block, so this page is where you
// capture project-scoped facts the model should remember.
//
// Phase 9 will replace this with a document-intelligence view (PDF
// viewer, table extraction, OCR). The notes table itself is independent
// and survives the migration.

import { useEffect, useMemo, useState } from "react";
import {
  type Note,
  type Project,
  getActiveProject,
  notesCreate,
  notesDelete,
  notesList,
  notesUpdate,
  projectsList,
} from "../lib/projects";
import { MarkdownView } from "../components/MarkdownView";
import { DocumentIntelligence } from "../components/DocumentIntelligence";
import { usePromptDialog } from "../hooks/usePromptDialog";

export function DocumentsPage() {
  const { confirm: confirmModal } = usePromptDialog();
  const [projects, setProjects] = useState<Project[]>([]);
  const [active, setActive] = useState<Project | null>(null);
  const [notes, setNotes] = useState<Note[]>([]);
  const [filterProject, setFilterProject] = useState<string | "active" | "all" | "none">("active");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [showIntelligence, setShowIntelligence] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const [ps, ap, ns] = await Promise.all([
        projectsList(false),
        getActiveProject(),
        notesList(filterProject === "all" ? null
          : filterProject === "active" ? (active?.id ?? null)
          : filterProject === "none" ? null
          : filterProject),
      ]);
      setProjects(ps);
      setActive(ap);
      setNotes(ns);
    } catch (e) { setError(String(e)); }
  };

  useEffect(() => { refresh(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, [filterProject]);
  useEffect(() => { (async () => { setActive(await getActiveProject()); })(); }, []);

  const filteredNotes = useMemo(() => {
    let list = notes;
    if (filterProject === "none") list = list.filter((n) => n.project_id == null);
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter((n) => n.title.toLowerCase().includes(q) || n.body.toLowerCase().includes(q));
    }
    return list;
  }, [notes, search, filterProject]);

  const selected = useMemo(() => notes.find((n) => n.id === selectedId) ?? null, [notes, selectedId]);

  return (
    <div className="p-6 max-w-7xl space-y-6">
      <div className="panel p-5">
        <div className="flex items-start gap-3 flex-wrap">
          <div className="flex-1">
            <div className="h-title">Notes</div>
            <p className="mt-1 text-sm text-slate-400">
              Project-scoped notes. The three most recent notes for the active project are
              prepended to every chat turn as context, so capture facts the model should remember.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <select className="input" value={filterProject} onChange={(e) => setFilterProject(e.target.value as any)}>
              <option value="active">active project{active ? `: ${active.name}` : " (none)"}</option>
              <option value="all">all notes</option>
              <option value="none">unassigned only</option>
              {projects.map((p) => <option key={p.id} value={p.id}>only: {p.name}</option>)}
            </select>
            <input className="input" placeholder="Search…" value={search} onChange={(e) => setSearch(e.target.value)} />
            <button
              className="btn btn-primary"
              onClick={async () => {
                const projectId = filterProject === "all" ? null
                  : filterProject === "active" ? (active?.id ?? null)
                  : filterProject === "none" ? null
                  : filterProject;
                const n = await notesCreate(projectId, "Untitled note", "# New note\n");
                await refresh();
                setSelectedId(n.id);
              }}
            >+ New note</button>
          </div>
        </div>
        {error && <div className="mt-3 text-xs text-rose2-400 font-mono">{error}</div>}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-12 gap-4">
        <aside className="md:col-span-3 panel p-3 max-h-[34rem] overflow-y-auto">
          {filteredNotes.length === 0 && <div className="text-xs text-slate-500 p-2">No notes.</div>}
          <div className="space-y-1">
            {filteredNotes.map((n) => (
              <button
                key={n.id}
                onClick={() => setSelectedId(n.id)}
                className={`w-full text-left rounded-md p-2 ${selectedId === n.id ? "bg-cyan2-500/10 ring-1 ring-cyan2-500/30" : "hover:bg-white/5"}`}
              >
                <div className="text-sm text-slate-200 truncate">{n.title || "Untitled"}</div>
                <div className="text-[10px] font-mono text-slate-500 truncate">
                  {n.project_id ? projects.find((p) => p.id === n.project_id)?.name ?? "project" : "no project"} · {new Date(n.updated_at).toLocaleString()}
                </div>
              </button>
            ))}
          </div>
        </aside>

        <div className="md:col-span-9">
          {showIntelligence && (
            <div className="mb-3">
              <DocumentIntelligence initialText={selected?.body} onClose={() => setShowIntelligence(false)} />
            </div>
          )}
          {selected ? (
            <NoteEditor
              key={selected.id}
              note={selected}
              projects={projects}
              onSave={async (req) => { await notesUpdate(req); await refresh(); }}
              onDelete={async () => {
                const ok = await confirmModal({
                  title: "Delete note",
                  description: `Delete note "${selected.title}"? This cannot be undone.`,
                  submitLabel: "Delete note",
                  destructive: true,
                });
                if (!ok) return;
                await notesDelete(selected.id);
                setSelectedId(null);
                await refresh();
              }}
              onOpenIntelligence={() => setShowIntelligence(true)}
            />
          ) : (
            <div className="panel p-6 text-sm text-slate-500">Pick a note on the left, or create a new one.</div>
          )}
        </div>
      </div>
    </div>
  );
}

function NoteEditor({ note, projects, onSave, onDelete, onOpenIntelligence }: { note: Note; projects: Project[]; onSave: (req: { id: string; title?: string; body?: string }) => Promise<void>; onDelete: () => Promise<void>; onOpenIntelligence: () => void }) {
  const [title, setTitle] = useState(note.title);
  const [body, setBody] = useState(note.body);
  const [tab, setTab] = useState<"edit" | "preview">("edit");
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    setTitle(note.title);
    setBody(note.body);
    setDirty(false);
  }, [note.id]);

  const save = async () => {
    await onSave({ id: note.id, title: title.trim() || "Untitled note", body });
    setDirty(false);
  };

  return (
    <div className="panel p-4 space-y-3">
      <div className="flex items-center gap-2">
        <input
          className="input flex-1 text-base font-medium"
          value={title}
          onChange={(e) => { setTitle(e.target.value); setDirty(true); }}
          placeholder="Note title"
        />
        <div className="flex border border-white/10 rounded-md overflow-hidden">
          <button className={`px-3 py-1.5 text-sm ${tab === "edit" ? "bg-cyan2-500/10 text-cyan2-300" : "text-slate-400"}`} onClick={() => setTab("edit")}>Edit</button>
          <button className={`px-3 py-1.5 text-sm ${tab === "preview" ? "bg-cyan2-500/10 text-cyan2-300" : "text-slate-400"}`} onClick={() => setTab("preview")}>Preview</button>
        </div>
      </div>
      <div className="text-[10px] text-slate-500 font-mono">
        {projects.find((p) => p.id === note.project_id)?.name ?? "no project"} · updated {new Date(note.updated_at).toLocaleString()}
      </div>
      {tab === "edit" ? (
        <textarea
          className="input min-h-[24rem] font-mono text-sm leading-relaxed"
          value={body}
          onChange={(e) => { setBody(e.target.value); setDirty(true); }}
        />
      ) : (
        <div className="panel-soft p-4 min-h-[24rem]">
          {body.trim() ? <MarkdownView text={body} /> : <span className="text-slate-500 text-sm">Empty note.</span>}
        </div>
      )}
      <div className="flex flex-wrap gap-2">
        <button className="btn btn-primary" disabled={!dirty} onClick={save}>Save</button>
        <button className="btn text-rose2-400" onClick={onDelete}>Delete</button>
        {dirty && <span className="text-[11px] text-amber2-400 self-center">unsaved changes</span>}
      </div>
      <div className="mt-3">
        <button className="btn" onClick={onOpenIntelligence}>Open Document Intelligence</button>
      </div>
    </div>
  );
}
