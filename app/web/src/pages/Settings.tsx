// Settings — Phase 1 lets the user edit the safe subset (URLs, embedding
// toggle, permission level, theme) and persists them via the Rust config file.
import { useEffect, useState } from "react";
import { settings, type AppSettings } from "../lib/settings";
import { appInfo } from "../lib/ipc";
import {
  appDataDirInfo,
  appDataDirValidate,
  appDataDirMigrate,
  appDataDirOpen,
  type DataDirInfo,
  type DataDirValidation,
  type DataDirMigrationReport,
} from "../lib/dataDir";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

export function SettingsPage() {
  const [cfg, setCfg] = useState<AppSettings | null>(null);
  const [info, setInfo] = useState<{ data_dir: string } | null>(null);
  useEffect(() => {
    settings.get().then(setCfg);
    appInfo().then((a) => setInfo({ data_dir: a.data_dir }));
  }, []);

  if (!cfg || !info) return <div className="p-8 text-sm text-slate-500">Loading…</div>;

  const save = async (next: AppSettings) => {
    setCfg(next);
    await settings.save(next);
  };

  return (
    <div className="p-6 max-w-4xl space-y-6">
      <div className="panel p-5">
        <div className="h-title">Settings</div>
        <p className="mt-1 text-sm text-slate-400">
          Phase 1 ships the safe configuration surface. Each value is written to
          <span className="ml-1 font-mono text-slate-300">{info.data_dir}\config.toml</span>.
        </p>
      </div>

      <DataDirPanel info={info} />

      <Section title="General">
        <Row label="Theme">
          <select className="input" value={cfg.general.theme}
            onChange={(e) => save({ ...cfg, general: { ...cfg.general, theme: e.target.value } })}>
            <option value="dark">Dark</option>
            <option value="light" disabled>Light (Phase 10)</option>
          </select>
        </Row>
        <Row label="Reduced motion">
          <Toggle checked={cfg.general.reduced_motion}
            onChange={(v) => save({ ...cfg, general: { ...cfg.general, reduced_motion: v } })} />
        </Row>
        <Row label="Allow optional online features">
          <Toggle checked={cfg.general.allow_network}
            onChange={(v) => save({ ...cfg, general: { ...cfg.general, allow_network: v } })} />
          <div className="text-[11px] text-slate-500">Off by default. Controls outbound requests for optional MLflow/HF/script integrations.</div>
        </Row>
      </Section>

      <Section title="Ingestion">
        <Row label="Chunk size (tokens)">
          <input type="number" min={64} max={4000} className="input"
            value={cfg.ingestion.chunk_tokens}
            onChange={(e) => save({ ...cfg, ingestion: { ...cfg.ingestion, chunk_tokens: Number(e.target.value) || 500 } })} />
        </Row>
        <Row label="Chunk overlap (tokens)">
          <input type="number" min={0} max={512} className="input"
            value={cfg.ingestion.chunk_overlap}
            onChange={(e) => save({ ...cfg, ingestion: { ...cfg.ingestion, chunk_overlap: Number(e.target.value) || 0 } })} />
        </Row>
        <Row label="Max file size (MB)">
          <input type="number" min={1} max={1024} className="input"
            value={cfg.ingestion.max_file_mb}
            onChange={(e) => save({ ...cfg, ingestion: { ...cfg.ingestion, max_file_mb: Number(e.target.value) || 64 } })} />
        </Row>
        <Row label="Embeddings enabled">
          <Toggle checked={cfg.ingestion.embedding_enabled}
            onChange={(v) => save({ ...cfg, ingestion: { ...cfg.ingestion, embedding_enabled: v } })} />
        </Row>
        <Row label="Embedding model id">
          <input className="input"
            value={cfg.ingestion.embedding_model}
            onChange={(e) => save({ ...cfg, ingestion: { ...cfg.ingestion, embedding_model: e.target.value } })} />
        </Row>
      </Section>

      <Section title="Agent">
        <Row label="Default permission">
          <select className="input" value={cfg.agent.default_permission}
            onChange={(e) => save({ ...cfg, agent: { ...cfg.agent, default_permission: e.target.value } })}>
            <option value="readonly">Read-only</option>
            <option value="workspace_write">Workspace write</option>
            <option value="confirm">Confirm before execution</option>
          </select>
        </Row>
        <Row label="Max steps per run">
          <input type="number" min={1} max={64} className="input"
            value={cfg.agent.max_steps}
            onChange={(e) => save({ ...cfg, agent: { ...cfg.agent, max_steps: Number(e.target.value) || 8 } })} />
        </Row>
        <Row label="Step timeout (ms)">
          <input type="number" min={1000} max={600000} step={500} className="input"
            value={cfg.agent.step_timeout_ms}
            onChange={(e) => save({ ...cfg, agent: { ...cfg.agent, step_timeout_ms: Number(e.target.value) || 30000 } })} />
        </Row>
        <Row label="File allowlist">
          <AllowlistEditor
            value={cfg.agent.file_allowlist}
            placeholder="C:\notes"
            help="Folders the agent's file.read / file.list tools may access. One per line. Empty = no access."
            onChange={(v) => save({ ...cfg, agent: { ...cfg.agent, file_allowlist: v } })}
          />
        </Row>
        <Row label="Shell allowlist">
          <AllowlistEditor
            value={cfg.agent.shell_allowlist}
            placeholder="git"
            help="Command basenames the agent's shell.run tool may invoke. One per line. Empty = no access."
            onChange={(v) => save({ ...cfg, agent: { ...cfg.agent, shell_allowlist: v } })}
          />
        </Row>
      </Section>

      <Section title="Audit log">
        <Row label="Retention (days)">
          <input type="number" min={0} max={3650} className="input"
            value={cfg.agent.audit_retention_days}
            onChange={(e) => save({ ...cfg, agent: { ...cfg.agent, audit_retention_days: Math.max(0, Number(e.target.value) || 0) } })} />
          <div className="text-[11px] text-slate-500">0 = keep forever.</div>
        </Row>
        <Row label="Prune on boot">
          <Toggle checked={cfg.agent.audit_prune_on_boot}
            onChange={(v) => save({ ...cfg, agent: { ...cfg.agent, audit_prune_on_boot: v } })} />
          <div className="text-[11px] text-slate-500">Apply retention when the app launches.</div>
        </Row>
      </Section>

      <Section title="About">
        <AboutPanel />
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="panel p-5">
      <div className="h-title mb-3">{title}</div>
      <div className="space-y-3">{children}</div>
    </div>
  );
}

