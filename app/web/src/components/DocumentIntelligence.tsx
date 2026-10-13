// Document Intelligence panel. Phase J.
//
// A small companion panel: paste text (or load from a note) and
// the panel extracts any tables it finds, lets the user export the
// selected one to CSV/TSV, and offers a side-by-side diff of two
// revisions.

import { useEffect, useMemo, useState } from "react";
import {
  type DiffSummary,
  type ExtractedTable,
  documentDiff,
  documentExportCsv,
  documentExtractTables,
} from "../lib/intelligence";

type Tab = "tables" | "diff";

const SAMPLE = "| col1 | col2 |\n| ---- | ---- |\n| a    | b    |\n| c    | d    |\n";

export function DocumentIntelligence(props: { onClose?: () => void; initialText?: string }) {
  const [tab, setTab] = useState<Tab>("tables");
  const [text, setText] = useState(props.initialText ?? SAMPLE);
  const [tables, setTables] = useState<ExtractedTable[]>([]);
  const [selected, setSelected] = useState(0);
  const [csv, setCsv] = useState<string>("");
  const [delimiter, setDelimiter] = useState<"," | "\t">(",");
  const [error, setError] = useState<string | null>(null);
  const [textA, setTextA] = useState("");
  const [textB, setTextB] = useState("");
  const [diff, setDiff] = useState<DiffSummary | null>(null);

  useEffect(() => {
    if (props.initialText) setText(props.initialText);
  }, [props.initialText]);

  useEffect(() => {
    (async () => {
      try {
        const r = await documentExtractTables(text);
        setTables(r);
        setSelected(0);
      } catch (e) { setError(String(e)); }
    })();
  }, [text]);

  const current = useMemo(() => tables[selected], [tables, selected]);

  const onExport = async () => {
    if (!current) return;
    try {
      const r = await documentExportCsv({ text, index: selected, delimiter, include_header: true });
      setCsv(r.body);
    } catch (e) { setError(String(e)); }
  };

  const onCopy = async () => {
    if (!csv) return;
    try { await navigator.clipboard.writeText(csv); } catch {}
  };

  const onCompare = async () => {
    try { setDiff(await documentDiff(textA, textB)); } catch (e) { setError(String(e)); }
  };

  return (
    <div className="panel p-3 space-y-2 text-[12px] text-slate-300">
      <div className="flex items-center gap-2">
        <span className="h-sub">Document intelligence</span>
        <span className="kbd ml-auto" onClick={() => setTab(tab === "tables" ? "diff" : "tables")}>{tab}</span>
        {props.onClose && <span className="kbd" onClick={props.onClose}>close</span>}
      </div>
      {error && <div className="text-[11px] text-rose2-400 font-mono">{error}</div>}
      {tab === "tables" ? (
        <>
          <textarea
            className="input min-h-[10rem] font-mono text-[11px] leading-snug"
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder="Paste any text with pipe- or whitespace-aligned tables…"
          />
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[11px] text-slate-500">{tables.length} table{tables.length === 1 ? "" : "s"}</span>
            <select className="input" value={selected} onChange={(e) => setSelected(Number(e.target.value))}>
              {tables.map((_, i) => <option key={i} value={i}>table #{i + 1}</option>)}
            </select>
            <select className="input" value={delimiter} onChange={(e) => setDelimiter(e.target.value as "," | "\t")}>
              <option value=",">CSV (comma)</option>
              <option value="\t">TSV (tab)</option>
            </select>
            <button className="btn" onClick={onExport} disabled={!current}>Export</button>
            <button className="btn" onClick={onCopy} disabled={!csv}>Copy</button>
          </div>
          {current && (
            <div className="panel-soft p-2 max-h-60 overflow-auto">
              <table className="min-w-full text-[11px]">
                <thead><tr>{current.headers.map((h, i) => <th key={i} className="px-1 text-left text-cyan2-400">{h}</th>)}</tr></thead>
                <tbody>{current.rows.map((r, i) => <tr key={i}>{r.map((c, j) => <td key={j} className="px-1 border-t border-white/5">{c}</td>)}</tr>)}</tbody>
              </table>
            </div>
          )}
          {csv && (
            <details>
              <summary className="text-[11px] text-cyan2-400 cursor-pointer">CSV preview</summary>
              <pre className="mt-1 text-[10px] text-slate-400 whitespace-pre-wrap font-mono max-h-32 overflow-y-auto">{csv}</pre>
            </details>
          )}
        </>
      ) : (
        <>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
            <textarea className="input min-h-[8rem] font-mono text-[11px]" value={textA} onChange={(e) => setTextA(e.target.value)} placeholder="Version A" />
            <textarea className="input min-h-[8rem] font-mono text-[11px]" value={textB} onChange={(e) => setTextB(e.target.value)} placeholder="Version B" />
          </div>
          <button className="btn btn-primary" onClick={onCompare} disabled={!textA || !textB}>Compare</button>
          {diff && (
            <div className="panel-soft p-2 max-h-60 overflow-y-auto">
              <div className="text-[11px] text-slate-500 mb-1">+{diff.added} -{diff.removed} ={diff.same}</div>
              <pre className="text-[11px] font-mono whitespace-pre-wrap">
                {diff.lines.map((l, i) => (
                  <div key={i} className={l.kind === "add" ? "text-cyan2-400" : l.kind === "remove" ? "text-rose2-400" : "text-slate-400"}>
                    {l.kind === "add" ? "+ " : l.kind === "remove" ? "- " : "  "}{l.text || " "}
                  </div>
                ))}
              </pre>
            </div>
          )}
        </>
      )}
    </div>
  );
}