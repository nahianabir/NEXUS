// Typed IPC bindings for the data-directory management
// commands (Phase 5).

import { invoke } from "@tauri-apps/api/core";

export type DataDirOrigin = "env" | "portable" | "default";

export type DataDirInfo = {
  current: string;
  is_default: boolean;
  is_portable: boolean;
  free_bytes: number;
  total_bytes: number;
};

export type DataDirValidation = {
  ok: boolean;
  reason: string;
};

export type DataDirMigrationReport = {
  files: number;
  bytes: number;
  source: string;
  destination: string;
};

export async function appDataDirInfo(): Promise<DataDirInfo> {
  return invoke("app_data_dir_info");
}

export async function appDataDirValidate(path: string): Promise<DataDirValidation> {
  return invoke("app_data_dir_validate", { path });
}

export async function appDataDirMigrate(destination: string): Promise<DataDirMigrationReport> {
  return invoke("app_data_dir_migrate", { destination });
}

export async function appDataDirOpen(): Promise<void> {
  return invoke("app_data_dir_open");
}