function AboutPanel() {
  const [info, setInfo] = useState<{ name: string; version: string; data_dir: string; build_target: string; schema_version: number } | null>(null);
  useEffect(() => { appInfo().then(setInfo).catch(() => setInfo(null)); }, []);
  if (!info) return <div className="text-xs text-slate-500">Loading…</div>;
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-3">
        <div className="text-2xl font-semibold text-slate-100">{info.name}</div>
        <span className="chip text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10">v{info.version}</span>
      </div>
      <dl className="grid grid-cols-[120px_1fr] gap-y-1 text-sm">
        <dt className="text-slate-500">Data dir</dt>
        <dd className="font-mono text-slate-200 break-all">{info.data_dir}</dd>
        <dt className="text-slate-500">Schema</dt>
        <dd className="font-mono text-slate-200">v{info.schema_version}</dd>
        <dt className="text-slate-500">Target</dt>
        <dd className="font-mono text-slate-200">{info.build_target}</dd>
      </dl>
      <div className="text-[11px] text-slate-500">
        Local-first. No telemetry. No accounts. See <span className="font-mono">docs/USER_GUIDE.md</span> and <span className="font-mono">docs/BUILDING.md</span>.
      </div>
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[200px_1fr] items-start gap-4">
      <div className="h-sub pt-2">{label}</div>
      <div className="space-y-1">{children}</div>
    </div>
  );
}

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      className={`relative h-6 w-11 rounded-full border transition ${checked ? "border-cyan2-500/60 bg-cyan2-500/30" : "border-white/10 bg-ink-800"}`}
      onClick={() => onChange(!checked)}
    >
      <span className={`absolute top-0.5 h-5 w-5 rounded-full bg-slate-200 transition ${checked ? "left-5" : "left-0.5"}`} />
    </button>
  );
}

function AllowlistEditor({ value, onChange, placeholder, help }: { value: string[]; onChange: (v: string[]) => void; placeholder: string; help: string }) {
  const [text, setText] = useState(value.join("\n"));
  useEffect(() => { setText(value.join("\n")); }, [value]);
  return (
    <div className="space-y-1">
      <textarea
        className="input min-h-[5rem] font-mono text-[12px]"
        placeholder={placeholder}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onBlur={() => onChange(text.split(/\r?\n/).map((s) => s.trim()).filter((s) => s.length > 0))}
      />
      <div className="text-[11px] text-slate-500">{help}</div>
    </div>
  );
}

