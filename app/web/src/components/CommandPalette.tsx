// Command palette: fuzzy search over sidebar routes and a fixed list of
// cross-app actions. Esc closes; arrow keys move; Enter executes.

import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import type { SidebarItem } from "../lib/ipc";

type Props = {
  open: boolean;
  items: SidebarItem[];
  onClose: () => void;
};

type Entry =
  | { kind: "nav"; id: string; label: string; route: string; icon: string }
  | { kind: "action"; id: string; label: string; hint: string; action: () => void };

export function CommandPalette({ open, items, onClose }: Props) {
  const nav = useNavigate();
  const [query, setQuery] = useState("");
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setCursor(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const entries: Entry[] = useMemo(() => {
    const navEntries: Entry[] = items.map((i) => ({
      kind: "nav",
      id: i.id,
      label: i.label,
      route: i.route,
      icon: i.icon,
    }));
    const actions: Entry[] = [
      { kind: "action", id: "new-conversation", label: "New conversation", hint: "Assistant", action: () => nav("/assistant?new=1") },
      { kind: "action", id: "open-models", label: "Open Model Manager", hint: "Settings", action: () => nav("/models") },
      { kind: "action", id: "open-monitor", label: "Open System Monitor", hint: "Diagnostics", action: () => nav("/monitor") },
    ];
    return [...navEntries, ...actions];
  }, [items, nav]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter((e) => e.label.toLowerCase().includes(q) || (e.kind === "action" && e.hint.toLowerCase().includes(q)));
  }, [query, entries]);

  useEffect(() => { setCursor(0); }, [query]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-28 bg-ink-950/60 backdrop-blur-xs" onClick={onClose}>
      <div
        className="panel w-[640px] max-w-[92vw] overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          className="w-full bg-transparent px-4 py-3 text-sm placeholder:text-slate-500 focus:outline-none"
          placeholder="Type a module or action…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") { e.preventDefault(); setCursor((c) => Math.min(c + 1, filtered.length - 1)); }
            else if (e.key === "ArrowUp") { e.preventDefault(); setCursor((c) => Math.max(c - 1, 0)); }
            else if (e.key === "Enter") {
              const sel = filtered[cursor];
              if (!sel) return;
              if (sel.kind === "nav") nav(sel.route);
              else sel.action();
              onClose();
            } else if (e.key === "Escape") {
              onClose();
            }
          }}
        />
        <div className="max-h-80 overflow-y-auto border-t border-white/5">
          {filtered.length === 0 && (
            <div className="px-4 py-6 text-sm text-slate-500">No matches.</div>
          )}
          {filtered.map((e, i) => (
            <button
              key={e.id}
              className={`w-full text-left px-4 py-2 text-sm flex items-center gap-3 ${i === cursor ? "bg-cyan2-500/10 text-cyan2-400" : "text-slate-300 hover:bg-white/5"}`}
              onMouseEnter={() => setCursor(i)}
              onClick={() => {
                if (e.kind === "nav") nav(e.route);
                else e.action();
                onClose();
              }}
            >
              <span className="text-slate-500 w-16 text-[11px] uppercase tracking-wide">{e.kind}</span>
              <span className="flex-1 truncate">{e.label}</span>
              {e.kind === "action" && <span className="text-[11px] text-slate-500">{e.hint}</span>}
            </button>
          ))}
        </div>
        <div className="border-t border-white/5 px-3 py-1.5 text-[11px] text-slate-500 flex items-center gap-2">
          <span className="kbd">↑↓</span> move
          <span className="kbd">Enter</span> run
          <span className="kbd">Esc</span> close
        </div>
      </div>
    </div>
  );
}
