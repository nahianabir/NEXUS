// Model Manager — Phase 2 connects to the Rust core. The page surfaces the
// provider reachability probe, model list, profile CRUD, a test-prompt
// streaming action, and a clear "Start LM Studio server" control.

import { useEffect, useRef, useState } from "react";
import { appInfo } from "../lib/ipc";
import {
  type ModelProfile,
  type ProviderProbe,
  cancelInflight,
  modelsDelete,
  modelsList,
  modelsSetActive,
  modelsTestPrompt,
  modelsUpsert,
  onTestDelta,
  onTestDone,
  onTestError,
  onTestReasoning,
  providersProbe,
  providersSetActive,
  providersStartLmstudio,
  providersStopLmstudio,
} from "../lib/providers";

type StreamState = { running: boolean; text: string; reasoning: string; error: string | null };

export function ModelsPage() {
  const [info, setInfo] = useState<{ data_dir: string; version: string } | null>(null);
  const [probes, setProbes] = useState<ProviderProbe[]>([]);
  const [profiles, setProfiles] = useState<ModelProfile[]>([]);
  const [stream, setStream] = useState<StreamState>({ running: false, text: "", reasoning: "", error: null });
  const [lmsLog, setLmsLog] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState<boolean>(false);
  const [lastRefreshed, setLastRefreshed] = useState<number | null>(null);
  const [initialLoaded, setInitialLoaded] = useState<boolean>(false);
  const unlistens = useRef<(() => void)[]>([]);
  const inFlight = useRef<boolean>(false);

  const refresh = async () => {
    // Deduplicate: if a refresh is already in flight, do not start another.
    if (inFlight.current) return;
    inFlight.current = true;
    setRefreshing(true);
    try {
      const [probes, profiles] = await Promise.all([providersProbe(), modelsList()]);
      setProbes(probes);
      setProfiles(profiles);
      setLastRefreshed(Date.now());
      setInitialLoaded(true);
    } catch (e) {
      setLmsLog(`probe failed: ${e}`);
    } finally {
      inFlight.current = false;
      setRefreshing(false);
    }
  };

  useEffect(() => {
    appInfo().then((a) => setInfo({ data_dir: a.data_dir, version: a.version }));
    refresh().catch((e) => setLmsLog(`probe failed: ${e}`));
    let cancelled = false;
    (async () => {
      try {
        unlistens.current.push(await onTestDelta((t) => setStream((s) => ({ ...s, text: t }))));
        unlistens.current.push(await onTestReasoning((t) => setStream((s) => ({ ...s, reasoning: (s.reasoning + t).slice(-2000) }))));
        unlistens.current.push(await onTestError((e) => setStream((s) => ({ ...s, error: e, running: false }))));
        unlistens.current.push(await onTestDone((t) => { if (!cancelled) setStream((s) => ({ running: false, text: t, reasoning: s.reasoning, error: null })); }));
      } catch (e) { setLmsLog(`listener setup failed: ${e}`); }
    })();
    return () => {
      cancelled = true;
      unlistens.current.forEach((f) => f());
      unlistens.current = [];
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (!info) {
    return <div className="p-8 text-sm text-slate-500">Loading configuration…</div>;
  }

  const showInitialLoading = !initialLoaded && refreshing && probes.length === 0 && profiles.length === 0;

  const general = profiles.filter((p) => p.role === "general");
  const coding = profiles.filter((p) => p.role === "coding");
  const embedding = profiles.filter((p) => p.role === "embedding");

  const onTestPrompt = async (model: string) => {
    setStream({ running: true, text: "", reasoning: "", error: null });
    try {
      await modelsTestPrompt({
        model,
        prompt: "In one sentence, introduce yourself as a local assistant running on the user's PC.",
        temperature: 0.4,
        max_tokens: 120,
        system: "You are a concise assistant. Answer plainly.",
      });
    } catch (e) {
      setStream((s) => ({ ...s, running: false, error: String(e) }));
    }
  };

  const onCancel = async () => {
    await cancelInflight();
    setStream((s) => ({ ...s, running: false }));
  };

  const onStartLms = async () => {
    setLmsLog("starting…");
    try {
      const out = await providersStartLmstudio();
      setLmsLog(out || "started");
      setTimeout(refresh, 1500);
    } catch (e) {
      setLmsLog(`failed: ${e}`);
    }
  };
  const onStopLms = async () => {
    try {
      const out = await providersStopLmstudio();
      setLmsLog(out || "stopped");
      setTimeout(refresh, 1500);
    } catch (e) {
      setLmsLog(`failed: ${e}`);
    }
  };

  return (
    <div className="p-6 max-w-6xl space-y-6">
      {showInitialLoading && (
        <div className="panel p-3 text-sm text-slate-300 flex items-center gap-2" role="status" aria-live="polite">
          <span className="inline-block w-2 h-2 rounded-full bg-cyan2-400 animate-pulse" aria-hidden />
          Probing local backends… Model Manager opens with cached state once a probe completes.
        </div>
      )}
      <div className="panel p-5">
        <div className="flex items-start gap-3">
          <div className="flex-1">
            <div className="h-title">Model Manager</div>
            <p className="mt-1 text-sm text-slate-400">
              NEXUS talks to local backends. The reachability probe, the profile list, and the
              test-prompt streaming are all wired to the Rust core.
            </p>
          </div>
          <div className="flex gap-2">
            <button className="btn" onClick={onStartLms} title="Start the local LM Studio server">
              Start LM Studio
            </button>
            <button className="btn" onClick={onStopLms} title="Stop the local LM Studio server">
              Stop
            </button>
            <button className="btn" onClick={refresh} disabled={refreshing}>{refreshing ? "Refreshing…" : "Refresh"}</button>
          </div>
        </div>
        {lmsLog && <div className="mt-3 text-xs text-slate-500 font-mono whitespace-pre-wrap">{lmsLog}</div>}
      </div>

      <section className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {probes.map((p) => <BackendCard key={p.kind} probe={p} onActivate={async () => { await providersSetActive(p.kind as any); refresh(); }} />)}
      </section>

      <section className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <ProfileColumn
          title="General Assistant"
          subtitle="Conversational and research work."
          list={general}
          onTest={(m) => onTestPrompt(m)}
          onActivate={(p) => modelsSetActive(p.id).then(refresh)}
          onDelete={async (p) => { await modelsDelete(p.id); refresh(); }}
        />
        <ProfileColumn
          title="Coding Specialist"
          subtitle="Code generation and analysis."
          list={coding}
          onTest={(m) => onTestPrompt(m)}
          onActivate={(p) => modelsSetActive(p.id).then(refresh)}
          onDelete={async (p) => { await modelsDelete(p.id); refresh(); }}
        />
        <ProfileColumn
          title="Embedding Model"
          subtitle="Disabled until you set one up."
          list={embedding}
          onTest={(m) => onTestPrompt(m)}
          onActivate={(p) => modelsSetActive(p.id).then(refresh)}
          onDelete={async (p) => { await modelsDelete(p.id); refresh(); }}
          disabledTest="Embeddings don't chat; use Search in Phase 4."
        />
      </section>

      <section className="panel p-5">
        <div className="flex items-center justify-between">
          <div>
            <div className="h-title">Test prompt</div>
            <div className="text-xs text-slate-500">Stream a short reply from the active backend.</div>
          </div>
          {stream.running && (
            <button className="btn" onClick={onCancel}>Stop</button>
          )}
        </div>
        <div className="mt-3 panel-soft p-4 min-h-[6rem] text-sm text-slate-200 whitespace-pre-wrap font-mono">
          {stream.error ? <span className="text-rose2-400">{stream.error}</span> : stream.text || (stream.running ? "…" : "Idle.")}
        </div>
        {stream.reasoning && (
          <details className="mt-2 text-[11px] text-slate-500">
            <summary className="cursor-pointer">Reasoning ({stream.reasoning.length} chars)</summary>
            <pre className="mt-1 whitespace-pre-wrap font-mono">{stream.reasoning}</pre>
          </details>
        )}
      </section>

      <section className="panel p-5">
        <div className="h-title">New profile</div>
        <NewProfileForm onCreate={async (p) => { await modelsUpsert(p); refresh(); }} />
      </section>

      <div className="text-[11px] text-slate-500 font-mono">
        data dir: {info.data_dir} · v{info.version}
        {lastRefreshed && (
          <> · last refresh {new Date(lastRefreshed).toLocaleTimeString()}</>
        )}
        {!initialLoaded && refreshing && (
          <> · probing backends…</>
        )}
      </div>
    </div>
  );
}

function BackendCard({ probe, onActivate }: { probe: ProviderProbe; onActivate: () => void }) {
  return (
    <div className="panel p-4">
      <div className="flex items-center gap-2">
        <div className="h-title capitalize">{probe.kind}</div>
        {probe.health.reachable ? (
          <span className="chip text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10">
            reachable · {probe.health.latency_ms ?? "?"}ms
          </span>
        ) : (
          <span className="chip text-rose2-400 border-rose2-400/40 bg-rose2-400/10">unreachable</span>
        )}
      </div>
      <div className="mt-1 font-mono text-[12px] text-slate-300 break-all">{probe.health.base_url}</div>
      {probe.health.error && <div className="mt-2 text-[11px] text-rose2-400 font-mono break-all">{probe.health.error}</div>}
      {probe.models.length > 0 && (
        <div className="mt-3">
          <div className="h-sub mb-1">Models</div>
          <div className="space-y-1">
            {probe.models.map((m) => (
              <div key={m.id} className="font-mono text-[12px] text-slate-300 truncate">{m.id}</div>
            ))}
          </div>
        </div>
      )}
      <div className="mt-3 flex gap-2">
        <button className="btn" onClick={onActivate}>Use as default</button>
      </div>
    </div>
  );
}

function ProfileColumn({
  title, subtitle, list, onTest, onActivate, onDelete, disabledTest,
}: {
  title: string;
  subtitle: string;
  list: ModelProfile[];
  onTest: (modelId: string) => void;
  onActivate: (p: ModelProfile) => void;
  onDelete: (p: ModelProfile) => void;
  disabledTest?: string;
}) {
  return (
    <div className="panel p-4">
      <div className="h-title">{title}</div>
      <div className="text-xs text-slate-500">{subtitle}</div>
      <div className="mt-3 space-y-2">
        {list.length === 0 && <div className="text-xs text-slate-500">No profiles yet.</div>}
        {list.map((p) => (
          <div key={p.id} className="rounded-lg border border-white/5 bg-ink-800/50 p-3">
            <div className="flex items-center gap-2">
              <div className="font-mono text-[12px] text-slate-200 truncate flex-1">{p.model_id}</div>
              <span className="chip">{p.provider}</span>
            </div>
            <div className="mt-1 text-[11px] text-slate-500 font-mono">
              ctx={p.context_len} · temp={p.temperature} · max={p.max_tokens} · top_p={p.top_p}
            </div>
            <div className="mt-2 flex gap-2">
              <button className="btn" disabled={!!disabledTest} title={disabledTest} onClick={() => onTest(p.model_id)}>
                Test prompt
              </button>
              <button className="btn" onClick={() => onActivate(p)}>Activate</button>
              <button className="btn" onClick={() => onDelete(p)}>Delete</button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function NewProfileForm({ onCreate }: { onCreate: (p: ModelProfile) => void }) {
  const [role, setRole] = useState("general");
  const [displayName, setDisplayName] = useState("Custom Assistant");
  const [provider, setProvider] = useState("lmstudio");
  const [modelId, setModelId] = useState("");
  const [contextLen, setContextLen] = useState(4096);
  const [temperature, setTemperature] = useState(0.7);
  const [topP, setTopP] = useState(0.95);
  const [maxTokens, setMaxTokens] = useState(1024);
  // Embedding-only: the user can pre-set the dimension. The
  // Knowledge Vault "Detect dimension" button is the
  // recommended path; this field is for users who already
  // know the dimension.
  const [dimStr, setDimStr] = useState("");
  // True if the model advertises vision capability. The
  // chat pipeline refuses image attachments when this is
  // off; see Phase P3 — Bug D in the implementation plan.
  const [supportsVision, setSupportsVision] = useState(false);

  const submit = async () => {
    if (!modelId.trim()) return;
    const now = Date.now();
    const p: ModelProfile = {
      id: "",
      role,
      display_name: displayName,
      provider,
      model_id: modelId.trim(),
      gguf_path: null,
      context_len: contextLen,
      gpu_layers: null,
      temperature,
      top_p: topP,
      max_tokens: maxTokens,
      enabled: true,
      created_at: now,
      updated_at: now,
      dimension: dimStr.trim() === "" ? null : Number(dimStr) || null,
      model_identity: role === "embedding" ? `${provider}::${modelId.trim()}` : null,
      supports_vision: supportsVision,
    };
    await onCreate(p);
    setModelId("");
    setDimStr("");
    setSupportsVision(false);
  };

  return (
    <div className="mt-3 grid grid-cols-1 md:grid-cols-3 gap-3">
      <Field label="Role">
        <select className="input" value={role} onChange={(e) => setRole(e.target.value)}>
          <option value="general">General</option>
          <option value="coding">Coding</option>
          <option value="embedding">Embedding</option>
        </select>
      </Field>
      <Field label="Display name">
        <input className="input" value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
      </Field>
      <Field label="Provider">
        <select className="input" value={provider} onChange={(e) => setProvider(e.target.value)}>
          <option value="lmstudio">LM Studio</option>
          <option value="llamacpp">llama.cpp</option>
          <option value="ollama">Ollama</option>
        </select>
      </Field>
      <Field label="Model id">
        <input className="input" value={modelId} placeholder="qwen3.5-9b-q4_k_m" onChange={(e) => setModelId(e.target.value)} />
      </Field>
      <Field label="Context length">
        <input type="number" className="input" value={contextLen} onChange={(e) => setContextLen(Number(e.target.value) || 0)} />
      </Field>
      <Field label="Max tokens">
        <input type="number" className="input" value={maxTokens} onChange={(e) => setMaxTokens(Number(e.target.value) || 0)} />
      </Field>
      <Field label="Temperature">
        <input type="number" step="0.05" min="0" max="2" className="input" value={temperature} onChange={(e) => setTemperature(Number(e.target.value) || 0)} />
      </Field>
      <Field label="Top-p">
        <input type="number" step="0.05" min="0" max="1" className="input" value={topP} onChange={(e) => setTopP(Number(e.target.value) || 0)} />
      </Field>
      <Field label={<>Embedding dim <span className="text-slate-600">(embedding only)</span></>}>
        <input
          type="number"
          className="input"
          value={dimStr}
          placeholder="e.g. 768"
          onChange={(e) => setDimStr(e.target.value)}
        />
      </Field>
      <div className="md:col-span-3 flex items-center gap-3">
        <label className="flex items-center gap-2 text-[11px] text-slate-300 cursor-pointer select-none" title="Enable only if the model advertises OpenAI-style image_url input. Off by default; the chat pipeline refuses image attachments when this is off so a text-only LLM is never asked to interpret an image.">
          <input
            type="checkbox"
            className="accent-cyan2-500"
            checked={supportsVision}
            onChange={(e) => setSupportsVision(e.target.checked)}
          />
          Supports vision (image attachments)
        </label>
      </div>
      <div className="md:col-span-3 flex justify-end">
        <button className="btn btn-primary" onClick={submit}>Create profile</button>
      </div>
    </div>
  );
}

function Field({ label, children }: { label: React.ReactNode; children: React.ReactNode }) {
  return (
    <label className="space-y-1 block">
      <div className="h-sub">{label}</div>
      {children}
    </label>
  );
}