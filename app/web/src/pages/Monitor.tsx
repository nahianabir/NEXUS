// System Monitor — Phase 1 shows real CPU / RAM samples.
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { dashboardSummary, type DashboardSummary } from "../lib/ipc";
import { auditRecent, type AuditEvent } from "../lib/audit";
import { VirtualList } from "../components/VirtualList";
import { usePromptDialog } from "../hooks/usePromptDialog";

export function MonitorPage() {
  const { confirm: confirmModal, alert: alertModal } = usePromptDialog();
  const [data, setData] = useState<DashboardSummary | null>(null);
  useEffect(() => {
    let alive = true;
    const tick = () => dashboardSummary().then((d) => { if (alive) setData(d); });
    tick();
    const id = setInterval(tick, 3000);
    return () => { alive = false; clearInterval(id); };
  }, []);

  if (!data) return <div className="p-8 text-sm text-slate-500">Loading…</div>;

  const { sample } = data;
  const cpu = sample.cpu_percent;
  const cpuAvailable = sample.cpu_available !== false;
  const cpuBasis = sample.cpu_basis ?? "system · all logical processors";
  const ramPct = sample.mem_total_mb > 0 ? (sample.mem_used_mb / sample.mem_total_mb) * 100 : 0;

  return (
    <div className="p-6 max-w-4xl space-y-6">
      <div className="panel p-5">
        <div className="h-title">System Monitor</div>
        <p className="mt-1 text-sm text-slate-400">
          Real samples from the host. GPU metrics will appear here when an Intel Arc backend reports them.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {cpuAvailable ? (
          <Meter
            label="System CPU"
            value={cpu}
            max={100}
            suffix="%"
            detail={`${cpu.toFixed(1)}% · ${cpuBasis}`}
          />
        ) : (
          <Meter
            label="System CPU"
            value={0}
            max={100}
            suffix="%"
            unavailable="priming…"
            detail={`${cpuBasis} · first sample warming up`}
          />
        )}
        <Meter label="RAM" value={ramPct} max={100} suffix="%" detail={`${sample.mem_used_mb} / ${sample.mem_total_mb} MB`} />
      </div>

      <div className="panel p-5">
        <div className="h-title">Process</div>
        <div className="mt-2 text-sm text-slate-300">NEXUS app memory: <span className="font-mono">{sample.proc_mem_mb} MB</span></div>
      </div>

      <div className="panel p-5">
        <div className="h-title">Storage</div>
        <StorageRow />
      </div>

      <div className="panel p-5">
        <div className="flex items-start gap-3">
          <div className="flex-1">
            <div className="h-title">Audit log</div>
            <p className="mt-1 text-sm text-slate-400">
              Every privileged action is recorded. The most recent {200} events are loaded into a virtualised list.
            </p>
          </div>
          <button
            className="btn text-rose2-400"
            onClick={async () => {
              const ok = await confirmModal({
                title: "Clear audit log",
                description: "Permanently clear the audit log? This cannot be undone.",
                submitLabel: "Clear log",
                destructive: true,
              });
              if (!ok) return;
              try { await invoke("audit_clear"); } catch (e) { await alertModal("Clear failed", String(e)); return; }
              location.reload();
            }}
          >
            Clear log
          </button>
        </div>
        <AuditList />
      </div>
    </div>
  );
}

function Meter({ label, value, max, suffix, detail, unavailable }: { label: string; value: number; max: number; suffix?: string; detail?: string; unavailable?: string }) {
  const isUnavailable = unavailable !== undefined;
  const pct = isUnavailable ? 0 : Math.max(0, Math.min(100, (value / max) * 100));
  return (
    <div className="panel p-4">
      <div className="flex items-center justify-between">
        <div className="h-title">{label}</div>
        <div className="font-mono text-slate-300">
          {isUnavailable ? unavailable : `${value.toFixed(1)}${suffix ?? ""}`}
        </div>
      </div>
      <div className="mt-3 h-2 w-full overflow-hidden rounded-full bg-ink-800">
        {isUnavailable ? (
          <div
            className="h-full rounded-full bg-slate-700 animate-pulse"
            style={{ width: "100%" }}
            aria-label={`${label} unavailable`}
          />
        ) : (
          <div
            className="h-full rounded-full bg-gradient-to-r from-cyan2-500 to-violet2-500"
            style={{ width: `${pct}%` }}
          />
        )}
      </div>
      {detail && <div className="mt-2 text-[11px] text-slate-500 font-mono">{detail}</div>}
    </div>
  );
}

function StorageRow() {
  // Approximate via navigator if available, else leave a note.
  const [info, setInfo] = useState<{ used: number; quota: number } | null>(null);
  useEffect(() => {
    if (navigator.storage && navigator.storage.estimate) {
      navigator.storage.estimate().then((est) => {
        if (est.usage != null && est.quota != null) setInfo({ used: est.usage, quota: est.quota });
      });
    }
  }, []);
  if (!info) {
    return <div className="mt-2 text-sm text-slate-500">Browser storage estimate unavailable; see data_dir on the Command Center for the actual on-disk database path.</div>;
  }
  const gb = (b: number) => (b / 1024 / 1024 / 1024).toFixed(2);
  return (
    <div className="mt-2 text-sm text-slate-300">
      Browser storage: <span className="font-mono">{gb(info.used)} GB</span> used of <span className="font-mono">{gb(info.quota)} GB</span> available to the WebView profile.
    </div>
  );
}

function AuditList() {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [filter, setFilter] = useState("");
  useEffect(() => { auditRecent(200).then(setEvents).catch(() => setEvents([])); }, []);
  const filtered = events.filter((e) => !filter || e.action.toLowerCase().includes(filter.toLowerCase()) || (e.target ?? "").toLowerCase().includes(filter.toLowerCase()));
  if (filtered.length === 0) return <div className="text-xs text-slate-500 mt-2">No audit events yet.</div>;
  return (
    <div className="mt-2">
      <input className="input mb-2" placeholder="Filter by action or target…" value={filter} onChange={(e) => setFilter(e.target.value)} />
      <VirtualList
        items={filtered}
        rowHeight={32}
        height={320}
        getKey={(e) => e.id}
        render={(e) => (
          <div className="flex items-center gap-2 px-2 text-[11px] border-b border-white/5">
            <span className="text-slate-500 font-mono w-32 truncate">{new Date(e.created_at).toLocaleTimeString()}</span>
            <span className="text-cyan2-400 font-mono w-48 truncate">{e.action}</span>
            <span className="text-slate-300 truncate flex-1">{e.target ?? ""}</span>
          </div>
        )}
      />
    </div>
  );
}
