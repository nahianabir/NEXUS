// Typed IPC bindings for projects, notes, tasks, memory, and ui_state.
// Phase 5.

import { invoke } from "@tauri-apps/api/core";

// ---------------- workspaces ----------------

export type Workspace = {
  id: string;
  name: string;
  description: string | null;
  created_at: number;
};

export async function workspacesList(): Promise<Workspace[]> {
  return invoke("workspaces_list");
}
export async function workspacesCreate(name: string, description?: string | null): Promise<Workspace> {
  return invoke("workspaces_create", { name, description: description ?? null });
}
export async function workspacesRename(id: string, name: string): Promise<void> {
  return invoke("workspaces_rename", { id, name });
}
export async function workspacesDelete(id: string): Promise<void> {
  return invoke("workspaces_delete", { id });
}

// ---------------- projects -------------------

export type Project = {
  id: string;
  workspace_id: string | null;
  name: string;
  description: string | null;
  instructions: string | null;
  goals: string[];
  archived: boolean;
  created_at: number;
  updated_at: number;
};

export type ProjectSummary = {
  project: Project;
  task_count: number;
  open_task_count: number;
  note_count: number;
};

export async function projectsList(includeArchived = false): Promise<Project[]> {
  return invoke("projects_list", { includeArchived });
}
export async function projectsSummaries(includeArchived = false): Promise<ProjectSummary[]> {
  return invoke("projects_summaries", { includeArchived });
}
export async function projectsGet(id: string): Promise<Project> {
  return invoke("projects_get", { id });
}

export type CreateProjectRequest = {
  workspace_id?: string | null;
  name: string;
  description?: string | null;
  instructions?: string | null;
  goals?: string[] | null;
};

export async function projectsCreate(request: CreateProjectRequest): Promise<Project> {
  return invoke("projects_create", { request });
}

export type UpdateProjectRequest = {
  id: string;
  name?: string | null;
  description?: string | null;
  instructions?: string | null;
  goals?: string[] | null;
};

export async function projectsUpdate(request: UpdateProjectRequest): Promise<Project> {
  return invoke("projects_update", { request });
}

export async function projectsArchive(id: string, archived: boolean): Promise<void> {
  return invoke("projects_archive", { id, archived });
}
export async function projectsDelete(id: string): Promise<void> {
  return invoke("projects_delete", { id });
}

export async function getActiveProject(): Promise<Project | null> {
  return invoke("get_active_project");
}
export async function setActiveProject(id: string | null): Promise<void> {
  return invoke("set_active_project", { id });
}

// ---------------- notes ----------------------

export type Note = {
  id: string;
  project_id: string | null;
  title: string;
  body: string;
  created_at: number;
  updated_at: number;
};

export async function notesList(projectId?: string | null): Promise<Note[]> {
  return invoke("notes_list", { projectId: projectId ?? null });
}
export async function notesGet(id: string): Promise<Note> {
  return invoke("notes_get", { id });
}
export async function notesCreate(projectId: string | null, title: string, body: string): Promise<Note> {
  return invoke("notes_create", { request: { project_id: projectId, title, body } });
}
export type UpdateNoteRequest = { id: string; title?: string | null; body?: string | null };
export async function notesUpdate(request: UpdateNoteRequest): Promise<Note> {
  return invoke("notes_update", { request });
}
export async function notesDelete(id: string): Promise<void> {
  return invoke("notes_delete", { id });
}

// ---------------- tasks ----------------------

export type TaskStatus = "open" | "in_progress" | "done" | "cancelled";
export type TaskPriority = "low" | "normal" | "high" | "urgent";

export type ChecklistItem = { text: string; done: boolean };

export type Task = {
  id: string;
  project_id: string | null;
  title: string;
  description: string | null;
  status: string;
  priority: string;
  due_at: number | null;
  checklist: ChecklistItem[];
  notes: string | null;
  created_at: number;
  updated_at: number;
  completed_at: number | null;
};

export async function tasksList(
  projectId?: string | null,
  status?: TaskStatus | null,
  includeDone = true,
): Promise<Task[]> {
  return invoke("tasks_list", { projectId: projectId ?? null, status: status ?? null, includeDone });
}
export async function tasksDueInRange(fromMs: number, toMs: number): Promise<Task[]> {
  return invoke("tasks_due_in_range", { fromMs, toMs });
}
export async function tasksGet(id: string): Promise<Task> {
  return invoke("tasks_get", { id });
}
export type CreateTaskRequest = {
  project_id?: string | null;
  title: string;
  description?: string | null;
  priority: TaskPriority;
  due_at?: number | null;
  checklist?: ChecklistItem[] | null;
};
export async function tasksCreate(request: CreateTaskRequest): Promise<Task> {
  return invoke("tasks_create", { request });
}
export type UpdateTaskRequest = {
  id: string;
  title?: string | null;
  description?: string | null;
  priority?: TaskPriority | null;
  /** Tri-state: `null` = leave alone, `{ value: null }` = clear, `{ value: ms }` = set. */
  due_at?: { value: number | null } | null;
  checklist?: ChecklistItem[] | null;
  notes?: string | null;
};
export async function tasksUpdate(request: UpdateTaskRequest): Promise<Task> {
  return invoke("tasks_update", { request });
}
export async function tasksSetStatus(id: string, status: TaskStatus): Promise<Task> {
  return invoke("tasks_set_status", { id, status });
}
export async function tasksDelete(id: string): Promise<void> {
  return invoke("tasks_delete", { id });
}

// ---------------- memory ---------------------

export type MemoryScope = "user" | "project" | "auto" | "conv";

export type Memory = {
  id: string;
  scope: string;
  project_id: string | null;
  conversation_id: string | null;
  content: string;
  source: string | null;
  approved: boolean;
  created_at: number;
  updated_at: number;
};

export async function memoryList(scope?: MemoryScope | null, approvedOnly = false): Promise<Memory[]> {
  return invoke("memory_list", { scope: scope ?? null, approvedOnly });
}
export type CreateMemoryRequest = {
  scope: MemoryScope;
  project_id?: string | null;
  conversation_id?: string | null;
  content: string;
  source?: string | null;
  approved?: boolean | null;
};
export async function memoryCreate(request: CreateMemoryRequest): Promise<Memory> {
  return invoke("memory_create", { request });
}
export type UpdateMemoryRequest = {
  id: string;
  scope?: MemoryScope | null;
  content?: string | null;
  /** Tri-state: `null` = leave alone, `{ value: null }` = clear, `{ value: id }` = set. */
  project_id?: { value: string | null } | null;
  approved?: boolean | null;
};
export async function memoryUpdate(request: UpdateMemoryRequest): Promise<Memory> {
  return invoke("memory_update", { request });
}
export async function memoryApprove(id: string): Promise<Memory> {
  return invoke("memory_approve", { id });
}
export async function memoryDelete(id: string): Promise<void> {
  return invoke("memory_delete", { id });
}

// ---------------- ui_state ------------------

export async function uiStateGet(key: string): Promise<unknown> {
  return invoke("ui_state_get", { key });
}
export async function uiStateSet(key: string, value: unknown): Promise<void> {
  return invoke("ui_state_set", { key, value });
}
export async function uiStateUnset(key: string): Promise<void> {
  return invoke("ui_state_unset", { key });
}
