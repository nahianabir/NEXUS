// Orchestrator — Phase F.

import { useEffect, useState } from "react";
import {
  type RouterCapabilities,
  type RouterDecision,
  type RouterDecisionRecord,
  type RouterLifecycleStatus,
  routerCapabilitiesInfer,
  routerCapabilitiesList,
  routerCapabilitiesUpsert,
  routerDecisionsRecent,
  routerLifecycleList,
  routerLifecycleTransition,
  routerRoute,
} from "../lib/router";
import { type ModelProfile, modelsList } from "../lib/providers";

const STATES = ["unloaded", "loading", "ready", "busy", "unloading", "load_failed", "unload_failed"];

export function OrchestratorPage() {
  const [profiles, setProfiles] = useState<ModelProfile[]>([]);
  const [caps, setCaps] = useState<RouterCapabilities[]>([]);
  const [lifecycle, setLifecycle] = useState<RouterLifecycleStatus[]>([]);
  const [decisions, setDecisions] = useState<RouterDecisionRecord[]>([]);
  const [task, setTask] = useState("coding");
  const [prefer, setPrefer] = useState<string>("");
  const [ctx, setCtx] = useState<number>(4096);
  const [lastDecision, setLastDecision] = useState<RouterDecision | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setProfiles(await modelsList());
      setCaps(await routerCapabilitiesList());
      setLifecycle(await routerLifecycleList());
      setDecisions(await routerDecisionsRecent(50));
    } catch (e) { setError(String(e)); }
  };
  useEffect(() => { refresh(); }, []);

  return (
    <div className="p-6 max-w-7xl space-y-4">
      <div className="panel p-5">
        <div className="h-title">Model Orchestrator</div>
        <p className="mt-1 text-sm text-slate-400">
          Capability registry, lifecycle state, and a small routing playground.
          Every decision is recorded; the assistant uses the same policy.
        </p>
        {error && <div className="mt-2 text-xs text-rose2-400 font-mono">{error}</div>}
      </div>

      <div className="panel p-4">
        <div className="h-title text-sm">Test a routing decision</div>
        <div className="mt-2 flex flex-wrap items-center gap-2">
          <select className="input" value={task} onChange={(e) => setTask(e.target.value)}>
            <option value="general">general</option>
            <option value="coding">coding</option>
            <option value="extraction">extraction</option>
            <option value="embedding">embedding</option>
            <option value="reranking">reranking</option>
            <option value="structured">structured</option>
            <option value="tool">tool</option>
          </select>
          <select className="input" value={prefer} onChange={(e) => setPrefer(e.target.value)}>
            <option value="">(no preference)</option>
            {profiles.map((p) => <option key={p.id} value={p.id}>{p.display_name} · {p.model_id}</option>)}
          </select>
          <input className="input w-32" type="number" value={ctx} onChange={(e) => setCtx(Number(e.target.value))} />
          <span className="text-[11px] text-slate-500">min ctx</span>
          <button
            className="btn btn-primary"
            onClick={async () => {
              try {
                const d = await routerRoute({ task, min_context_tokens: ctx, prefer_profile_id: prefer || null });
                setLastDecision(d);
                await refresh();
              } catch (e) { setError(String(e)); }
            }}
          >Route</button>
        </div>
        {lastDecision && (
          <div className="mt-3 text-[12px] space-y-1">
            <div>
              selected: <span className="font-mono text-cyan2-400">{lastDecision.selected ?? "(none)"}</span>
              {lastDecision.fell_back && <span className="ml-2 chip text-amber2-400 border-amber2-400/40 bg-amber2-400/10">fell back</span>}
            </div>
            <div className="text-slate-400">{lastDecision.reason}</div>
            {lastDecision.rejected.length > 0 && (
              <div className="mt-1">
                <div className="text-[10px] text-slate-500">rejected:</div>
                <ul className="list-disc list-inside text-[11px] text-slate-400">
                  {lastDecision.rejected.map((r, i) => (
                    <li key={i}><code className="text-cyan2-400">{r.profile_id.slice(-8)}</code> — {r.reason}</li>
                  ))}
                </ul>
              </div>
            )}
          </div>
        )}
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="panel p-4 space-y-2">
          <div className="h-title text-sm">Profiles & capabilities</div>
          <div className="text-[10px] text-slate-500">
            {profiles.length} profiles · {caps.length} capability rows
          </div>
          <div className="space-y-1.5">
            {profiles.map((p) => {
              const cap = caps.find((c) => c.profile_id === p.id);
              return (
                <div key={p.id} className="border border-white/5 rounded-md p-2 text-[12px] space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="chip">{p.role}</span>
                    <span className="text-slate-200">{p.display_name}</span>
                    <span className="text-[10px] font-mono text-slate-500 ml-auto">{p.model_id}</span>
                  </div>
                  {cap ? (
                    <div className="flex flex-wrap gap-1 text-[10px]">
                      {cap.supports_chat && <span className="chip">chat</span>}
                      {cap.supports_code && <span className="chip">code</span>}
                      {cap.supports_streaming && <span className="chip">stream</span>}
                      {cap.supports_tool_calling && <span className="chip">tools</span>}
                      {cap.supports_structured_output && <span className="chip">json</span>}
                      {cap.is_embedding && <span className="chip">embed</span>}
                      <span className="chip">~{cap.approx_ctx_tokens} ctx</span>
                      <span className="chip">~{cap.approx_memory_mb} MB</span>
                      {cap.verified ? <span className="chip text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10">verified</span>
                                    : <span className="chip text-amber2-400 border-amber2-400/40 bg-amber2-400/10">inferred</span>}
                    </div>
                  ) : (
                    <div className="text-[10px] text-slate-500">no capabilities row yet</div>
                  )}
                  <div className="flex gap-1.5">
                    <button className="btn" onClick={async () => { await routerCapabilitiesInfer(p.id); await refresh(); }}>Infer</button>
                    {cap && <button className="btn" onClick={async () => {
                      await routerCapabilitiesUpsert({ ...cap, verified: !cap.verified });
                      await refresh();
                    }}>{cap.verified ? "Mark inferred" : "Mark verified"}</button>}
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        <div className="panel p-4 space-y-2">
          <div className="h-title text-sm">Lifecycle</div>
          <div className="text-[10px] text-slate-500">{lifecycle.length} profiles tracked</div>
          <div className="space-y-1.5">
            {profiles.map((p) => {
              const s = lifecycle.find((x) => x.profile_id === p.id);
              const state = s?.state ?? "untracked";
              return (
                <div key={p.id} className="border border-white/5 rounded-md p-2 text-[12px]">
                  <div className="flex items-center gap-2">
                    <span className="text-slate-200">{p.display_name}</span>
                    <span className="ml-auto chip">{state}</span>
                  </div>
                  {s?.last_error && <div className="text-rose2-400 text-[10px] font-mono">{s.last_error}</div>}
                  {s && (
                    <div className="text-[10px] font-mono text-slate-500">
                      inflight {s.inflight} · last {new Date(s.last_transition).toLocaleString()}
                    </div>
                  )}
                  <div className="mt-1 flex flex-wrap gap-1">
                    {STATES.map((st) => (
                      <button key={st} className="btn text-[10px] py-0.5 px-1.5" onClick={async () => {
                        try { await routerLifecycleTransition(p.id, st, null); await refresh(); }
                        catch (e) { setError(String(e)); }
                      }}>{st}</button>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      <div className="panel p-4">
        <div className="h-title text-sm">Recent decisions</div>
        {decisions.length === 0 && <div className="text-[12px] text-slate-500">No decisions yet — try the "Route" button above.</div>}
        <div className="mt-2 space-y-1 text-[12px]">
          {decisions.map((d) => (
            <div key={d.id} className="border border-white/5 rounded p-2">
              <div className="flex items-center gap-2">
                <span className="chip">{d.task}</span>
                <span className="font-mono text-cyan2-400">{d.selected ?? "(none)"}</span>
                {d.fell_back && <span className="chip text-amber2-400 border-amber2-400/40 bg-amber2-400/10">fallback</span>}
                <span className="text-[10px] font-mono text-slate-500 ml-auto">{new Date(d.created_at).toLocaleString()}</span>
              </div>
              <div className="text-[11px] text-slate-400">{d.reason}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
