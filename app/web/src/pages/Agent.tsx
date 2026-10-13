// Controlled Agent. Phase I.
//
// Two-pane view:
//   * Tools: every registered tool, grouped by category, with its
//     permission level and an on/off toggle.
//   * Runs: a list of agent runs (each is a sequence of tool
//     invocations). Pending invocations are flagged with an
//     "approve" / "deny" pair.
//
// The agent has no autonomous loop in this phase; the user (or a
// future planner) creates runs and adds invocations explicitly. The
// point of the page is to make every action auditable.

import { useEffect, useState } from "react";
import {
  type AgentInvocation,
  type AgentRun,
  type AgentTool,
  agentInvocationApprove,
  agentInvocationCreate,
  agentInvocationDeny,
  agentInvocationList,
  agentInvocationRecent,
  agentRunCreate,
  agentRunList,
  agentRunSetStatus,
  agentRunTool,
  agentToolSetEnabled,
  agentToolsList,
} from "../lib/agent";
import { settings } from "../lib/settings";
import { usePromptDialog } from "../hooks/usePromptDialog";

const CATEGORIES = ["knowledge", "research", "projects", "files", "dev", "system"] as const;

const PERM_TINT: Record<string, string> = {
  always: "border-cyan2-500/30 text-cyan2-400",
  ask: "border-amber2-500/30 text-amber2-400",
  restricted: "border-violet2-500/30 text-violet2-400",
  deny: "border-rose2-500/30 text-rose2-400",
};

