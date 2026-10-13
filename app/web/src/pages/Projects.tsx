// Projects — Phase 5 hub for workspaces, projects, instructions, and goals.
// Each project owns its tasks, notes, and (in the next phase) experiments.
// The "active" project is the one whose context gets prepended to chat.

import { useEffect, useMemo, useState } from "react";
import {
  type Project,
  type ProjectSummary,
  type Workspace,
  getActiveProject,
  projectsArchive,
  projectsCreate,
  projectsDelete,
  projectsSummaries,
  projectsUpdate,
  setActiveProject,
  workspacesCreate,
  workspacesDelete,
  workspacesList,
  workspacesRename,
} from "../lib/projects";
import { usePromptDialog } from "../hooks/usePromptDialog";

export function ProjectsPage() {
  const { confirm: confirmModal } = usePromptDialog();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [summaries, setSummaries] = useState<ProjectSummary[]>([]);
  const [active, setActive] = useState<Project | null>(null);
  const [selectedWsId, setSelectedWsId] = useState<string | "unassigned" | "all">("all");
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showArchived, setShowArchived] = useState(false);

  const refresh = async () => {
    try {
      const [ws, sm, ap] = await Promise.all([
        workspacesList(),
        projectsSummaries(showArchived),
        getActiveProject(),
      ]);
      setWorkspaces(ws);
      setSummaries(sm);
      setActive(ap);
      if (selectedProjectId && !sm.some((s) => s.project.id === selectedProjectId)) {
        setSelectedProjectId(null);
      }
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showArchived]);

  const visibleProjects = useMemo(() => {
    let list = summaries.map((s) => s.project);
    if (selectedWsId === "unassigned") {
      list = list.filter((p) => !p.workspace_id);
    } else if (selectedWsId !== "all") {
      list = list.filter((p) => p.workspace_id === selectedWsId);
    }
    return list;
  }, [summaries, selectedWsId]);

  const selectedSummary = useMemo(
    () => summaries.find((s) => s.project.id === selectedProjectId) ?? null,
    [summaries, selectedProjectId],
  );

  return (
    <div className="p-6 max-w-7xl space-y-6">
      <div className="panel p-5">
        <div className="flex items-start gap-3">
          <div className="flex-1">
            <div className="h-title">Projects</div>
            <p className="mt-1 text-sm text-slate-400">
              Workspaces hold projects; each project owns its tasks, notes, and (later) research
              artifacts. The <span className="text-cyan2-400">active project</span> is prepended
              to every chat turn as project context.
            </p>
          </div>
          <label className="flex items-center gap-2 text-[11px] text-slate-400 cursor-pointer">
            <input
              type="checkbox"
              className="accent-cyan2-500"
              checked={showArchived}
              onChange={(e) => setShowArchived(e.target.checked)}
            />
            show archived
          </label>
        </div>
        {active && (
          <div className="mt-3 panel-soft p-3 text-xs text-cyan2-400 font-mono">
            active project: {active.name}
            <button
              className="ml-3 text-slate-400 hover:text-rose2-400"
              onClick={async () => { await setActiveProject(null); refresh(); }}
            >
              clear
            </button>
          </div>
        )}
        {error && <div className="mt-3 text-xs text-rose2-400 font-mono">{error}</div>}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-12 gap-4">
        {/* Workspaces sidebar */}
        <aside className="md:col-span-3 panel p-4 space-y-3">
          <div className="h-title">Workspaces</div>
          <div className="space-y-1">
            <WsButton id="all" selected={selectedWsId} onClick={setSelectedWsId} label="All projects" count={summaries.length} />
            <WsButton id="unassigned" selected={selectedWsId} onClick={setSelectedWsId} label="Unassigned" count={summaries.filter((s) => !s.project.workspace_id).length} />
            {workspaces.map((w) => (
              <WsButton
                key={w.id}
                id={w.id}
                selected={selectedWsId}
                onClick={setSelectedWsId}
                label={w.name}
                count={summaries.filter((s) => s.project.workspace_id === w.id).length}
              />
            ))}
          </div>
          <NewWorkspaceForm onCreate={async (name, desc) => {
            const w = await workspacesCreate(name, desc);
            await refresh();
            setSelectedWsId(w.id);
          }} />
        </aside>

        {/* Projects list */}
        <div className="md:col-span-4 panel p-4">
          <div className="flex items-center justify-between">
            <div className="h-title">Projects</div>
            <NewProjectButton
              workspaces={workspaces}
              onCreate={async (req) => {
                const p = await projectsCreate(req);
                await refresh();
                setSelectedProjectId(p.id);
              }}
              defaultWorkspaceId={selectedWsId === "all" || selectedWsId === "unassigned" ? null : selectedWsId}
            />
          </div>
          <div className="mt-3 space-y-2 max-h-[28rem] overflow-y-auto pr-1">
            {visibleProjects.length === 0 && (
              <div className="text-xs text-slate-500">No projects. Create one to get started.</div>
            )}
            {visibleProjects.map((p) => {
              const s = summaries.find((x) => x.project.id === p.id)!;
              return (
                <button
                  key={p.id}
                  onClick={() => setSelectedProjectId(p.id)}
                  className={`w-full text-left rounded-lg border p-3 transition ${selectedProjectId === p.id ? "border-cyan2-500/60 bg-cyan2-500/5" : "border-white/5 bg-ink-800/50 hover:bg-white/5"}`}
                >
                  <div className="flex items-center gap-2">
                    <div className="font-medium text-slate-200 truncate flex-1">{p.name}</div>
                    {p.archived && <span className="chip">archived</span>}
                    {active?.id === p.id && <span className="chip text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10">active</span>}
                  </div>
                  {p.description && <div className="mt-1 text-[11px] text-slate-500 line-clamp-2">{p.description}</div>}
                  <div className="mt-1 text-[10px] font-mono text-slate-500">
                    {s.open_task_count} open task{s.open_task_count === 1 ? "" : "s"} · {s.note_count} note{s.note_count === 1 ? "" : "s"}
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        {/* Project detail */}
        <div className="md:col-span-5 panel p-4">
          {selectedSummary ? (
            <ProjectEditor
              key={selectedSummary.project.id}
              summary={selectedSummary}
              workspaces={workspaces}
              isActive={active?.id === selectedSummary.project.id}
              onSave={async (req) => {
                await projectsUpdate(req);
                await refresh();
              }}
              onArchive={async (archived) => {
                await projectsArchive(selectedSummary.project.id, archived);
                await refresh();
              }}
              onDelete={async () => {
                const ok = await confirmModal({
                  title: "Delete project",
                  description: `Delete project "${selectedSummary.project.name}"? Conversations stay but are detached.`,
                  submitLabel: "Delete project",
                  destructive: true,
                });
                if (!ok) return;
                await projectsDelete(selectedSummary.project.id);
                if (active?.id === selectedSummary.project.id) await setActiveProject(null);
                setSelectedProjectId(null);
                await refresh();
              }}
              onActivate={async () => {
                if (active?.id === selectedSummary.project.id) {
                  await setActiveProject(null);
                } else {
                  await setActiveProject(selectedSummary.project.id);
                }
                await refresh();
              }}
            />
          ) : (
            <div className="text-sm text-slate-500">Select a project to edit its details, or create a new one.</div>
          )}
        </div>
      </div>

      {selectedWsId !== "all" && selectedWsId !== "unassigned" && (() => {
        const w = workspaces.find((x) => x.id === selectedWsId);
        if (!w) return null;
        return (
          <WorkspaceDetail
            workspace={w}
            onRename={async (name) => { await workspacesRename(w.id, name); await refresh(); }}
            onDelete={async () => {
              const ok = await confirmModal({
                title: "Delete workspace",
                description: `Delete workspace "${w.name}"? Projects will become unassigned.`,
                submitLabel: "Delete workspace",
                destructive: true,
              });
              if (!ok) return;
              await workspacesDelete(w.id);
              setSelectedWsId("all");
              await refresh();
            }}
          />
        );
      })()}
    </div>
  );
}

function WsButton({ id, selected, onClick, label, count }: { id: string; selected: string; onClick: (id: any) => void; label: string; count: number }) {
  const active = id === selected;
  return (
    <button
      onClick={() => onClick(id)}
      className={`w-full text-left px-2 py-1.5 rounded-md text-sm flex items-center gap-2 ${active ? "bg-cyan2-500/10 text-cyan2-200" : "text-slate-300 hover:bg-white/5"}`}
    >
      <span className="truncate flex-1">{label}</span>
      <span className="text-[10px] text-slate-500 font-mono">{count}</span>
    </button>
  );
}

function NewWorkspaceForm({ onCreate }: { onCreate: (name: string, desc: string | null) => Promise<void> }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [desc, setDesc] = useState("");
  return (
    <div className="border-t border-white/5 pt-3">
      {!open && <button className="btn w-full" onClick={() => setOpen(true)}>+ New workspace</button>}
      {open && (
        <div className="space-y-2">
          <input className="input" placeholder="Workspace name" value={name} onChange={(e) => setName(e.target.value)} />
          <input className="input" placeholder="Description (optional)" value={desc} onChange={(e) => setDesc(e.target.value)} />
          <div className="flex gap-2">
            <button className="btn btn-primary flex-1" onClick={async () => { if (!name.trim()) return; await onCreate(name.trim(), desc.trim() || null); setName(""); setDesc(""); setOpen(false); }}>Create</button>
            <button className="btn" onClick={() => setOpen(false)}>Cancel</button>
          </div>
        </div>
      )}
    </div>
  );
}

function NewProjectButton({ workspaces, onCreate, defaultWorkspaceId }: { workspaces: Workspace[]; onCreate: (req: { name: string; workspace_id: string | null; description: string | null; instructions: string | null; goals: string[] }) => Promise<void>; defaultWorkspaceId: string | null }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [workspaceId, setWorkspaceId] = useState<string | null>(defaultWorkspaceId);
  const [description, setDescription] = useState("");
  const [instructions, setInstructions] = useState("");
  const [goals, setGoals] = useState("");
  return (
    <>
      <button className="btn" onClick={() => setOpen(true)}>+ New project</button>
      {open && (
        <div className="fixed inset-0 z-30 bg-black/60 flex items-center justify-center p-4" onClick={() => setOpen(false)}>
          <div className="panel p-5 w-full max-w-xl space-y-3" onClick={(e) => e.stopPropagation()}>
            <div className="h-title">New project</div>
            <Field label="Name">
              <input className="input" autoFocus value={name} onChange={(e) => setName(e.target.value)} />
            </Field>
            <Field label="Workspace">
              <select className="input" value={workspaceId ?? ""} onChange={(e) => setWorkspaceId(e.target.value || null)}>
                <option value="">(unassigned)</option>
                {workspaces.map((w) => <option key={w.id} value={w.id}>{w.name}</option>)}
              </select>
            </Field>
            <Field label="Description">
              <textarea className="input min-h-[3rem]" value={description} onChange={(e) => setDescription(e.target.value)} />
            </Field>
            <Field label="Instructions (honoured as firm rules in chat)">
              <textarea className="input min-h-[4rem]" placeholder="e.g. Always answer in formal English; cite sources; never invent file paths." value={instructions} onChange={(e) => setInstructions(e.target.value)} />
            </Field>
            <Field label="Goals (one per line)">
              <textarea className="input min-h-[3rem]" value={goals} onChange={(e) => setGoals(e.target.value)} />
            </Field>
            <div className="flex justify-end gap-2 pt-2">
              <button className="btn" onClick={() => setOpen(false)}>Cancel</button>
              <button
                className="btn btn-primary"
                onClick={async () => {
                  if (!name.trim()) return;
                  const goalLines = goals.split("\n").map((g) => g.trim()).filter(Boolean);
                  await onCreate({
                    name: name.trim(),
                    workspace_id: workspaceId,
                    description: description.trim() || null,
                    instructions: instructions.trim() || null,
                    goals: goalLines,
                  });
                  setOpen(false);
                  setName(""); setDescription(""); setInstructions(""); setGoals("");
                }}
              >Create</button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}

function ProjectEditor({ summary, workspaces, isActive, onSave, onArchive, onDelete, onActivate }: {
  summary: ProjectSummary;
  workspaces: Workspace[];
  isActive: boolean;
  onSave: (req: { id: string; name?: string | null; description?: string | null; instructions?: string | null; goals?: string[] | null }) => Promise<void>;
  onArchive: (archived: boolean) => Promise<void>;
  onDelete: () => Promise<void>;
  onActivate: () => Promise<void>;
}) {
  const [name, setName] = useState(summary.project.name);
  const [workspaceId, setWorkspaceId] = useState<string | null>(summary.project.workspace_id);
  const [description, setDescription] = useState(summary.project.description ?? "");
  const [instructions, setInstructions] = useState(summary.project.instructions ?? "");
  const [goals, setGoals] = useState(summary.project.goals.join("\n"));
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    setName(summary.project.name);
    setWorkspaceId(summary.project.workspace_id);
    setDescription(summary.project.description ?? "");
    setInstructions(summary.project.instructions ?? "");
    setGoals(summary.project.goals.join("\n"));
    setDirty(false);
  }, [summary.project.id]);

  const onSubmit = async () => {
    setSaving(true);
    try {
      await onSave({
        id: summary.project.id,
        name: name.trim() || summary.project.name,
        description: description.trim() || null,
        instructions: instructions.trim() || null,
        goals: goals.split("\n").map((g) => g.trim()).filter(Boolean),
      });
      setDirty(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <div className="h-title flex-1">Edit project</div>
        <button
          className={isActive ? "btn" : "btn btn-primary"}
          onClick={onActivate}
          title={isActive ? "Click to deactivate" : "Use as the active project for chat"}
        >
          {isActive ? "Deactivate" : "Set active"}
        </button>
      </div>
      <Field label="Name">
        <input className="input" value={name} onChange={(e) => { setName(e.target.value); setDirty(true); }} />
      </Field>
      <Field label="Workspace">
        <select className="input" value={workspaceId ?? ""} onChange={(e) => { setWorkspaceId(e.target.value || null); setDirty(true); }}>
          <option value="">(unassigned)</option>
          {workspaces.map((w) => <option key={w.id} value={w.id}>{w.name}</option>)}
        </select>
      </Field>
      <Field label="Description">
        <textarea className="input min-h-[3rem]" value={description} onChange={(e) => { setDescription(e.target.value); setDirty(true); }} />
      </Field>
      <Field label="Instructions">
        <textarea className="input min-h-[4rem]" value={instructions} onChange={(e) => { setInstructions(e.target.value); setDirty(true); }} />
      </Field>
      <Field label="Goals (one per line)">
        <textarea className="input min-h-[3rem]" value={goals} onChange={(e) => { setGoals(e.target.value); setDirty(true); }} />
      </Field>
      <div className="text-[11px] text-slate-500 font-mono">
        {summary.open_task_count} open task{summary.open_task_count === 1 ? "" : "s"} · {summary.note_count} note{summary.note_count === 1 ? "" : "s"} · {summary.task_count} task{summary.task_count === 1 ? "" : "s"} total
      </div>
      <div className="flex flex-wrap gap-2 pt-2">
        <button className="btn btn-primary" disabled={!dirty || saving} onClick={onSubmit}>
          {saving ? "Saving…" : "Save"}
        </button>
        <button className="btn" onClick={() => onArchive(!summary.project.archived)}>
          {summary.project.archived ? "Unarchive" : "Archive"}
        </button>
        <button className="btn text-rose2-400" onClick={onDelete}>Delete</button>
      </div>
    </div>
  );
}

function WorkspaceDetail({ workspace, onRename, onDelete }: { workspace: Workspace; onRename: (name: string) => Promise<void>; onDelete: () => Promise<void> }) {
  const { prompt: promptModal } = usePromptDialog();
  return (
    <>
    <div className="panel p-4">
      <div className="h-title">Workspace · {workspace.name}</div>
      <div className="mt-3 flex flex-wrap gap-2">
        <button
          className="btn"
          onClick={async () => {
            const name = await promptModal({
              title: "Rename workspace",
              label: "Name",
              defaultValue: workspace.name,
              submitLabel: "Rename",
              validate: (v) => (v.trim() ? "" : "Name is required"),
            });
            if (name && name.trim()) await onRename(name.trim());
          }}
        >Rename</button>
        <button className="btn text-rose2-400" onClick={onDelete}>Delete workspace</button>
      </div>
    </div>
    </>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1">
      <div className="h-sub">{label}</div>
      {children}
    </label>
  );
}
