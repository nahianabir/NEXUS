// Thin typed wrapper over `@tauri-apps/api` invoke. Phase 1 only defines the
// commands that the layout actually calls; later phases add the rest.

import { invoke } from "@tauri-apps/api/core";
import type { Project } from "./projects";

export type { Project } from "./projects";

export type SidebarItem = {
  id: string;
  label: string;
  icon: string;
  group: string;
  route: string;
};

export type AppInfo = {
  name: string;
  version: string;
  data_dir: string;
  build_target: string;
  schema_version: number;
};

export type SystemSample = {
  /** System-wide global CPU usage, 0.0..=100.0. -1.0 when not yet available. */
  cpu_percent: number;
  /** True once the CPU-time baseline has been primed and the value is trustworthy. */
  cpu_available?: boolean;
  /** Human-readable description of the measurement basis. */
  cpu_basis?: string;
  mem_used_mb: number;
  mem_total_mb: number;
  proc_mem_mb: number;
  collected_at: number;
};

export type DashboardSummary = {
  app: AppInfo;
  active_model: string | null;
  active_project: Project | null;
  queue_depth: number;
  documents: number;
  notes: number;
  conversations: number;
  tasks_open: number;
  projects_total: number;
  sample: SystemSample;
  sidebar: SidebarItem[];
};

export async function appInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
export async function listSidebar(): Promise<SidebarItem[]> {
  return invoke<SidebarItem[]>("list_sidebar");
}
export async function dashboardSummary(): Promise<DashboardSummary> {
  return invoke<DashboardSummary>("dashboard_summary");
}
export async function openMainWindow(): Promise<void> {
  return invoke("open_main_window");
}