export function AgentPage() {
  const { prompt: promptModal, alert: alertModal } = usePromptDialog();
  const [tab, setTab] = useState<"tools" | "runs">("tools");
  const [tools, setTools] = useState<AgentTool[]>([]);
  const [runs, setRuns] = useState<AgentRun[]>([]);
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const [invocations, setInvocations] = useState<AgentInvocation[]>([]);
  const [pending, setPending] = useState<AgentInvocation[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setTools(await agentToolsList(null, true));
      setRuns(await agentRunList(50));
      setPending(await agentInvocationRecent(20));
    } catch (e) { setError(String(e)); }
  };

  // Persisted allowlists from Settings.
  const runInvocation = async (invocationId: string) => {
    const cfg = await settings.get();
    return agentRunTool(invocationId, cfg.agent.file_allowlist, cfg.agent.shell_allowlist);
  };

  useEffect(() => { void refresh(); }, []);

  useEffect(() => {
    if (activeRunId) {
      (async () => {
        try { setInvocations(await agentInvocationList(activeRunId)); }
        catch (e) { setError(String(e)); }
      })();
    } else { setInvocations([]); }
  }, [activeRunId]);

  const onToggleTool = async (tool: AgentTool) => {
    await agentToolSetEnabled(tool.name, !tool.enabled);
    await refresh();
  };

  const onNewRun = async () => {
    const title = await promptModal({
      title: "New agent run",
      label: "Run title",
      defaultValue: "agent run",
      submitLabel: "Start run",
      validate: (v) => (v.trim() ? "" : "Title is required"),
    });
    if (!title) return;
    const run = await agentRunCreate({
      id: "", conversation_id: null, project_id: null,
      title, status: "running",
      created_at: 0, updated_at: 0, finished_at: null,
    });
    await refresh();
    setActiveRunId(run.id);
    setTab("runs");
  };

  const onAddInvocation = async (tool: AgentTool) => {
    if (!activeRunId) { await alertModal("Run required", "Create or select a run first."); return; }
    if (tool.permission === "deny") { await alertModal("Tool denied", "This tool is denied."); return; }
    const inputStr = await promptModal({
      title: `Input JSON for ${tool.name}`,
      label: "Input",
      defaultValue: "{}",
      placeholder: "{...}",
      submitLabel: "Add invocation",
    });
    if (inputStr == null) return;
    let input: unknown;
    try { input = JSON.parse(inputStr); } catch (e) { await alertModal("Invalid JSON", String(e)); return; }
    const inv = await agentInvocationCreate({
      id: "", run_id: activeRunId, tool_name: tool.name, permission: tool.permission,
      input, output: null, status: "pending", error: null, started_at: 0, finished_at: null,
    });
    if (tool.permission === "always") {
      await runInvocation(inv.id);
    }
    if (activeRunId) setInvocations(await agentInvocationList(activeRunId));
    setPending(await agentInvocationRecent(20));
  };

  const onApprove = async (id: string) => {
    await agentInvocationApprove(id);
    await agentRunTool(id, [], []);
    if (activeRunId) setInvocations(await agentInvocationList(activeRunId));
    setPending(await agentInvocationRecent(20));
  };
  const onDeny = async (id: string) => {
    await agentInvocationDeny(id);
    if (activeRunId) setInvocations(await agentInvocationList(activeRunId));
    setPending(await agentInvocationRecent(20));
  };

  const onFinishRun = async (status: string) => {
    if (!activeRunId) return;
    await agentRunSetStatus(activeRunId, status, true);
    await refresh();
  };

  return (
    <>
    <div className="grid h-full grid-cols-12 gap-4 p-4 min-h-0">
      <aside className="col-span-3 panel p-3 flex flex-col min-h-0">
        <div className="flex items-center justify-between">
          <div className="h-title">Agent</div>
          <button className="btn btn-primary" onClick={onNewRun}>New run</button>
        </div>
        <div className="mt-2 flex gap-1 text-[11px]">
          <button className={`kbd ${tab === "tools" ? "text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10" : ""}`} onClick={() => setTab("tools")}>tools</button>
          <button className={`kbd ${tab === "runs" ? "text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10" : ""}`} onClick={() => setTab("runs")}>runs</button>
        </div>
        <div className="mt-3 space-y-1 overflow-y-auto flex-1 min-h-0">
          {pending.filter((p) => p.status === "pending").length > 0 && (
            <div className="panel-soft p-2 text-[11px] text-amber2-400">
              <div className="h-sub">Pending approval</div>
              {pending.filter((p) => p.status === "pending").map((p) => (
                <div key={p.id} className="mt-1 border border-white/5 rounded p-1.5">
                  <div className="text-slate-200">{p.tool_name}</div>
                  <div className="mt-1 flex gap-1">
                    <button className="kbd" onClick={() => { void onApprove(p.id); }}>approve</button>
                    <button className="kbd" onClick={() => { void onDeny(p.id); }}>deny</button>
                  </div>
                </div>
              ))}
            </div>
          )}
          {tab === "tools" && (
            <div className="space-y-1.5">
              {CATEGORIES.map((cat) => {
                const inCat = tools.filter((t) => t.category === cat);
                if (inCat.length === 0) return null;
                return (
                  <div key={cat} className="panel-soft p-2">
                    <div className="text-[10px] text-slate-500 uppercase tracking-wide mb-1">{cat}</div>
                    <ul className="space-y-1">
                      {inCat.map((t) => (
                        <li key={t.id} className="text-[12px] flex items-center gap-1.5">
                          <input type="checkbox" checked={t.enabled} onChange={() => { void onToggleTool(t); }} />
                          <span className="flex-1 truncate text-slate-200" title={t.description}>{t.name}</span>
                          <span className={`kbd ${PERM_TINT[t.permission] ?? ""}`}>{t.permission}</span>
                        </li>
                      ))}
                    </ul>
                  </div>
                );
              })}
            </div>
          )}
          {tab === "runs" && (
            <div className="space-y-1.5">
              {runs.length === 0 && <div className="text-xs text-slate-500">No runs yet.</div>}
              {runs.map((r) => (
                <button
                  key={r.id}
                  className={`w-full text-left rounded p-2 text-[12px] border ${activeRunId === r.id ? "bg-cyan2-500/10 border-cyan2-500/30" : "border-white/5 hover:bg-white/5"}`}
                  onClick={() => setActiveRunId(r.id)}
                >
                  <div className="flex items-center gap-1">
                    <span className="flex-1 truncate text-slate-200">{r.title}</span>
                    <span className="chip">{r.status}</span>
                  </div>
                  <div className="text-[10px] text-slate-500 font-mono">{new Date(r.created_at).toLocaleString()}</div>
                </button>
              ))}
            </div>
          )}
        </div>
      </aside>

      <section className="col-span-9 panel p-4 flex flex-col min-h-0">
        {error && <div className="text-[11px] text-rose2-400 font-mono mb-2">{error}</div>}
        {tab === "tools" ? (
          <div className="space-y-3 overflow-y-auto pr-1">
            <div className="h-title">Tool registry</div>
            <p className="text-[12px] text-slate-400">
              Every tool the agent can call. "always" runs without prompting;
              "ask" requires explicit approval; "restricted" is opt-in via Settings;
              "deny" never runs.
            </p>
            {CATEGORIES.map((cat) => {
              const inCat = tools.filter((t) => t.category === cat);
              if (inCat.length === 0) return null;
              return (
                <div key={cat} className="panel-soft p-3 space-y-2">
                  <div className="h-sub">{cat}</div>
                  <div className="space-y-1.5">
                    {inCat.map((t) => (
                      <div key={t.id} className="border border-white/5 rounded p-2 text-[12px]">
                        <div className="flex items-center gap-2">
                          <span className="font-mono text-cyan2-400">{t.name}</span>
                          <span className={`kbd ${PERM_TINT[t.permission] ?? ""}`}>{t.permission}</span>
                          <span className="ml-auto text-[10px] font-mono text-slate-500">{t.enabled ? "enabled" : "disabled"}</span>
                        </div>
                        <div className="mt-1 text-slate-300">{t.description}</div>
                        <pre className="mt-1 text-[10px] text-slate-500 font-mono whitespace-pre-wrap">{JSON.stringify(t.schema, null, 2)}</pre>
                        <div className="mt-1 flex gap-2">
                          <button className="btn" onClick={() => { void onToggleTool(t); }}>{t.enabled ? "Disable" : "Enable"}</button>
                          {activeRunId && t.permission !== "deny" && (
                            <button className="btn btn-primary" onClick={() => { void onAddInvocation(t); }}>Add to run</button>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="space-y-3 overflow-y-auto pr-1">
            <div className="flex items-center gap-2">
              <div className="h-title flex-1">Runs</div>
              {activeRunId && <span className="text-[11px] text-slate-500">{invocations.length} invocations</span>}
            </div>
            {activeRunId ? (
              <>
                <div className="flex gap-2">
                  <button className="btn" onClick={() => { void onFinishRun("completed"); }}>Mark completed</button>
                  <button className="btn" onClick={() => { void onFinishRun("failed"); }}>Mark failed</button>
                  <button className="btn" onClick={() => { void onFinishRun("cancelled"); }}>Mark cancelled</button>
                </div>
                <div className="space-y-1.5">
                  {invocations.length === 0 && <div className="text-xs text-slate-500">No invocations yet. Add one from the Tools tab.</div>}
                  {invocations.map((inv) => (
                    <div key={inv.id} className="panel-soft p-2 text-[12px]">
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-cyan2-400">{inv.tool_name}</span>
                        <span className="kbd">{inv.permission}</span>
                        <span className={`kbd ${inv.status === "executed" ? "border-cyan2-500/40 bg-cyan2-500/10 text-cyan2-400" : inv.status === "failed" || inv.status === "denied" ? "border-rose2-500/40 bg-rose2-500/10 text-rose2-400" : "border-amber2-500/40 bg-amber2-500/10 text-amber2-400"}`}>{inv.status}</span>
                        <span className="ml-auto text-[10px] font-mono text-slate-500">{new Date(inv.started_at).toLocaleString()}</span>
                      </div>
                      <div className="mt-1 grid grid-cols-1 md:grid-cols-2 gap-2 text-[11px]">
                        <div>
                          <div className="text-slate-500 uppercase text-[10px]">input</div>
                          <pre className="font-mono text-slate-300 whitespace-pre-wrap">{JSON.stringify(inv.input, null, 2)}</pre>
                        </div>
                        <div>
                          <div className="text-slate-500 uppercase text-[10px]">output</div>
                          <pre className="font-mono text-slate-300 whitespace-pre-wrap">{JSON.stringify(inv.output, null, 2) || "(none)"}</pre>
                          {inv.error && <div className="text-rose2-400 font-mono">{inv.error}</div>}
                        </div>
                      </div>
                      {(inv.status === "pending") && (
                        <div className="mt-1 flex gap-1">
                          <button className="btn" onClick={() => { void onApprove(inv.id); }}>approve</button>
                          <button className="btn" onClick={() => { void onDeny(inv.id); }}>deny</button>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <div className="text-sm text-slate-500 text-center py-12">Select a run from the sidebar.</div>
            )}
          </div>
        )}
      </section>
    </div>
    </>
  );
}