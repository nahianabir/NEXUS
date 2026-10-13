// Activity timeline used in the AI Assistant header. It shows a small list
// of recent events: "thinking", "retrieving", "generating", "cancelled",
// "error". The events are emitted from the Rust core over the chat stream
// or are derived from the assistant message status.

import type { JSX } from "react";

export type ActivityEvent = {
  id: string;
  label: string;
  detail?: string;
  variant: "info" | "think" | "gen" | "exec" | "warn" | "error" | "ok";
  at: number;
};

type Props = { events: ActivityEvent[]; running: boolean };

const VARIANT_CLASS: Record<ActivityEvent["variant"], string> = {
  info: "text-slate-400",
  think: "text-violet2-400",
  gen: "text-cyan2-400",
  exec: "text-amber2-400",
  warn: "text-amber2-400",
  error: "text-rose2-400",
  ok: "text-cyan2-400",
};

const DOT: Record<ActivityEvent["variant"], string> = {
  info: "bg-slate-500",
  think: "bg-violet2-500",
  gen: "bg-cyan2-500",
  exec: "bg-amber2-500",
  warn: "bg-amber2-500",
  error: "bg-rose2-500",
  ok: "bg-cyan2-500",
};

export function ActivityTimeline({ events, running }: Props): JSX.Element {
  return (
    <div className="rounded-xl border border-white/5 bg-ink-900/60 p-3 space-y-2">
      <div className="flex items-center gap-2">
        <span className={`h-1.5 w-1.5 rounded-full ${running ? "bg-cyan2-500 animate-breathe" : "bg-slate-600"}`} />
        <span className="h-sub">Activity</span>
      </div>
      {events.length === 0 && <div className="text-[11px] text-slate-500">No activity yet.</div>}
      <ul className="space-y-1.5 max-h-32 overflow-y-auto">
        {events.slice(-12).map((e) => (
          <li key={e.id} className="flex items-start gap-2 text-[11px]">
            <span className={`mt-1 h-1.5 w-1.5 rounded-full ${DOT[e.variant]}`} />
            <div className="flex-1 min-w-0">
              <div className={VARIANT_CLASS[e.variant]}>{e.label}</div>
              {e.detail && <div className="text-slate-500 truncate font-mono">{e.detail}</div>}
            </div>
            <span className="text-slate-600 font-mono">{new Date(e.at).toLocaleTimeString()}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}