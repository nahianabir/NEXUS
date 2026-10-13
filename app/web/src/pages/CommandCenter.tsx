// Command Center — the home dashboard. Polls the core for a real summary and
// renders the AI activity glyph, telemetry, and quick links to every module.
// Every tile is clickable; nothing is decorative.

import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { NexusGlyph } from "../components/NexusGlyph";
import { dashboardSummary, type DashboardSummary } from "../lib/ipc";

function Stat({ label, value, suffix }: { label: string; value: number | string; suffix?: string }) {
  return (
    <div className="panel-soft px-4 py-3">
      <div className="h-sub">{label}</div>
      <div className="mt-1 font-mono text-lg text-slate-100">
        {value}
        {suffix ? <span className="text-slate-500 text-sm ml-1">{suffix}</span> : null}
      </div>
    </div>
  );
}

export function CommandCenterPage() {
  const [data, setData] = useState<DashboardSummary | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    const tick = () => dashboardSummary()
      .then((d) => { if (alive) setData(d); })
      .catch((e) => { if (alive) setErr(String(e)); });
    tick();
    const id = setInterval(tick, 4000);
    return () => { alive = false; clearInterval(id); };
  }, []);

  if (err) {
    return (
      <div className="p-8">
        <div className="panel p-6">
          <div className="h-title">Core unavailable</div>
          <p className="mt-2 text-sm text-slate-400">
            NEXUS could not reach its Rust core. The IPC bridge is required for every page.
            <span className="block mt-2 font-mono text-xs text-rose2-400">{err}</span>
          </p>
        </div>
      </div>
    );
  }

  if (!data) {
    return (
      <div className="grid h-full place-items-center text-sm text-slate-500">
        Booting…
      </div>
    );
  }

  return (
    <div className="grid h-full grid-cols-12 gap-4 p-6">
      <section className="col-span-12 lg:col-span-7 panel relative overflow-hidden p-6">
        <div className="absolute inset-0 grid-bg opacity-40" />
        <div className="relative flex flex-col h-full">
          <div className="flex items-center gap-2">
            <div className="h-sub">Command Center</div>
            <div className="ml-auto chip">
              <span className="h-1.5 w-1.5 rounded-full bg-cyan2-500 animate-breathe" />
              {data.active_model ?? "No model loaded"}
            </div>
          </div>
          <div className="flex-1 grid place-items-center">
            <NexusGlyph activity="idle" size={280} />
          </div>
          <div className="text-center text-sm text-slate-300">
            <div className="h-title">Standing by.</div>
            <div className="mt-1 text-slate-500">
              {data.app.name} v{data.app.version} · data at {data.app.data_dir}
            </div>
          </div>
        </div>
      </section>

      <section className="col-span-12 lg:col-span-5 grid grid-cols-2 gap-3">
        <Link to="/assistant" className="panel p-4 hover:border-cyan2-500/40">
          <div className="h-sub">Assistant</div>
          <div className="mt-1 text-2xl font-mono">{data.conversations}</div>
          <div className="text-xs text-slate-500">conversations</div>
        </Link>
        <Link to="/knowledge" className="panel p-4 hover:border-cyan2-500/40">
          <div className="h-sub">Knowledge</div>
          <div className="mt-1 text-2xl font-mono">{data.documents}</div>
          <div className="text-xs text-slate-500">indexed documents</div>
        </Link>
        <Link to="/tasks" className="panel p-4 hover:border-cyan2-500/40">
          <div className="h-sub">Tasks</div>
          <div className="mt-1 text-2xl font-mono">{data.tasks_open}</div>
          <div className="text-xs text-slate-500">open</div>
        </Link>
        <Link to="/experiments" className="panel p-4 hover:border-cyan2-500/40">
          <div className="h-sub">Experiments</div>
          <div className="mt-1 text-2xl font-mono">0</div>
          <div className="text-xs text-slate-500">runs · Phase 7</div>
        </Link>
        <Link to="/agent" className="panel p-4 hover:border-cyan2-500/40 col-span-2">
          <div className="h-sub">Agent & Tools</div>
          <div className="mt-1 text-sm text-slate-300">
            Read-only by default. Workspace writes and shell commands require explicit approval.
          </div>
        </Link>
      </section>

      <section className="col-span-12 panel p-4">
        <div className="h-sub mb-3">Local telemetry</div>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
          <Stat label="CPU" value={data.sample.cpu_percent.toFixed(1)} suffix="%" />
          <Stat label="RAM used" value={data.sample.mem_used_mb} suffix={`/ ${data.sample.mem_total_mb} MB`} />
          <Stat label="App memory" value={data.sample.proc_mem_mb} suffix="MB" />
          <Stat label="Job queue" value={data.queue_depth} />
        </div>
        <div className="mt-3 text-[11px] text-slate-500 font-mono">
          Sample collected at {new Date(data.sample.collected_at).toLocaleTimeString()}
        </div>
      </section>
    </div>
  );
}
