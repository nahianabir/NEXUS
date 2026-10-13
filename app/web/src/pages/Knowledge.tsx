// Knowledge Vault — Phase 4.
//
// Three panels:
//   1. Sources: register a directory or a single file, kick off ingest,
//      watch progress live.
//   2. Documents: list indexed files, see chunk count + status, delete.
//   3. Search: FTS5 + vector hybrid retrieval; the same backend the
//      Assistant uses for citation-grounded answers.
//
// Embeddings are opt-in. The user creates an "embedding" model profile in
// the Model Manager, then clicks "Backfill embeddings" here.

import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  type DocumentRow,
  type IngestProgress,
  type SearchHit,
  type SearchResult,
  type SourceRow,
  type VaultStats,
  onVaultBackfill,
  onVaultDone,
  onVaultError,
  onVaultProgress,
  vaultDeleteDocument,
  vaultDeleteSource,
  vaultEmbedBackfill,
  vaultIngestSource,
  vaultListDocuments,
  vaultListSources,
  vaultSearch,
  vaultStats,
} from "../lib/vault";
import { type ModelProfile, modelsList, modelsTestEmbedding } from "../lib/providers";
import { usePromptDialog } from "../hooks/usePromptDialog";

export function KnowledgePage() {
  const { confirm: confirmModal } = usePromptDialog();
  const [sources, setSources] = useState<SourceRow[]>([]);
  const [documents, setDocuments] = useState<DocumentRow[]>([]);
  const [stats, setStats] = useState<VaultStats | null>(null);
  const [profiles, setProfiles] = useState<ModelProfile[]>([]);
  const [progress, setProgress] = useState<IngestProgress | null>(null);
  const [lastResult, setLastResult] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResult, setSearchResult] = useState<SearchResult | null>(null);
  const [searching, setSearching] = useState(false);
  // Bumped on every new search. The async search only
  // commits its result to state when its captured
  // `searchSeq` still matches `searchSeqRef.current`. A
  // newer search that finished later cannot overwrite the
  // active result, and an older search that finished after
  // a newer one is dropped. This is the user-visible fix
  // for the "results stacking" bug.
  const searchSeqRef = useRef(0);
  const [selectedEmbeddingProfile, setSelectedEmbeddingProfile] = useState<string>("");
  const [backfillStatus, setBackfillStatus] = useState<string | null>(null);
  const [detectedDim, setDetectedDim] = useState<number | null>(null);
  const unlistens = useRef<(() => void)[]>([]);

  const refresh = async () => {
    const [s, d, st] = await Promise.all([vaultListSources(), vaultListDocuments(200), vaultStats()]);
    setSources(s);
    setDocuments(d);
    setStats(st);
  };

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
    modelsList().then(setProfiles).catch(() => {});
    let dead = false;
    (async () => {
      unlistens.current.push(await onVaultProgress((p) => { if (!dead) setProgress(p); }));
      unlistens.current.push(await onVaultDone((r) => {
        if (dead) return;
        setProgress(null);
        setLastResult(`Indexed ${r.result.files_indexed} files (${r.result.chunks} chunks${r.result.embedded_chunks ? `, ${r.result.embedded_chunks} embedded` : ""}) in source ${r.result.source_id.slice(-8)}.`);
        refresh().catch(() => {});
      }));
      unlistens.current.push(await onVaultError((err) => {
        if (dead) return;
        setProgress(null);
        setError(err.error);
      }));
      unlistens.current.push(await onVaultBackfill((b) => {
        if (dead) return;
        setBackfillStatus(`Embedded ${b.done}/${b.total}`);
      }));
    })();
    return () => {
      dead = true;
      unlistens.current.forEach((f) => f());
      unlistens.current = [];
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onAddDirectory = async () => {
    try {
      const picked = await open({ directory: true, multiple: false, title: "Choose a directory to index" });
      if (!picked || typeof picked !== "string") return;
      await vaultIngestSource({ path: picked, kind: "directory", display_name: basename(picked) });
      setLastResult(`Started indexing ${picked}…`);
      setError(null);
      refresh().catch(() => {});
    } catch (e) {
      setError(String(e));
    }
  };

  const onAddFile = async () => {
    try {
      const picked = await open({ multiple: false, title: "Choose a file to index" });
      if (!picked || typeof picked !== "string") return;
      await vaultIngestSource({ path: picked, kind: "file", display_name: basename(picked) });
      setLastResult(`Started indexing ${picked}…`);
      setError(null);
      refresh().catch(() => {});
    } catch (e) {
      setError(String(e));
    }
  };

  const onReindex = async (s: SourceRow) => {
    try {
      await vaultIngestSource({ path: s.root_path, kind: s.kind as any, display_name: s.display_name });
      setLastResult(`Re-indexing ${s.display_name}…`);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const onDeleteSource = async (s: SourceRow) => {
    const ok = await confirmModal({
      title: "Delete source",
      description: `Delete source "${s.display_name}" and all of its documents? This cannot be undone.`,
      submitLabel: "Delete source",
      destructive: true,
    });
    if (!ok) return;
    await vaultDeleteSource(s.id);
    refresh().catch(() => {});
  };

  const onDeleteDocument = async (d: DocumentRow) => {
    const ok = await confirmModal({
      title: "Remove document",
      description: `Remove "${d.title}" from the vault? The original file on disk is not touched.`,
      submitLabel: "Remove",
      destructive: true,
    });
    if (!ok) return;
    await vaultDeleteDocument(d.id);
    refresh().catch(() => {});
  };

  // Run a new search and replace the active result list.
  // The seq-guard ensures a slow older request cannot
  // overwrite a newer result. A search that returns 0 hits
  // still REPLACES the previous list (we do not leave the
  // stale results on screen) — this is the "results
  // stacking" bug fix the user reported.
  const onSearch = async () => {
    const q = searchQuery.trim();
    if (!q) {
      // Clearing the query also clears the result list,
      // explicitly. The user must not be left looking at
      // results from a previous search.
      searchSeqRef.current += 1;
      setSearchResult(null);
      return;
    }
    const mySeq = searchSeqRef.current + 1;
    searchSeqRef.current = mySeq;
    setSearching(true);
    setError(null);
    // Optimistically clear the prior result list so the
    // user sees a clear "no results yet" / "searching…"
    // state instead of stale rows.
    setSearchResult(null);
    try {
      const res = await vaultSearch({ query: q, limit: 12 });
      // Drop the result if a newer search has started in
      // the meantime. This is the only way to defeat a
      // network race where the older request resolves
      // after the newer one.
      if (searchSeqRef.current !== mySeq) return;
      setSearchResult(res);
    } catch (e) {
      if (searchSeqRef.current !== mySeq) return;
      setError(String(e));
      setSearchResult(null);
    } finally {
      if (searchSeqRef.current === mySeq) {
        setSearching(false);
      }
    }
  };

  const onBackfill = async () => {
    if (!selectedEmbeddingProfile) return;
    setBackfillStatus("Starting…");
    try {
      const done = await vaultEmbedBackfill(selectedEmbeddingProfile, 5000);
      setBackfillStatus(`Done. Embedded ${done} chunks.`);
      refresh().catch(() => {});
    } catch (e) {
      setBackfillStatus(`Failed: ${e}`);
    }
  };

  // Probe the selected embedding model with a short string
  // and remember its dimension. The Rust core then routes
  // vectors to the right `chunk_vectors_<dim>` table.
  const onDetectDim = async () => {
    if (!selectedEmbeddingProfile) return;
    const profile = profiles.find((p) => p.id === selectedEmbeddingProfile);
    if (!profile) return;
    setBackfillStatus("Probing model…");
    try {
      const r = await modelsTestEmbedding(profile.model_id);
      setDetectedDim(r.dimension);
      setBackfillStatus(null);
      // Best-effort: persist the dimension on the profile so
      // the user does not have to re-probe next time.
      try {
        await (await import("../lib/providers")).modelsUpsert({
          ...profile,
          dimension: r.dimension,
          model_identity: `${profile.provider}::${profile.model_id}`,
        });
        const fresh = await modelsList();
        setProfiles(fresh);
      } catch {
        // Persisting the dimension is best-effort; the
        // backfill itself can still discover the dimension
        // from the first batch.
      }
    } catch (e) {
      setBackfillStatus(`Failed: ${e}`);
    }
  };

  const embeddingProfiles = useMemo(() => profiles.filter((p) => p.role === "embedding" && p.enabled), [profiles]);
  const progressPct = progress && progress.files_seen > 0
    ? Math.min(100, Math.round((progress.files_done / Math.max(1, progress.files_seen)) * 100))
    : progress?.stage === "scanning" ? 5 : 0;

  return (
    <div className="p-6 max-w-7xl space-y-6">
      <div className="panel p-5">
        <div className="flex items-start gap-3">
          <div className="flex-1">
            <div className="h-title">Knowledge Vault</div>
            <p className="mt-1 text-sm text-slate-400">
              Ingest directories or files. NEXUS extracts text, splits into 500-token chunks
              (80-token overlap), SHA-256 deduplicates, indexes FTS5, and (optionally) embeds
              with the configured embedding model. The Assistant uses this for citations.
            </p>
          </div>
          <div className="flex gap-2">
            <button className="btn" onClick={onAddFile}>Add file</button>
            <button className="btn btn-primary" onClick={onAddDirectory}>Add directory</button>
            <button className="btn" onClick={() => refresh().catch((e) => setError(String(e)))}>Refresh</button>
          </div>
        </div>
        {stats && (
          <div className="mt-4 grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
            <Stat label="Sources" value={stats.sources} />
            <Stat label="Documents" value={stats.documents} accent={stats.documents > 0 ? "ok" : "muted"} />
            <Stat label="Chunks" value={stats.chunks} accent={stats.chunks > 0 ? "ok" : "muted"} />
            <Stat label="Vectors" value={stats.vector_rows} accent={stats.vector_rows > 0 ? "ok" : "muted"} hint={stats.vector_rows === 0 ? "embeddings not run" : `${stats.embedding_profiles} embedding model${stats.embedding_profiles === 1 ? "" : "s"} configured`} />
          </div>
        )}
      </div>

      {progress && (
        <div className="panel p-4">
          <div className="flex items-center gap-3">
            <div className="h-title flex-1">
              {progress.stage} · {progress.message}
            </div>
            <div className="text-xs text-slate-500 font-mono">
              {progress.files_done}/{progress.files_seen} files · {progress.chunks} chunks
            </div>
          </div>
          <div className="mt-3 h-2 rounded-full bg-ink-800 overflow-hidden">
            <div
              className="h-full bg-gradient-to-r from-cyan2-500 to-violet2-500 transition-all"
              style={{ width: `${progressPct}%` }}
            />
          </div>
        </div>
      )}

      {lastResult && !progress && <div className="panel-soft p-3 text-xs text-cyan2-400 font-mono">{lastResult}</div>}
      {error && <div className="panel-soft p-3 text-xs text-rose2-400 font-mono">{error}</div>}

      <section className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        <div className="panel p-4">
          <div className="h-title">Sources</div>
          <div className="mt-3 space-y-2">
            {sources.length === 0 && <div className="text-xs text-slate-500">No sources yet. Add a directory or a file to get started.</div>}
            {sources.map((s) => (
              <div key={s.id} className="rounded-lg border border-white/5 bg-ink-800/50 p-3">
                <div className="flex items-center gap-2">
                  <div className="font-mono text-[12px] text-slate-200 truncate flex-1">{s.root_path}</div>
                  <span className="chip">{s.kind}</span>
                </div>
                <div className="mt-1 text-[11px] text-slate-500 font-mono">
                  {s.display_name} · {s.last_indexed_at ? `indexed ${new Date(s.last_indexed_at).toLocaleString()}` : "never indexed"}
                </div>
                <div className="mt-2 flex gap-2">
                  <button className="btn" onClick={() => onReindex(s)}>Re-index</button>
                  <button className="btn" onClick={() => onDeleteSource(s)}>Delete</button>
                </div>
              </div>
            ))}
          </div>

          <div className="mt-6 border-t border-white/5 pt-4">
            <div className="h-title">Backfill embeddings</div>
            <p className="text-xs text-slate-500 mt-1">
              Embed every chunk that does not yet have a vector. Requires an embedding model profile. Each
              profile remembers its vector dimension so 384 and 768-dim collections stay separate.
            </p>
            <div className="mt-3 flex flex-col md:flex-row gap-2 md:items-end">
              <label className="flex-1">
                <div className="h-sub">Embedding model</div>
                <select
                  className="input"
                  value={selectedEmbeddingProfile}
                  onChange={(e) => {
                    setSelectedEmbeddingProfile(e.target.value);
                    setDetectedDim(null);
                  }}
                >
                  <option value="">(choose…)</option>
                  {embeddingProfiles.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.display_name} · {p.model_id}
                      {p.dimension ? ` (${p.dimension}D)` : ""}
                    </option>
                  ))}
                </select>
              </label>
              <button
                className="btn"
                disabled={!selectedEmbeddingProfile}
                onClick={onDetectDim}
                title="Probe the model with a short string and remember its dimension"
              >
                Detect dimension
              </button>
              <button className="btn btn-primary" disabled={!selectedEmbeddingProfile} onClick={onBackfill}>Backfill</button>
            </div>
            {detectedDim != null && (
              <div className="mt-2 text-[11px] text-emerald2-400 font-mono">
                Detected dimension: {detectedDim}
              </div>
            )}
            {backfillStatus && <div className="mt-2 text-[11px] text-slate-500 font-mono">{backfillStatus}</div>}
            {embeddingProfiles.length === 0 && (
              <div className="mt-2 text-[11px] text-amber2-400 font-mono">
                No embedding profiles. Add one in the Model Manager.
              </div>
            )}
          </div>
        </div>

        <div className="panel p-4">
          <div className="h-title">Search</div>
          <p className="text-xs text-slate-500 mt-1">
            Hybrid FTS5 + vector. Each hit is a single chunk with its document path and a relevance score.
          </p>
          <div className="mt-3 flex gap-2">
            <input
              className="input flex-1"
              placeholder="e.g. Why did the author pick chunked retrieval?"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") onSearch(); }}
            />
            <button className="btn btn-primary" disabled={searching || !searchQuery.trim()} onClick={onSearch}>
              {searching ? "Searching…" : "Search"}
            </button>
          </div>
          <div className="mt-3 space-y-2 max-h-[26rem] overflow-y-auto pr-1">
            {searchResult === null && <div className="text-xs text-slate-500">No results yet.</div>}
            {searchResult && searchResult.hits.length === 0 && <div className="text-xs text-slate-500">No hits.</div>}
            {searchResult?.hits.map((h, i) => <SearchHitView key={h.chunk.id} hit={h} index={i + 1} />)}
          </div>
        </div>
      </section>

      <section className="panel p-4">
        <div className="flex items-center justify-between">
          <div className="h-title">Documents</div>
          <div className="text-xs text-slate-500">{documents.length} indexed</div>
        </div>
        <div className="mt-3 overflow-x-auto">
          <table className="w-full text-sm">
            <thead className="text-[10px] uppercase tracking-wider text-slate-500">
              <tr>
                <th className="text-left py-2 px-2">Title</th>
                <th className="text-left py-2 px-2">Path</th>
                <th className="text-right py-2 px-2">Size</th>
                <th className="text-left py-2 px-2">Status</th>
                <th className="text-left py-2 px-2">Hash</th>
                <th className="text-right py-2 px-2">Actions</th>
              </tr>
            </thead>
            <tbody>
              {documents.length === 0 && (
                <tr><td colSpan={6} className="py-6 text-center text-xs text-slate-500">No documents yet.</td></tr>
              )}
              {documents.map((d) => (
                <tr key={d.id} className="border-t border-white/5 hover:bg-white/5">
                  <td className="py-2 px-2 font-medium text-slate-200">{d.title}</td>
                  <td className="py-2 px-2 font-mono text-[11px] text-slate-400 max-w-md truncate" title={d.path}>{d.path}</td>
                  <td className="py-2 px-2 text-right font-mono text-[11px] text-slate-400">{humanSize(d.size_bytes)}</td>
                  <td className="py-2 px-2"><StatusChip status={d.status} /></td>
                  <td className="py-2 px-2 font-mono text-[10px] text-slate-500">
                    {d.content_hash ? `${d.content_hash.slice(0, 10)}…` : <span className="text-slate-600">no hash</span>}
                  </td>
                  <td className="py-2 px-2 text-right">
                    <button className="btn" onClick={() => onDeleteDocument(d)}>Remove</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}

function Stat({ label, value, accent, hint }: { label: string; value: number; accent?: "ok" | "muted"; hint?: string }) {
  const tone = accent === "ok" ? "text-cyan2-300" : "text-slate-300";
  return (
    <div className="rounded-lg border border-white/5 bg-ink-800/40 p-3">
      <div className="text-[10px] uppercase tracking-wide text-slate-500">{label}</div>
      <div className={`mt-1 text-2xl font-semibold ${tone}`}>{value.toLocaleString()}</div>
      {hint && <div className="mt-0.5 text-[10px] text-slate-500">{hint}</div>}
    </div>
  );
}

function StatusChip({ status }: { status: string }) {
  if (status === "indexed") return <span className="chip text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10">indexed</span>;
  if (status === "failed") return <span className="chip text-rose2-400 border-rose2-400/40 bg-rose2-400/10">failed</span>;
  if (status === "pending") return <span className="chip text-amber2-400 border-amber2-400/40 bg-amber2-400/10">pending</span>;
  return <span className="chip">{status}</span>;
}

function SearchHitView({ hit, index }: { hit: SearchHit; index: number }) {
  return (
    <div className="rounded-lg border border-white/5 bg-ink-800/40 p-3">
      <div className="flex items-center gap-2">
        <span className="kbd">[{index}]</span>
        <div className="font-medium text-slate-200 truncate flex-1">{hit.chunk.document_title}</div>
        <span className="chip">{hit.source}</span>
        <span className="text-[10px] text-slate-500 font-mono">score {hit.score.toFixed(3)}</span>
      </div>
      <div className="mt-1 text-[11px] font-mono text-slate-500 truncate" title={hit.chunk.document_path}>{hit.chunk.document_path}</div>
      <pre className="mt-2 text-[12px] text-slate-300 whitespace-pre-wrap font-mono leading-relaxed max-h-32 overflow-y-auto">{hit.chunk.text}</pre>
    </div>
  );
}

function basename(p: string): string {
  return p.replace(/[\\/]+$/, "").split(/[\\/]/).pop() || p;
}

function humanSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}