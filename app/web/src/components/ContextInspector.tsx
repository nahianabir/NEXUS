// ContextInspector. Phase E2.
//
// An expandable panel that shows exactly which items the unified
// context builder gathered for the most recent chat turn. Items are
// grouped by `source` so the user can see at a glance whether the
// builder pulled in project notes, memories, vault chunks, or
// knowledge-graph edges. Each row carries its approximate token
// count, verification state, and a "open source" link where the
// source has a navigable target (vault document, graph entity).
//
// The component is purely a viewer: the data is precomputed by the
// `context_preview` Tauri command and passed in as a `ContextBundle`.

import { useMemo, useState } from "react";
import {
  type ContextBundle,
  type ContextItem,
  type ContextSource,
  contextPreview,
} from "../lib/context";
import { setActiveProject } from "../lib/projects";
import { kgGetEntity, kgEvidenceFor } from "../lib/kg";
import { vaultGetDocument } from "../lib/vault";

type Props = {
  bundle: ContextBundle | null;
  loading?: boolean;
  onJumpToMemory?: (id: string) => void;
  onJumpToProject?: (projectId: string) => void;
  onJumpToVaultDocument?: (documentId: string) => void;
  onJumpToGraphEntity?: (entityId: string) => void;
};

const SOURCE_ORDER: ContextSource[] = [
  "project_context",
  "manual_instruction",
  "native_memory",
  "mempalace_memory",
  "vault_document",
  "graph_entity",
  "graph_relationship",
];

const SOURCE_TINT: Record<ContextSource, string> = {
  project_context: "border-cyan2-500/30 text-cyan2-400",
  manual_instruction: "border-violet2-500/30 text-violet2-400",
  native_memory: "border-violet2-500/30 text-violet2-400",
  mempalace_memory: "border-violet2-500/20 text-violet2-300",
  vault_document: "border-amber2-500/30 text-amber2-400",
  graph_entity: "border-cyan2-500/30 text-cyan2-400",
  graph_relationship: "border-cyan2-500/20 text-cyan2-300",
};

/** Fallback tint applied when the wire returns a `source` value
 *  that is not in the lookup table. This can happen after a
 *  backward-incompatible schema change on the Rust side, a stale
 *  persisted record, or a new `ContextSource` variant that the
 *  frontend has not yet been updated for. Using a single neutral
 *  tint keeps the page from crashing; the source label still
 *  shows the raw string so the user can identify the gap. */
const FALLBACK_TINT = "border-slate-500/30 text-slate-400";

/** Always returns a known-good tint string. */
function tintFor(source: string): string {
  return (SOURCE_TINT as Record<string, string>)[source] ?? FALLBACK_TINT;
}

/** Splits a tint into [border, text] safely. Defensive against
 *  any future tint that is not exactly two space-separated
 *  tokens. */
function splitTint(tint: string): [string, string] {
  const parts = tint.split(" ");
  if (parts.length < 2) return [tint, "text-slate-400"];
  return [parts[0]!, parts[1]!];
}

const SOURCE_LABEL: Record<ContextSource, string> = {
  project_context: "Project",
  manual_instruction: "Instruction",
  native_memory: "Memory",
  mempalace_memory: "MemPalace",
  vault_document: "Vault",
  graph_entity: "Graph node",
  graph_relationship: "Graph edge",
};

