// Typed IPC binding for the audit log. Phase K.

import { invoke } from "@tauri-apps/api/core";

export type AuditEvent = {
  id: string;
  actor: string;
  action: string;
  target: string | null;
  detail: unknown;
  created_at: number;
};

export async function auditRecent(limit: number | null = null): Promise<AuditEvent[]> {
  return invoke("audit_recent", { limit });
}