// -------------------- Data directory panel --------------------

function DataDirPanel({ info }: { info: { data_dir: string } }) {
  const [dirInfo, setDirInfo] = useState<DataDirInfo | null>(null);
  const [destination, setDestination] = useState<string>("");
  const [validation, setValidation] = useState<DataDirValidation | null>(null);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<DataDirMigrationReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    appDataDirInfo().then(setDirInfo).catch((e) => setError(String(e)));
  }, []);

  const onPick = async () => {
    try {
      const picked = await openDialog({ directory: true, multiple: false, title: "Pick a new NEXUS data directory" });
      if (typeof picked === "string") {
        setDestination(picked);
        const v = await appDataDirValidate(picked);
        setValidation(v);
      }
    } catch (e) { setError(String(e)); }
  };

  const onMigrate = async () => {
    if (!destination || !validation?.ok) return;
    if (!window.confirm(
      "NEXUS will copy its data to the new location and then need to be restarted. Continue?",
    )) return;
    setBusy(true);
    setError(null);
    try {
      const r = await appDataDirMigrate(destination);
      setReport(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const onRecheck = async () => {
    if (!destination) return;
    setValidation(await appDataDirValidate(destination));
  };

  const onOpen = async () => {
    try { await appDataDirOpen(); } catch (e) { setError(String(e)); }
  };

  return (
    <div className="panel p-5 space-y-3">
      <div className="h-title">Data directory</div>
      <p className="text-sm text-slate-400">
        NEXUS stores its database, imported documents, vector indexes, settings, and logs in a single
        directory. By default this is the per-user roaming location on the C: drive. You can move it
        to a different drive (e.g. D:\NEXUS-Data) to keep large files off the system drive.
      </p>
      <div className="rounded border border-white/5 bg-ink-800/40 p-3 font-mono text-[12px]">
        <div className="text-slate-500">Current</div>
        <div className="text-slate-200">{info.data_dir}</div>
        {dirInfo && (
          <div className="mt-1 text-[11px] text-slate-500">
            {dirInfo.is_portable ? "Portable mode" : dirInfo.is_default ? "Default roaming location" : "Custom location"}
            {dirInfo.total_bytes > 0 && (
              <span> · {humanBytes(dirInfo.free_bytes)} free of {humanBytes(dirInfo.total_bytes)}</span>
            )}
          </div>
        )}
      </div>
      <div className="flex gap-2">
        <button className="btn" onClick={onOpen}>Open in Explorer</button>
        <button className="btn" onClick={() => { setDestination(""); setValidation(null); setReport(null); }}>Move…</button>
      </div>
      {destination !== "" && (
        <div className="space-y-2">
          <label className="block">
            <div className="h-sub">Destination</div>
            <div className="flex gap-2">
              <input className="input flex-1 font-mono" value={destination} onChange={(e) => setDestination(e.target.value)} />
              <button className="btn" onClick={onPick}>Browse</button>
            </div>
          </label>
          <button className="btn" onClick={onRecheck}>Check writable</button>
          {validation && (
            <div className={`text-[12px] font-mono ${validation.ok ? "text-emerald2-400" : "text-rose2-400"}`}>
              {validation.ok ? "✓ Destination is writable." : `✗ ${validation.reason}`}
            </div>
          )}
          <button className="btn btn-primary" disabled={busy || !validation?.ok} onClick={onMigrate}>
            {busy ? "Copying…" : "Copy data and switch"}
          </button>
          <div className="text-[11px] text-slate-500">
            Copying copies every file from the current data directory to the new one. NEXUS will
            need to be restarted to open the new database. The original directory is preserved as a
            backup until you delete it manually.
          </div>
        </div>
      )}
      {report && (
        <div className="rounded border border-emerald2-500/30 bg-emerald2-500/10 p-3 text-[12px] text-emerald2-400 font-mono">
          Copied {report.files} files ({humanBytes(report.bytes)}).<br />
          From: {report.source}<br />
          To:&nbsp;&nbsp;{report.destination}<br />
          <span className="text-slate-300">Restart NEXUS to open the new location.</span>
        </div>
      )}
      {error && <div className="text-[12px] text-rose2-400 font-mono">{error}</div>}
    </div>
  );
}

function humanBytes(n: number): string {
  if (!n) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(v < 10 ? 2 : 1)} ${units[i]}`;
}