export function ContextInspector(props: Props) {
  const { bundle, loading } = props;
  const [openBySource, setOpenBySource] = useState<Record<string, boolean>>({});
  const [openItems, setOpenItems] = useState<Record<string, boolean>>({});
  const [evidence, setEvidence] = useState<Record<string, { source_kind: string; excerpt: string | null; source_location: string | null }[]>>({});
  const [evidenceLoading, setEvidenceLoading] = useState<Record<string, boolean>>({});
  const [openDoc, setOpenDoc] = useState<{ id: string; title: string; path: string; mime: string | null } | null>(null);
  const [openEntity, setOpenEntity] = useState<{ id: string; name: string; type: string; description: string | null } | null>(null);

  const grouped = useMemo(() => {
    const map = new Map<ContextSource, ContextItem[]>();
    if (!bundle) return map;
    for (const it of bundle.items) {
      const list = map.get(it.source) ?? [];
      list.push(it);
      map.set(it.source, list);
    }
    // Stable source order.
    const ordered: [ContextSource, ContextItem[]][] = [];
    for (const src of SOURCE_ORDER) {
      const items = map.get(src);
      if (items && items.length > 0) ordered.push([src, items]);
    }
    // Anything we did not anticipate falls at the end.
    for (const [src, items] of map.entries()) {
      if (!SOURCE_ORDER.includes(src)) ordered.push([src, items]);
    }
    return new Map(ordered);
  }, [bundle]);

  if (!bundle) {
    return (
      <div className="panel-soft p-2 text-[11px] text-slate-500">
        <div className="flex items-center gap-2">
          <span className="text-slate-300">context</span>
          <span className="font-mono">no draft yet</span>
        </div>
      </div>
    );
  }

  const totalLabel = `~${bundle.approx_tokens} tok${bundle.approx_tokens === 1 ? "" : "ens"}`;
  const sourceCount = Object.keys(bundle.hits_by_source).length;

  const toggleSource = (s: string) => setOpenBySource((m) => ({ ...m, [s]: !m[s] }));
  const toggleItem = (k: string) => setOpenItems((m) => ({ ...m, [k]: !m[k] }));

  const loadEvidence = async (item: ContextItem) => {
    const relId = item.source_id;
    if (evidence[relId] || evidenceLoading[relId]) return;
    setEvidenceLoading((m) => ({ ...m, [relId]: true }));
    try {
      const rows = await kgEvidenceFor(relId);
      setEvidence((m) => ({
        ...m,
        [relId]: rows.map((r) => ({ source_kind: r.source_kind, excerpt: r.excerpt, source_location: r.source_location })),
      }));
    } catch {
      setEvidence((m) => ({ ...m, [relId]: [] }));
    } finally {
      setEvidenceLoading((m) => ({ ...m, [relId]: false }));
    }
  };

  const openVaultDoc = async (documentId: string) => {
    try {
      const d = await vaultGetDocument(documentId);
      setOpenDoc({ id: d.id, title: d.title, path: d.path, mime: d.mime_type });
    } catch (e) {
      setOpenDoc({ id: documentId, title: "(failed to load)", path: "", mime: null });
    }
  };

  const openGraphEntity = async (entityId: string) => {
    try {
      const e = await kgGetEntity(entityId);
      setOpenEntity({ id: e.id, name: e.canonical_name, type: e.entity_type, description: e.description });
    } catch {
      setOpenEntity({ id: entityId, name: "(failed to load)", type: "?", description: null });
    }
  };

  return (
    <div className="panel-soft p-2 text-[11px] text-slate-400">
      <div className="flex items-center gap-2 flex-wrap">
        <span className="text-slate-300">context</span>
        <span className="font-mono text-slate-500">
          {totalLabel} · {sourceCount} source{sourceCount === 1 ? "" : "s"}
          {bundle.truncated ? " · truncated" : ""}
        </span>
        {loading && <span className="font-mono text-slate-600 animate-breathe">refreshing…</span>}
        <span className="ml-auto font-mono text-slate-600 text-[10px]">
          searched: {bundle.sources_searched.map((s) => SOURCE_LABEL[s] ?? s).join(", ") || "(none)"}
        </span>
      </div>
      {bundle.items.length === 0 ? (
        <div className="mt-2 text-slate-500">No context items matched. (Try a longer draft, or enable the vault.)</div>
      ) : (
        <div className="mt-2 space-y-1.5">
          {Array.from(grouped.entries()).map(([source, items]) => {
            const open = openBySource[source] ?? true;
            const tint = tintFor(source);
            const [borderCls, textCls] = splitTint(tint);
            const label = (SOURCE_LABEL as Record<string, string>)[source] ?? source;
            return (
              <div key={source} className={`rounded-md border ${borderCls} bg-ink-900/40`}>
                <button
                  type="button"
                  className="w-full flex items-center gap-2 px-2 py-1.5 text-left"
                  onClick={() => toggleSource(source)}
                >
                  <span className={`font-mono text-[10px] ${textCls}`}>[{label}]</span>
                  <span className="text-slate-200">{items.length} item{items.length === 1 ? "" : "s"}</span>
                  <span className="ml-auto text-slate-500 font-mono text-[10px]">{open ? "▾" : "▸"}</span>
                </button>
                {open && (
                  <div className="px-2 pb-2 space-y-1">
                    {items.map((it, idx) => {
                      const k = `${source}:${it.source_id}:${idx}`;
                      const isOpen = openItems[k] ?? false;
                      return (
                        <div key={k} className="rounded border border-white/5 bg-ink-900/60 p-1.5">
                          <button
                            type="button"
                            className="w-full flex items-center gap-2 text-left"
                            onClick={() => toggleItem(k)}
                          >
                            <span className="text-slate-300 truncate flex-1" title={it.title ?? undefined}>
                              {it.title ?? "(untitled)"}
                              {it.source_label ? <span className="text-slate-500 font-mono"> · {it.source_label}</span> : null}
                            </span>
                            <span className="font-mono text-[10px] text-slate-500">~{it.approx_tokens} tok</span>
                            <span className="font-mono text-[10px] text-slate-500">{isOpen ? "▾" : "▸"}</span>
                          </button>
                          {isOpen && (
                            <div className="mt-1 space-y-1">
                              <div className="text-slate-300 whitespace-pre-wrap leading-relaxed">{it.excerpt}</div>
                              <div className="flex flex-wrap items-center gap-2 text-[10px] font-mono text-slate-500">
                                {it.verification && <span className="kbd">v: {it.verification}</span>}
                                {it.provenance && <span title={it.provenance}>src: {shortenProvenance(it.provenance)}</span>}
                                <span>score {it.score.toFixed(1)}</span>
                                <OpenSourceLink
                                  source={it.source}
                                  item={it}
                                  onMemory={props.onJumpToMemory}
                                  onProject={(id) => { void props.onJumpToProject?.(id); }}
                                  onVault={openVaultDoc}
                                  onGraph={openGraphEntity}
                                  onProjectFromLabel={async (label) => {
                                    // For project_context items, source_id is the
                                    // project id directly; the label is the
                                    // project name. Setting active project by id
                                    // would be ideal but the user might only
                                    // have the name. Try id first, then refresh.
                                    if (props.onJumpToProject) props.onJumpToProject(it.source_id);
                                    // Also clear any current active project as a
                                    // visual signal: we cannot guarantee a
                                    // round-trip from a label alone, so we just
                                    // log the label.
                                    console.debug("project context label:", label);
                                  }}
                                />
                                {it.source === "graph_relationship" && (
                                  <button
                                    type="button"
                                    className="kbd"
                                    onClick={(e) => { e.stopPropagation(); void loadEvidence(it); }}
                                  >
                                    {evidenceLoading[it.source_id] ? "loading…" : evidence[it.source_id] ? `evidence (${evidence[it.source_id].length})` : "load evidence"}
                                  </button>
                                )}
                              </div>
                              {it.source === "graph_relationship" && evidence[it.source_id] && evidence[it.source_id].length > 0 && (
                                <div className="mt-1 space-y-1 border-l-2 border-white/10 pl-2">
                                  {evidence[it.source_id].map((e, i) => (
                                    <div key={i} className="text-[10px] text-slate-500 font-mono">
                                      <span className="text-cyan2-400">[{e.source_kind}]</span>{" "}
                                      {e.source_location ? <span>{e.source_location}</span> : null}
                                      {e.excerpt ? <div className="text-slate-400 mt-0.5">{e.excerpt}</div> : null}
                                    </div>
                                  ))}
                                </div>
                              )}
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
      {openDoc && (
        <div className="mt-2 panel p-2 text-[11px] text-slate-300">
          <div className="flex items-center gap-2">
            <span className="text-cyan2-400 font-mono">vault document</span>
            <span className="truncate flex-1" title={openDoc.path}>{openDoc.title}</span>
            <button type="button" className="kbd" onClick={() => setOpenDoc(null)}>close</button>
          </div>
          <div className="mt-1 text-slate-500 font-mono text-[10px] truncate" title={openDoc.path}>{openDoc.path}</div>
          {openDoc.mime && <div className="text-slate-600 font-mono text-[10px]">{openDoc.mime}</div>}
        </div>
      )}
      {openEntity && (
        <div className="mt-2 panel p-2 text-[11px] text-slate-300">
          <div className="flex items-center gap-2">
            <span className="text-cyan2-400 font-mono">graph entity</span>
            <span className="truncate flex-1">{openEntity.name} <span className="text-slate-500">· {openEntity.type}</span></span>
            <button type="button" className="kbd" onClick={() => setOpenEntity(null)}>close</button>
          </div>
          {openEntity.description && <div className="mt-1 text-slate-400">{openEntity.description}</div>}
        </div>
      )}
    </div>
  );
}

function shortenProvenance(p: string): string {
  if (p.length <= 48) return p;
  return p.slice(0, 45) + "…";
}

function OpenSourceLink(props: {
  source: ContextSource;
  item: ContextItem;
  onMemory?: (id: string) => void;
  onProject: (id: string) => void | Promise<void>;
  onVault: (id: string) => void | Promise<void>;
  onGraph: (id: string) => void | Promise<void>;
  onProjectFromLabel: (label: string) => void;
}) {
  const { source, item } = props;
  if (source === "vault_document") {
    // The source_id is the chunk id; the document id is encoded in the
    // provenance. Extract it if present.
    const docId = extractDocId(item.provenance);
    if (!docId) return null;
    return (
      <button type="button" className="kbd" onClick={(e) => { e.stopPropagation(); void props.onVault(docId); }}>
        open source
      </button>
    );
  }
  if (source === "graph_entity") {
    return (
      <button type="button" className="kbd" onClick={(e) => { e.stopPropagation(); void props.onGraph(item.source_id); }}>
        open source
      </button>
    );
  }
  if (source === "graph_relationship") {
    return (
      <button type="button" className="kbd" onClick={(e) => { e.stopPropagation(); void props.onGraph(item.source_id); }}>
        open source
      </button>
    );
  }
  if (source === "native_memory" || source === "mempalace_memory") {
    if (!props.onMemory) return null;
    return (
      <button type="button" className="kbd" onClick={(e) => { e.stopPropagation(); props.onMemory!(item.source_id); }}>
        open source
      </button>
    );
  }
  if (source === "project_context") {
    // source_id is the project id (for the main project block) or the
    // note id (for project notes). For the main block we offer "set
    // active project"; for notes we just point to the project.
    return (
      <button
        type="button"
        className="kbd"
        onClick={(e) => { e.stopPropagation(); void setActiveProject(item.source_id).then(() => props.onProject(item.source_id)); }}
      >
        set active
      </button>
    );
  }
  return null;
}

function extractDocId(provenance: string | null): string | null {
  if (!provenance) return null;
  // Format: "chunks:<document_id>:<ordinal>;...". Defensive:
  // an empty provenance (which can happen for a freshly-inserted
  // record) should not crash the call site.
  const trimmed = provenance.trim();
  if (trimmed === "") return null;
  const after = trimmed.split(":", 3);
  if (after.length < 3) return null;
  if (after[0] !== "chunks") return null;
  const docId = (after[1] ?? "").trim();
  return docId === "" ? null : docId;
}

// Re-export the context builder entry point so pages can refresh on
// demand (e.g. when a memory is approved and the bundle should be
// rebuilt).
export { contextPreview };

// Wrap the inspector in a small error boundary so a malformed
// context bundle (e.g. an unknown `source` value after a schema
// upgrade) cannot take down the entire Assistant page. The
// route-level boundary in App.tsx is the last line of defence;
// this boundary is the first.
import { ErrorBoundary } from "./ErrorBoundary";
export function ContextInspectorSafe(props: Props) {
  return (
    <ErrorBoundary title="Context inspector" fallbackRoute="/assistant">
      <ContextInspector {...props} />
    </ErrorBoundary>
  );
}
