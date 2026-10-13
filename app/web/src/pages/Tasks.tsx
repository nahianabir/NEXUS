// Tasks — Phase 5.
//
// Two views: Board (kanban-style grouped by status) and Calendar (a 14-day
// strip of upcoming due dates). The user picks the active project from the
// header; the view scopes to that project. Tasks inherit the active
// project unless the user clears it via the project filter.

import { useEffect, useMemo, useState } from "react";
import {
  type ChecklistItem,
  type Project,
  type Task,
  type TaskPriority,
  type TaskStatus,
  getActiveProject,
  projectsList,
  tasksCreate,
  tasksDelete,
  tasksDueInRange,
  tasksList,
  tasksSetStatus,
  tasksUpdate,
} from "../lib/projects";
import { checkpointsCreate } from "../lib/checkpoints";
import { usePromptDialog } from "../hooks/usePromptDialog";

type View = "board" | "calendar";

export function TasksPage() {
  const { prompt: promptModal, confirm: confirmModal, alert: alertModal } = usePromptDialog();
  const [projects, setProjects] = useState<Project[]>([]);
  const [active, setActive] = useState<Project | null>(null);
  const [filterProject, setFilterProject] = useState<string | "active" | "all">("active");
  const [tasks, setTasks] = useState<Task[]>([]);
  const [includeDone, setIncludeDone] = useState(true);
  const [view, setView] = useState<View>("board");
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<Task | null>(null);
  const [creating, setCreating] = useState(false);

  const refresh = async () => {
    try {
      const [ps, ap] = await Promise.all([projectsList(false), getActiveProject()]);
      setProjects(ps);
      setActive(ap);
    } catch (e) { setError(String(e)); return; }
    const projectId = filterProject === "all" ? null : (filterProject === "active" ? active?.id ?? null : filterProject);
    if (view === "calendar") {
      const now = Date.now();
      const to = now + 14 * 86_400_000;
      try { setTasks(await tasksDueInRange(now - 86_400_000, to)); }
      catch (e) { setError(String(e)); }
      return;
    }
    try {
      const list = await tasksList(projectId, null, includeDone);
      setTasks(list);
    } catch (e) { setError(String(e)); }
  };

  useEffect(() => { refresh(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, [filterProject, view, includeDone]);
  useEffect(() => { (async () => { setActive(await getActiveProject()); })(); }, []);

  const grouped = useMemo(() => {
    const out: Record<TaskStatus, Task[]> = { open: [], in_progress: [], done: [], cancelled: [] };
    for (const t of tasks) {
      const k = t.status as TaskStatus;
      if (out[k]) out[k].push(t);
    }
    return out;
  }, [tasks]);

  return (
    <div className="p-6 max-w-7xl space-y-6">
      <div className="panel p-5">
        <div className="flex items-start gap-3 flex-wrap">
          <div className="flex-1">
            <div className="h-title">Tasks &amp; Calendar</div>
            <p className="mt-1 text-sm text-slate-400">
              Tasks can belong to a project or live as ad-hoc items. Drag is not required — set the
              status, due date, priority, and checklist inline.
            </p>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            <select className="input" value={filterProject} onChange={(e) => setFilterProject(e.target.value as any)}>
              <option value="active">active project{active ? `: ${active.name}` : " (none)"}</option>
              <option value="all">all projects</option>
              {projects.map((p) => <option key={p.id} value={p.id}>only: {p.name}</option>)}
            </select>
            <label className="flex items-center gap-1 text-[11px] text-slate-400">
              <input type="checkbox" className="accent-cyan2-500" checked={includeDone} onChange={(e) => setIncludeDone(e.target.checked)} />
              done
            </label>
            <div className="flex border border-white/10 rounded-md overflow-hidden">
              <button className={`px-3 py-1.5 text-sm ${view === "board" ? "bg-cyan2-500/10 text-cyan2-300" : "text-slate-400"}`} onClick={() => setView("board")}>Board</button>
              <button className={`px-3 py-1.5 text-sm ${view === "calendar" ? "bg-cyan2-500/10 text-cyan2-300" : "text-slate-400"}`} onClick={() => setView("calendar")}>Calendar</button>
            </div>
            <button className="btn btn-primary" onClick={() => setCreating(true)}>+ New task</button>
          </div>
        </div>
        {error && <div className="mt-3 text-xs text-rose2-400 font-mono">{error}</div>}
      </div>

      {view === "board" ? (
        <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
          {(["open", "in_progress", "done", "cancelled"] as TaskStatus[]).map((s) => (
            <div key={s} className="panel p-3">
              <div className="flex items-center justify-between mb-2">
                <div className="h-title capitalize">{s.replace("_", " ")}</div>
                <span className="text-[10px] text-slate-500 font-mono">{grouped[s].length}</span>
              </div>
              <div className="space-y-2 max-h-[34rem] overflow-y-auto pr-1">
                {grouped[s].length === 0 && <div className="text-[11px] text-slate-500">No tasks.</div>}
                {grouped[s].map((t) => (
                  <TaskCard
                    key={t.id}
                    task={t}
                    projectName={projects.find((p) => p.id === t.project_id)?.name ?? null}
                    onClick={() => setEditing(t)}
                    onSetStatus={async (status) => {
                      await tasksSetStatus(t.id, status);
                      await refresh();
                    }}
                    onCheckpoint={async (task) => {
                      if (!task.project_id) return;
                      const summary = await promptModal({
                        title: "Create checkpoint",
                        description: "Captures the current workspace state for this task. Leave the title as the task title or override it.",
                        label: "Checkpoint title",
                        defaultValue: task.title,
                        submitLabel: "Create checkpoint",
                      });
                      if (summary === null) return;
                      try {
                        await checkpointsCreate({
                          project_id: task.project_id,
                          task_id: task.id,
                          title: summary.trim() || task.title,
                          goal: task.description ?? null,
                          completed_work: task.checklist.length > 0
                            ? task.checklist.filter((c) => c.done).map((c) => c.text).join("\n")
                            : null,
                          current_state: task.notes ?? null,
                          next_action_text: null,
                          observed_files: [],
                          observed_tests: [],
                          errors: [],
                          decisions: [],
                          constraints: [],
                          pending_actions: task.checklist.filter((c) => !c.done).map((c) => ({ text: c.text, done: false })),
                          evidence_refs: [{ kind: "task", id: task.id }],
                          created_by: "user",
                          model_hint: null,
                        });
                        await refresh();
                      } catch (e) {
                        await alertModal("Checkpoint failed", `Could not save checkpoint: ${e}`);
                      }
                    }}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <Calendar tasks={tasks} onClick={setEditing} />
      )}

      {editing && (
        <TaskEditorModal
          task={editing}
          projects={projects}
          onClose={() => setEditing(null)}
          onSave={async (req) => {
            await tasksUpdate(req);
            setEditing(null);
            await refresh();
          }}
          onDelete={async () => {
            const ok = await confirmModal({
              title: "Delete task",
              description: `Delete task "${editing.title}"? This cannot be undone.`,
              submitLabel: "Delete task",
              destructive: true,
            });
            if (!ok) return;
            await tasksDelete(editing.id);
            setEditing(null);
            await refresh();
          }}
        />
      )}

      {creating && (
        <CreateTaskModal
          projects={projects}
          defaultProjectId={
            filterProject === "active" ? active?.id ?? null
            : filterProject === "all" ? null
            : filterProject
          }
          onClose={() => setCreating(false)}
          onCreate={async (req) => {
            await tasksCreate(req);
            setCreating(false);
            await refresh();
          }}
        />
      )}
    </div>
  );
}

function TaskCard({ task, projectName, onClick, onSetStatus, onCheckpoint }: {
  task: Task;
  projectName: string | null;
  onClick: () => void;
  onSetStatus: (s: TaskStatus) => Promise<void>;
  onCheckpoint?: (t: Task) => Promise<void> | void;
}) {
  return (
    <div className="rounded-lg border border-white/5 bg-ink-800/50 p-2.5 hover:border-white/10 cursor-pointer" onClick={onClick}>
      <div className="flex items-center gap-2">
        <div className="text-sm text-slate-200 flex-1 line-clamp-2">{task.title}</div>
        <PriorityChip priority={task.priority as TaskPriority} />
      </div>
      {projectName && <div className="mt-1 text-[10px] text-slate-500 font-mono truncate">{projectName}</div>}
      <div className="mt-1 flex items-center gap-1.5 text-[10px] text-slate-500 font-mono">
        {task.due_at && <span className="text-cyan2-400">due {formatDate(task.due_at)}</span>}
        {task.checklist.length > 0 && <span>· {task.checklist.filter((c) => c.done).length}/{task.checklist.length} checklist</span>}
      </div>
      <div className="mt-1.5 flex flex-wrap gap-1" onClick={(e) => e.stopPropagation()}>
        {task.status !== "done" && <button className="text-[10px] text-cyan2-400 hover:underline" onClick={() => onSetStatus("done")}>✓ done</button>}
        {task.status === "open" && <button className="text-[10px] text-amber2-400 hover:underline" onClick={() => onSetStatus("in_progress")}>→ in_progress</button>}
        {task.status !== "open" && task.status !== "cancelled" && <button className="text-[10px] text-slate-400 hover:underline" onClick={() => onSetStatus("open")}>↺ reopen</button>}
        {task.status !== "cancelled" && <button className="text-[10px] text-slate-500 hover:underline" onClick={() => onSetStatus("cancelled")}>× cancel</button>}
        {onCheckpoint && task.project_id && (
          <button
            className="text-[10px] text-cyan2-400 hover:underline"
            title="Save a checkpoint for this task"
            onClick={() => onCheckpoint(task)}
          >＋ checkpoint</button>
        )}
      </div>
    </div>
  );
}

function PriorityChip({ priority }: { priority: TaskPriority }) {
  const cls = priority === "urgent"
    ? "text-rose2-400 border-rose2-400/40 bg-rose2-400/10"
    : priority === "high"
    ? "text-amber2-400 border-amber2-400/40 bg-amber2-400/10"
    : "text-slate-400";
  return <span className={`chip ${cls}`}>{priority}</span>;
}

function Calendar({ tasks, onClick }: { tasks: Task[]; onClick: (t: Task) => void }) {
  const days: { date: number; label: string; isToday: boolean; items: Task[] }[] = [];
  const now = new Date();
  for (let i = -1; i < 14; i++) {
    const d = new Date(now);
    d.setHours(0, 0, 0, 0);
    d.setDate(d.getDate() + i);
    const start = d.getTime();
    const end = start + 86_400_000;
    const items = tasks.filter((t) => t.due_at != null && t.due_at >= start && t.due_at < end);
    days.push({
      date: start,
      label: d.toLocaleDateString(undefined, { weekday: "short", month: "numeric", day: "numeric" }),
      isToday: i === 0,
      items,
    });
  }
  return (
    <div className="panel p-4">
      <div className="grid grid-cols-7 gap-2">
        {days.map((d) => (
          <div key={d.date} className={`rounded-lg border ${d.isToday ? "border-cyan2-500/40 bg-cyan2-500/5" : "border-white/5 bg-ink-800/40"} p-2 min-h-[10rem]`}>
            <div className="text-[11px] font-medium text-slate-300">{d.label}</div>
            <div className="mt-1 space-y-1">
              {d.items.length === 0 && <div className="text-[10px] text-slate-500">—</div>}
              {d.items.map((t) => (
                <button key={t.id} onClick={() => onClick(t)} className="w-full text-left rounded border border-white/5 bg-ink-900/50 p-1.5 text-[11px] text-slate-200 hover:bg-white/5">
                  <div className="truncate">{t.title}</div>
                  <PriorityChip priority={t.priority as TaskPriority} />
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function TaskEditorModal({ task, projects, onClose, onSave, onDelete }: {
  task: Task;
  projects: Project[];
  onClose: () => void;
  onSave: (req: { id: string; title?: string | null; description?: string | null; priority?: TaskPriority | null; due_at?: { value: number | null } | null; checklist?: ChecklistItem[] | null; notes?: string | null }) => Promise<void>;
  onDelete: () => Promise<void>;
}) {
  const [title, setTitle] = useState(task.title);
  const [description, setDescription] = useState(task.description ?? "");
  const [projectId, setProjectId] = useState<string | null>(task.project_id);
  const [priority, setPriority] = useState<TaskPriority>(task.priority as TaskPriority);
  const [dueAt, setDueAt] = useState<string>(task.due_at ? toLocalInput(task.due_at) : "");
  const [checklist, setChecklist] = useState<ChecklistItem[]>(task.checklist);
  const [notes, setNotes] = useState(task.notes ?? "");

  const save = async () => {
    const dueMs = dueAt ? new Date(dueAt).getTime() : null;
    await onSave({
      id: task.id,
      title: title.trim() || task.title,
      description: description.trim() || null,
      priority,
      due_at: dueAt ? { value: isNaN(dueMs!) ? null : dueMs } : { value: null },
      checklist,
      notes: notes.trim() || null,
    });
    // suppress unused warning
    void projectId;
  };

  return (
    <div className="fixed inset-0 z-30 bg-black/60 flex items-center justify-center p-4" onClick={onClose}>
      <div className="panel p-5 w-full max-w-2xl space-y-3" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center gap-2">
          <div className="h-title flex-1">Edit task</div>
          <span className="chip">{task.status}</span>
        </div>
        <Field label="Title">
          <input className="input" autoFocus value={title} onChange={(e) => setTitle(e.target.value)} />
        </Field>
        <Field label="Description">
          <textarea className="input min-h-[3rem]" value={description} onChange={(e) => setDescription(e.target.value)} />
        </Field>
        <div className="grid grid-cols-2 gap-3">
          <Field label="Project">
            <select className="input" value={projectId ?? ""} onChange={(e) => setProjectId(e.target.value || null)}>
              <option value="">(none)</option>
              {projects.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </Field>
          <Field label="Priority">
            <select className="input" value={priority} onChange={(e) => setPriority(e.target.value as TaskPriority)}>
              <option value="low">low</option>
              <option value="normal">normal</option>
              <option value="high">high</option>
              <option value="urgent">urgent</option>
            </select>
          </Field>
          <Field label="Due date">
            <input className="input" type="datetime-local" value={dueAt} onChange={(e) => setDueAt(e.target.value)} />
          </Field>
        </div>
        <Field label="Checklist">
          <ChecklistEditor items={checklist} onChange={setChecklist} />
        </Field>
        <Field label="Notes">
          <textarea className="input min-h-[3rem]" value={notes} onChange={(e) => setNotes(e.target.value)} />
        </Field>
        <div className="flex justify-between gap-2 pt-2">
          <button className="btn text-rose2-400" onClick={onDelete}>Delete</button>
          <div className="flex gap-2">
            <button className="btn" onClick={onClose}>Cancel</button>
            <button className="btn btn-primary" onClick={save}>Save</button>
          </div>
        </div>
      </div>
    </div>
  );
}

function CreateTaskModal({ projects, defaultProjectId, onClose, onCreate }: {
  projects: Project[];
  defaultProjectId: string | null;
  onClose: () => void;
  onCreate: (req: { project_id: string | null; title: string; description: string | null; priority: TaskPriority; due_at: number | null; checklist: ChecklistItem[] }) => Promise<void>;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [projectId, setProjectId] = useState<string | null>(defaultProjectId);
  const [priority, setPriority] = useState<TaskPriority>("normal");
  const [dueAt, setDueAt] = useState("");

  return (
    <div className="fixed inset-0 z-30 bg-black/60 flex items-center justify-center p-4" onClick={onClose}>
      <div className="panel p-5 w-full max-w-xl space-y-3" onClick={(e) => e.stopPropagation()}>
        <div className="h-title">New task</div>
        <Field label="Title">
          <input className="input" autoFocus value={title} onChange={(e) => setTitle(e.target.value)} />
        </Field>
        <Field label="Description">
          <textarea className="input min-h-[3rem]" value={description} onChange={(e) => setDescription(e.target.value)} />
        </Field>
        <div className="grid grid-cols-3 gap-3">
          <Field label="Project">
            <select className="input" value={projectId ?? ""} onChange={(e) => setProjectId(e.target.value || null)}>
              <option value="">(none)</option>
              {projects.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
            </select>
          </Field>
          <Field label="Priority">
            <select className="input" value={priority} onChange={(e) => setPriority(e.target.value as TaskPriority)}>
              <option value="low">low</option>
              <option value="normal">normal</option>
              <option value="high">high</option>
              <option value="urgent">urgent</option>
            </select>
          </Field>
          <Field label="Due">
            <input className="input" type="datetime-local" value={dueAt} onChange={(e) => setDueAt(e.target.value)} />
          </Field>
        </div>
        <div className="flex justify-end gap-2 pt-2">
          <button className="btn" onClick={onClose}>Cancel</button>
          <button
            className="btn btn-primary"
            disabled={!title.trim()}
            onClick={async () => {
              const dueMs = dueAt ? new Date(dueAt).getTime() : null;
              await onCreate({
                project_id: projectId,
                title: title.trim(),
                description: description.trim() || null,
                priority,
                due_at: isNaN(dueMs!) ? null : dueMs,
                checklist: [],
              });
            }}
          >Create</button>
        </div>
      </div>
    </div>
  );
}

function ChecklistEditor({ items, onChange }: { items: ChecklistItem[]; onChange: (next: ChecklistItem[]) => void }) {
  const [text, setText] = useState("");
  return (
    <div className="space-y-1.5">
      {items.length === 0 && <div className="text-[11px] text-slate-500">No items yet.</div>}
      {items.map((c, i) => (
        <label key={i} className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            className="accent-cyan2-500"
            checked={c.done}
            onChange={() => onChange(items.map((x, j) => j === i ? { ...x, done: !x.done } : x))}
          />
          <span className={`flex-1 ${c.done ? "line-through text-slate-500" : "text-slate-200"}`}>{c.text}</span>
          <button className="text-[10px] text-rose2-400 hover:underline" onClick={() => onChange(items.filter((_, j) => j !== i))}>remove</button>
        </label>
      ))}
      <div className="flex gap-2 pt-1">
        <input className="input" placeholder="Add an item…" value={text} onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && text.trim()) {
              e.preventDefault();
              onChange([...items, { text: text.trim(), done: false }]);
              setText("");
            }
          }} />
        <button className="btn" onClick={() => { if (text.trim()) { onChange([...items, { text: text.trim(), done: false }]); setText(""); } }}>Add</button>
      </div>
    </div>
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

function toLocalInput(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
function formatDate(ms: number): string {
  return new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
