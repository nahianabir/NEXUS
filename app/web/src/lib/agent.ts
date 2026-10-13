// Typed IPC bindings for the Controlled Agent. Phase I.

import { invoke } from "@tauri-apps/api/core";

export type Permission = "always" | "ask" | "restricted" | "deny";

export type AgentTool = {
  id: string;
  name: string;
  description: string;
  category: string;
  permission: string;
  enabled: boolean;
  schema: unknown;
  created_at: number;
};

export type AgentRun = {
  id: string;
  conversation_id: string | null;
  project_id: string | null;
  title: string;
  status: string;
  created_at: number;
  updated_at: number;
  finished_at: number | null;
};

export type AgentInvocation = {
  id: string;
  run_id: string;
  tool_name: string;
  permission: string;
  input: unknown;
  output: unknown;
  status: string;
  error: string | null;
  started_at: number;
  finished_at: number | null;
};

export type RunToolResult = {
  invocation: AgentInvocation;
  output: unknown;
};

export async function agentToolsList(category: string | null = null, include_disabled: boolean | null = null): Promise<AgentTool[]> {
  return invoke("agent_tools_list", { category, includeDisabled: include_disabled });
}
export async function agentToolGet(name: string): Promise<AgentTool> {
  return invoke("agent_tool_get", { name });
}
export async function agentToolSetEnabled(name: string, enabled: boolean): Promise<AgentTool> {
  return invoke("agent_tool_set_enabled", { name, enabled });
}
export async function agentRunCreate(run: AgentRun): Promise<AgentRun> {
  return invoke("agent_run_create", { run });
}
export async function agentRunList(limit: number | null = null): Promise<AgentRun[]> {
  return invoke("agent_run_list", { limit });
}
export async function agentRunGet(id: string): Promise<AgentRun> {
  return invoke("agent_run_get", { id });
}
export async function agentRunSetStatus(id: string, status: string, finished: boolean | null = null): Promise<AgentRun> {
  return invoke("agent_run_set_status", { id, status, finished });
}
export async function agentRunDelete(id: string): Promise<void> {
  return invoke("agent_run_delete", { id });
}
export async function agentInvocationCreate(inv: AgentInvocation): Promise<AgentInvocation> {
  return invoke("agent_invocation_create", { inv });
}
export async function agentInvocationGet(id: string): Promise<AgentInvocation> {
  return invoke("agent_invocation_get", { id });
}
export async function agentInvocationList(run_id: string): Promise<AgentInvocation[]> {
  return invoke("agent_invocation_list", { runId: run_id });
}
export async function agentInvocationRecent(limit: number | null = null): Promise<AgentInvocation[]> {
  return invoke("agent_invocation_recent", { limit });
}
export async function agentRunTool(invocation_id: string, file_allowlist: string[] | null = null, shell_allowlist: string[] | null = null): Promise<RunToolResult> {
  return invoke("agent_run_tool", { invocationId: invocation_id, fileAllowlist: file_allowlist, shellAllowlist: shell_allowlist });
}
export async function agentInvocationApprove(id: string): Promise<void> {
  return invoke("agent_invocation_approve", { id });
}
export async function agentInvocationDeny(id: string): Promise<void> {
  return invoke("agent_invocation_deny", { id });
}
