// AI Assistant. Phase 3 ships the streaming conversation UI with Markdown
// rendering, an activity timeline, and conversation list / rename / archive
// / delete. Citations land in Phase 4; the source-strip placeholder is wired
// to message_sources as soon as retrieval is enabled.

import { useEffect, useMemo, useRef, useState } from "react";
import {
  type ChatCitationsEvent,
  type ChatDeltaEvent,
  type Conversation,
  type Message,
  archiveConversation,
  chatSend,
  createConversation,
  deleteConversation,
  listConversations,
  listMessages,
  onChatCitations,
  onChatDelta,
  onChatReasoning,
  onChatTitleUpdated,
  renameConversation,
  setSystemPrompt,
  // Phase P4 — Bug E, L, F. The chat request registry
  // is the source of truth for in-flight requests; the
  // memory-save path is a deterministic local service.
  chatRequestListActive,
  chatRequestCancel,
  onMemorySaved,
  type ActiveRequestSnapshot,
} from "../lib/chat";
import { cancelInflight, modelsList, type ModelProfile } from "../lib/providers";
import { MarkdownView } from "../components/MarkdownView";
import { ActivityTimeline, type ActivityEvent } from "../components/ActivityTimeline";

// Helper: find the most recent activity entry that represents an
// in-flight "Sending" / "Preparing" / "Waiting" state. Used to
// transition that entry to a terminal state when the stream
// completes (done / error / cancelled) so the activity panel
// never shows a stale "Sending" record after the response
// finishes. Returns -1 if no such entry exists.
function humanSize(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function findLastSending(events: ActivityEvent[]): number {
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const e = events[i];
    if (
      e.label === "Sending" ||
      e.label === "Waiting for model" ||
      e.label === "Generating" ||
      e.label.startsWith("Preparing")
    ) {
      return i;
    }
  }
  return -1;
}

// Derive a short, human-readable conversation title from a free-form
// user message. Strips surrounding whitespace, normalises internal
// whitespace, trims trailing punctuation, and clips to a sensible
// length. Falls back to "New conversation" if the input is empty
// or otherwise unusable.
function deriveTitle(message: string, maxLen = 48): string {
  const cleaned = message
    .replace(/\s+/g, " ")
    .replace(/[\u0000-\u001f\u007f]/g, "")
    .trim();
  if (!cleaned) return "New conversation";
  // Strip leading bullets / list markers.
  const stripped = cleaned.replace(/^([-*•·\d+.)]+\s+)+/, "");
  // Trim trailing punctuation.
  const trimmed = stripped.replace(/[\s.,;:!?]+$/, "");
  if (trimmed.length <= maxLen) return trimmed;
  // Clip on a word boundary when possible.
  const clip = trimmed.slice(0, maxLen);
  const lastSpace = clip.lastIndexOf(" ");
  if (lastSpace > maxLen * 0.6) return clip.slice(0, lastSpace) + "…";
  return clip + "…";
}
import { NexusGlyph } from "../components/NexusGlyph";
import { type Citation, vaultListCitations } from "../lib/vault";
import { type Project, getActiveProject, setActiveProject } from "../lib/projects";
import { type ContextBundle, contextPreview } from "../lib/context";
import { ContextInspectorSafe as ContextInspector } from "../components/ContextInspector";
import { usePromptDialog } from "../hooks/usePromptDialog";
import { type AttachmentPayload, chatReadAttachment } from "../lib/attachments";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

export function AssistantPage() {
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [active, setActive] = useState<Conversation | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [draft, setDraft] = useState("");
  const [running, setRunning] = useState(false);
  const [elapsedMs, setElapsedMs] = useState<number>(0);
  const [lastPhase, setLastPhase] = useState<"waiting" | "generating" | "finalizing" | null>(null);
  const [ttftMs, setTtftMs] = useState<number | null>(null);
  const startedAt = useRef<number | null>(null);
  // Phase P4 — Bug L: stable request id of the in-flight
  // chat request, or null when nothing is in flight. Set
  // when the local `chatSend(...)` promise resolves
  // (the backend already emitted the initial delta with
  // the request id before the promise resolves). Used
  // to filter late `nexus:chat.delta` events from a
  // stale request so they cannot overwrite the state of
  // a newer active request.
  const activeRequestId = useRef<string | null>(null);
  // Attachment list for the current draft. Cleared on send.
  const [attachments, setAttachments] = useState<AttachmentPayload[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [reasoning, setReasoning] = useState<string>("");
  const [activity, setActivity] = useState<ActivityEvent[]>([]);
  const [profiles, setProfiles] = useState<ModelProfile[]>([]);
  const [activeProfileId, setActiveProfileId] = useState<string | null>(null);
  const [useVault, setUseVault] = useState(true);
  const [citations, setCitations] = useState<Record<string, Citation[]>>({});
  const [activeProject, setActiveProjectState] = useState<Project | null>(null);
  const [contextBundle, setContextBundle] = useState<ContextBundle | null>(null);
  const [contextLoading, setContextLoading] = useState(false);
  const unlistens = useRef<(() => void)[]>([]);
  // The chat-delta listener is registered once in a useEffect with
  // empty deps so the closure captures a stale `active`. To make
  // every event see the *current* `active` we mirror it into a
  // ref and read from there in the listener. The same goes for
  // `ttftMs` (which we want to be settable from inside the
  // listener without recreating the listener on every change).
  const activeRef = useRef<Conversation | null>(null);
  useEffect(() => { activeRef.current = active; }, [active]);
  // Watchdog: when a chat request is in flight, schedule a
  // timeout that transitions the UI out of the "running" state
  // if no `done`/`error`/`cancelled` event arrives in time. The
  // Rust side also has its own 180s timeout, but the JS side
  // needs a comparable bound to recover from a lost event.
  const watchdogRef = useRef<number | null>(null);
  function clearWatchdog() {
    if (watchdogRef.current !== null) {
      window.clearTimeout(watchdogRef.current);
      watchdogRef.current = null;
    }
  }
  const { prompt: promptModal, confirm: confirmModal } = usePromptDialog();

  const refresh = async () => {
    const list = await listConversations(false);
    setConversations(list);
    if (!active && list.length > 0) setActive(list[0]);
  };

  useEffect(() => {
    refresh().catch((e) => setError(String(e)));
    modelsList().then(setProfiles).catch(() => setProfiles([]));
    getActiveProject().then(setActiveProjectState).catch(() => setActiveProjectState(null));
    let dead = false;
    (async () => {
      unlistens.current.push(await onChatDelta((ev) => {
        // Read the *current* active conversation from the ref
        // (see activeRef above). This makes event filtering
        // resilient to the user switching conversations while a
        // request is in flight.
        const currentActive = activeRef.current;
        if (currentActive && ev.conversation_id !== currentActive.id) return;
        handleDelta(ev);
      }));
      unlistens.current.push(await onChatReasoning((ev) => {
        if (dead) return;
        setReasoning((r) => (r + ev.text).slice(-2000));
        setActivity((a) => [...a, { id: `r-${Date.now()}-${Math.random().toString(16).slice(2,6)}`, label: "Reasoning", variant: "think", at: Date.now() }]);
      }));
      unlistens.current.push(await onChatCitations((ev: ChatCitationsEvent) => {
        if (dead) return;
        // Map the citations onto the *assistant* message that is going to be
        // produced; we don't know its id yet, so we just store the list under
        // a synthetic key. When the assistant message arrives, we re-attach.
        setCitations((c) => ({ ...c, [`pending:${ev.message_id_user}`]: ev.citations }));
        if (ev.citations.length > 0) {
          setActivity((a) => [...a, { id: `cv-${Date.now()}`, label: `Retrieved ${ev.citations.length} citation${ev.citations.length === 1 ? "" : "s"}${ev.used_vector ? " (vector + FTS)" : " (FTS only)"}`, variant: "info", at: Date.now() }]);
        }
      }));
      // Listen for the server-side auto-title. The chat
      // pipeline emits this on a background task after a
      // successful `done` event; the frontend refreshes
      // the conversation list so the sidebar updates
      // without a full reload.
      unlistens.current.push(await onChatTitleUpdated((ev) => {
        if (dead) return;
        setConversations((cs) => cs.map((c) => c.id === ev.conversation_id ? { ...c, title: ev.title } : c));
        // Also patch the active conversation so the
        // header pill shows the new title immediately.
        setActive((cur) => cur && cur.id === ev.conversation_id ? { ...cur, title: ev.title } : cur);
        setActivity((a) => [...a, {
          id: `tt-${Date.now()}`,
          label: `Title auto-set to “${ev.title}”${ev.from_model ? " (model)" : " (heuristic)"}`,
          variant: "info",
          at: Date.now(),
        }]);
      }));
      // Phase P4 — Bug F: when an explicit memory save
      // completes the backend emits `nexus:memory.saved`.
      // We surface the outcome in the activity timeline
      // and clear the running state because the chat
      // pipeline returns synchronously from
      // `handle_explicit_save` (the model is not
      // called). Without this, the `done` event the
      // explicit-save path emits would still drive the
      // `running` flag to false, but the `setRunning`
      // setter is inside `handleDelta` and runs only
      // when the listener fires — being explicit here
      // makes the lifecycle clear.
      unlistens.current.push(await onMemorySaved((ev) => {
        if (dead) return;
        if (!activeRef.current || ev.conversation_id !== activeRef.current.id) return;
        const variant: ActivityEvent["variant"] = ev.outcome === "saved" || ev.outcome === "updated" ? "ok" : ev.outcome === "rejected" ? "warn" : "error";
        const label = ev.outcome === "saved"
          ? `Memory saved: “${ev.content ?? ""}” (${ev.category ?? "project_fact"})`
          : ev.outcome === "updated"
          ? `Memory already remembered: “${ev.content ?? ""}”`
          : ev.outcome === "rejected"
          ? `Memory rejected: ${ev.reason ?? ""}`
          : `Memory save failed: ${ev.reason ?? ""}`;
        setActivity((a) => [...a, { id: `ms-${Date.now()}-${Math.random().toString(16).slice(2,6)}`, label, variant, at: Date.now() }]);
        // Re-fetch messages so the in-chat bubble
        // (synthesised by `handle_explicit_save`) shows
        // up immediately.
        if (activeRef.current) {
          listMessages(activeRef.current.id).then(setMessages).catch(() => {});
        }
      }));
    })();
    return () => {
      dead = true;
      unlistens.current.forEach((f) => f());
      unlistens.current = [];
      clearWatchdog();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (active) {
      listMessages(active.id).then(setMessages).catch((e) => setError(String(e)));
    } else {
      setMessages([]);
    }
  }, [active?.id]);

  // Phase P4 — Bug E + L: rehydrate the in-flight state
  // from the backend request registry when the active
  // conversation changes (or the component remounts
  // after the user navigated to another page and back).
  // The registry is the source of truth for "is
  // anything in flight?" so the UI can safely rehydrate
  // even if the user has been away for several minutes.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const activeList: ActiveRequestSnapshot[] = await chatRequestListActive();
        if (cancelled) return;
        if (!active) { return; }
        // Find the request for the *current* active
        // conversation (if any). Requests belonging to
        // other conversations are ignored — they are
        // not the chat the user is currently looking at.
        const mine = activeList.find((r) => r.conversation_id === active.id);
        if (mine) {
          // There is an in-flight request for this
          // conversation. Rehydrate the running state.
          activeRequestId.current = mine.request_id;
          startedAt.current = Date.now();
          setRunning(true);
          setLastPhase("waiting");
          setError(null);
        } else {
          // No in-flight request for this conversation;
          // make sure the local state is consistent.
          activeRequestId.current = null;
          startedAt.current = null;
          setRunning(false);
          setLastPhase(null);
        }
      } catch (e) {
        // The list endpoint is best-effort. If it fails
        // (e.g. registry not yet wired) the local state
        // is unchanged, so a user mid-stream still sees
        // their progress.
        if (!cancelled) {
          // No-op: leave the previous state.
          void e;
        }
      }
    })();
    return () => { cancelled = true; };
  }, [active?.id]);

  const handleDelta = (ev: ChatDeltaEvent) => {
    // Phase P4 — Bug L: filter out late events from a
    // stale request. Every `nexus:chat.delta` carries a
    // `request_id` (ULID). If we have a known
    // `activeRequestId` and the event's id does not
    // match, the event belongs to an old request and
    // must not be applied. This is the only line of
    // defence against late events from a previous
    // request overwriting a newer request's state
    // (the user sent a new message while the previous
    // one was still streaming).
    if (ev.request_id && activeRequestId.current && ev.request_id !== activeRequestId.current) {
      return;
    }
    // Note: the listener already filters by the *current* active
    // conversation via `activeRef`. We do not filter again here.
    // Capture latency telemetry on the first token so the chat
    // composer can show time-to-first-token alongside the
    // elapsed-time clock. Backend emits `dispatched_at_ms` and
    // `first_token_at_ms`; we use the wall-clock here so the
    // values are independent of clock skew between host and IPC.
    if (ev.kind === "token" && ev.first_token_at_ms && ev.dispatched_at_ms && ttftMs === null) {
      setTtftMs(Math.max(0, ev.first_token_at_ms - ev.dispatched_at_ms));
    }
    // Phase P4 — Bug L: capture the request id on the
    // first delta so subsequent deltas can be matched
    // to the in-flight request. The backend's `chat_send`
    // promise resolves before the first token arrives,
    // so the request id is only known via the events.
    if (ev.request_id) {
      activeRequestId.current = ev.request_id;
    }
    // Update the message text in place. If no local message has
    // the backend id (e.g. a race where the user navigated away
    // and came back) the message is silently skipped; the
    // optimistic refresh below will pick up the final state from
    // the DB.
    setMessages((prev) => prev.map((m) => m.id === ev.message_id ? { ...m, content: ev.text, status: ev.kind === "done" ? "complete" : ev.kind === "error" ? "error" : ev.kind === "cancelled" ? "cancelled" : "streaming" } : m));
    if (ev.kind === "done") {
      // Terminal event: always clear the running state, even if
      // the local message list does not contain the id (so the
      // waiting indicator cannot get stuck).
      // Phase P4 — Bug L: clear the active request id so the
      // next `nexus:chat.delta` cannot match a stale
      // request id.
      activeRequestId.current = null;
      clearWatchdog();
      setRunning(false);
      setLastPhase(null);
      // Auto-title: if the user has not yet renamed this
      // conversation, derive a short title from the first user
      // message. Avoids a costly model call and gives the sidebar
      // a meaningful name immediately.
      void maybeAutoTitle(ev.conversation_id);
      // Transition the latest "Sending" activity to a terminal "Done" state
      // so the activity timeline does not show stale "Sending" entries.
      setActivity((a) => {
        const idx = findLastSending(a);
        if (idx >= 0) {
          const updated = [...a];
          updated[idx] = {
            ...updated[idx],
            label: `Done${ev.finish_reason ? ` · ${ev.finish_reason}` : ""}${ev.usage?.total_tokens ? ` · ${ev.usage.total_tokens} tok` : ""}`,
            variant: "ok",
            at: Date.now(),
          };
          return updated;
        }
        return [...a, { id: `d-${Date.now()}`, label: `Done${ev.finish_reason ? ` · ${ev.finish_reason}` : ""}${ev.usage?.total_tokens ? ` · ${ev.usage.total_tokens} tok` : ""}`, variant: "ok", at: Date.now() }];
      });
    } else if (ev.kind === "error") {
      clearWatchdog();
      setError(ev.text);
      setRunning(false);
      setLastPhase(null);
      // Phase P4 — Bug L: clear the active request id so
      // the next `nexus:chat.delta` cannot match a stale
      // request id.
      activeRequestId.current = null;
      // The backend now preserves any partial content the
      // model already produced and tags the message
      // `interrupted` rather than clobbering the bubble. We
      // surface the failure reason in the activity timeline
      // (where it is most visible) and in the inline status
      // banner above the composer; we do NOT replace
      // `m.content` because the streamed text is the user's
      // real partial answer.
      setMessages((prev) => prev.map((m) => m.id === ev.message_id && m.content ? m : m.id === ev.message_id ? { ...m, status: "interrupted" } : m));
      setActivity((a) => {
        const idx = findLastSending(a);
        if (idx >= 0) {
          const updated = [...a];
          updated[idx] = { ...updated[idx], label: `Error: ${ev.text}`, variant: "error", at: Date.now() };
          return updated;
        }
        return [...a, { id: `e-${Date.now()}`, label: `Error: ${ev.text}`, variant: "error", at: Date.now() }];
      });
    } else if (ev.kind === "cancelled") {
      clearWatchdog();
      setRunning(false);
      // Phase P4 — Bug L: clear the active request id.
      activeRequestId.current = null;
      setLastPhase(null);
      setActivity((a) => {
        const idx = findLastSending(a);
        if (idx >= 0) {
          const updated = [...a];
          updated[idx] = { ...updated[idx], label: "Cancelled", variant: "warn", at: Date.now() };
          return updated;
        }
        return [...a, { id: `c-${Date.now()}`, label: "Cancelled", variant: "warn", at: Date.now() }];
      });
    } else if (ev.kind === "token") {
      // The token event already contains the cumulative text.
      // Transition the "Waiting for model" activity entry to
      // "Generating" so the user sees the model is producing output.
      setLastPhase("generating");
      setActivity((a) => {
        const idx = findLastSending(a);
        if (idx < 0) return a;
        if (a[idx].label !== "Waiting for model") return a;
        const updated = [...a];
        updated[idx] = { ...updated[idx], label: "Generating", variant: "info", at: Date.now() };
        return updated;
      });
    }
  };

  const onNewConversation = async () => {
    const c = await createConversation("New conversation", undefined, activeProfileId ?? undefined, activeProject?.id ?? null);
    await refresh();
    setActive(c);
  };

  const onSend = async () => {
    if (!draft.trim() && attachments.length === 0) return;
    if (!active) {
      const c = await createConversation(draft.slice(0, 60), undefined, activeProfileId ?? undefined, activeProject?.id ?? null);
      setActive(c);
      await refresh();
    }
    const conv = active ?? (await listConversations()).find((c) => c.title);
    if (!conv) return;
    setError(null);
    setReasoning("");
    setLastPhase("waiting");
    setTtftMs(null);
    // Phase P4 — Bug L: cancel any in-flight request for
    // this conversation before sending. The backend
    // allows only one active request per conversation,
    // so a new send must deterministically cancel the
    // previous one. Without this, a user sending "what's
    // my age?" while the "Save to your memory" request
    // is still going would orphan the in-flight token.
    if (activeRequestId.current) {
      try {
        await chatRequestCancel(activeRequestId.current);
      } catch (e) {
        // Best-effort: the request may have completed
        // between the registry check and the cancel
        // call. The new request will register its own
        // id and overwrite the ref.
        void e;
      }
      activeRequestId.current = null;
    }
    setActivity((a) => [...a, { id: `s-${Date.now()}`, label: "Waiting for model", variant: "info", at: Date.now() }]);
    setRunning(true);
    // Watchdog: the Rust side has a 180s timeout. We give the
    // JS side a slightly longer bound so a lost `done` event does
    // not leave the UI stuck. The watchdog is cleared on any
    // terminal `done`/`error`/`cancelled` event and on
    // `onCancel`.
    clearWatchdog();
    watchdogRef.current = window.setTimeout(() => {
      watchdogRef.current = null;
      setRunning(false);
      setLastPhase(null);
      setError("The model did not respond in time. The request was abandoned after 240 s of inactivity. Check the LM Studio server and try again.");
      setActivity((a) => {
        const idx = findLastSending(a);
        if (idx >= 0) {
          const updated = [...a];
          updated[idx] = { ...updated[idx], label: "Timed out", variant: "error", at: Date.now() };
          return updated;
        }
        return [...a, { id: `t-${Date.now()}`, label: "Timed out", variant: "error", at: Date.now() }];
      });
    }, 240_000);
    const text = draft;
    const draftAttachments = attachments;
    setDraft("");
    setAttachments([]);
    // Optimistically append the user message. We show the folded
    // text (with attachment blocks) so the user can see exactly
    // what was sent.
    const folded = draftAttachments.length === 0
      ? text
      : `${text}\n\n---\nAttached files (read-only context, not persisted as documents):\n` +
        draftAttachments.map((a) => {
          // Image attachments are sent via the vision path —
          // show a short marker rather than the (very long)
          // base64 data URI.
          if (a.kind === "image") {
            return `\n### ${a.filename} (image; sent via vision path; mime: ${a.mime_type})\n`;
          }
          return `\n### ${a.filename}${a.truncated ? " (truncated)" : ""}\n\`\`\`\n${a.text.length > 64 * 1024 ? a.text.slice(0, 64 * 1024) : a.text}\n\`\`\`\n`;
        }).join("");
    const userMsg: Message = {
      id: `tmp-${Date.now()}`,
      conversation_id: conv.id,
      role: "user",
      content: folded,
      status: "complete",
      token_count: null, latency_ms: null, model_profile_id: null,
      created_at: Date.now(),
    };
    setMessages((prev) => [...prev, userMsg]);
    try {
      await chatSend(conv.id, {
        text,
        modelProfileId: activeProfileId ?? undefined,
        options: { useVault, vaultLimit: 5 },
        attachments: draftAttachments.map((a) => ({
          filename: a.filename,
          kind: a.kind,
          mime_type: a.mime_type,
          text: a.text,
          truncated: a.truncated,
        })),
      });
      const fresh = await listMessages(conv.id);
      setMessages(fresh);
      // For each newly-arrived assistant message, fetch its persisted citations
      // and merge them into our local map. The pending citations from the
      // `nexus:chat.citations` event were stored under `pending:<user_id>`; we
      // copy them onto the assistant message that immediately follows.
      const updatedCitations: Record<string, Citation[]> = { ...citations };
      for (let i = 0; i < fresh.length; i++) {
        const m = fresh[i];
        if (m.role !== "user") continue;
        const pending = updatedCitations[`pending:${m.id}`];
        if (!pending) continue;
        // The assistant message that follows this user message is the next
        // message in the list.
        const assistant = fresh[i + 1];
        if (assistant && assistant.role === "assistant") {
          updatedCitations[assistant.id] = pending;
        }
        // Pull the persisted citations too, so a reload still shows them.
        try {
          const persisted = await vaultListCitations(assistant?.id ?? "");
          if (persisted.length > 0 && assistant) {
            updatedCitations[assistant.id] = persisted;
          }
        } catch { /* ignore */ }
        delete updatedCitations[`pending:${m.id}`];
      }
      setCitations(updatedCitations);
      await refresh();
    } catch (e) {
      clearWatchdog();
      setError(String(e));
      setRunning(false);
      setLastPhase(null);
      // Transition the "Sending" activity entry to an error state so
      // the activity panel does not show a stale "Sending" record.
      setActivity((a) => {
        const idx = findLastSending(a);
        if (idx >= 0) {
          const updated = [...a];
          updated[idx] = { ...updated[idx], label: `Failed: ${String(e)}`, variant: "error", at: Date.now() };
          return updated;
        }
        return [...a, { id: `e-${Date.now()}`, label: `Failed: ${String(e)}`, variant: "error", at: Date.now() }];
      });
    }
  };

  const onCancel = async () => {
    clearWatchdog();
    await cancelInflight();
    setRunning(false);
    setLastPhase(null);
  };

  // Attachment picker. Opens a native file dialog, then asks the
  // Rust core to read each selected file. We cap at 8 attachments
  // per send to keep prompt size predictable; the Rust side
  // already caps per-file size at 256 KB.
  const onPickAttachments = async () => {
    if (running) return;
    try {
      const picked = await openDialog({
        multiple: true,
        directory: false,
        title: "Attach files (text, source, PDF, DOCX, or images)",
        filters: [
          { name: "Images (vision-capable models)", extensions: ["png", "jpg", "jpeg", "gif", "webp"] },
          { name: "Text & source", extensions: ["txt", "md", "markdown", "rst", "log", "py", "js", "jsx", "ts", "tsx", "rs", "go", "c", "cpp", "h", "hpp", "java", "kt", "swift", "rb", "sh", "ps1", "toml", "yaml", "yml", "json", "xml", "ini", "cfg", "conf", "html", "css", "csv", "tsv"] },
          { name: "Documents", extensions: ["pdf", "docx", "xlsx", "xls", "ods", "ipynb"] },
          { name: "All files", extensions: ["*"] },
        ],
      });
      if (!picked) return;
      const paths: string[] = Array.isArray(picked) ? picked : [picked];
      const room = Math.max(0, 8 - attachments.length);
      const limit = paths.slice(0, room);
      const added: AttachmentPayload[] = [];
      for (const p of limit) {
        try {
          const payload = await chatReadAttachment(p);
          added.push(payload);
        } catch (e) {
          // Skip files the Rust core refused (too large / not a
          // regular file / unreadable) and continue with the
          // remaining ones rather than aborting the whole batch.
          // The error is logged for diagnostics.
          // eslint-disable-next-line no-console
          console.warn("attachment failed:", p, e);
        }
      }
      if (paths.length > room) {
        // eslint-disable-next-line no-console
        console.warn(`attachment cap reached: dropped ${paths.length - room} file(s)`);
      }
      setAttachments((prev) => [...prev, ...added]);
    } catch (e) {
      // eslint-disable-next-line no-console
      console.warn("attachment picker failed:", e);
    }
  };

  const onRemoveAttachment = (path: string) => {
    setAttachments((prev) => prev.filter((a) => a.path !== path));
  };

  // Auto-title fallback. The primary auto-title is now
  // server-side: the chat pipeline emits a
  // `nexus:chat.title_updated` event after a successful
  // `done`. This function is the **fallback** for cases
  // where the server-side flow could not run (no model
  // profile available, the call timed out, etc.). It
  // produces a deterministic title from the first user
  // message so the user never sees "New conversation"
  // for an active chat for long.
  const maybeAutoTitle = async (conversationId: string) => {
    try {
      const msgs = await listMessages(conversationId);
      const firstUser = msgs.find((m) => m.role === "user");
      if (!firstUser || !firstUser.content.trim()) return;
      const current = await listConversations(false).then((list) => list.find((c) => c.id === conversationId));
      if (!current) return;
      const isUntitled = !current.title || current.title === "New conversation" || current.title.trim() === "";
      if (!isUntitled) return;
      // Wait briefly for the server-side title to
      // arrive; if it does, the sidebar will be
      // updated by the listener above and we should
      // not stomp on it.
      const next = deriveTitle(firstUser.content);
      if (!next || next === current.title) return;
      // Re-check the title after a short wait so we
      // do not clobber a server-side title that is
      // still in flight.
      await new Promise((r) => setTimeout(r, 1500));
      const recheck = await listConversations(false).then((list) => list.find((c) => c.id === conversationId));
      if (recheck && !isUntitledRefFor(recheck.title)) return;
      await renameConversation(conversationId, next);
      await refresh();
    } catch { /* ignore — best-effort */ }
  };

  function isUntitledRefFor(title: string | undefined | null): boolean {
    return !title || title === "New conversation" || title.trim() === "";
  }

  // Live elapsed-time tick: while a request is in flight, update
  // `elapsedMs` every 200ms so the chat composer can show how long
  // the current phase has been running. Cancels cleanly on unmount
  // or when `running` flips back to false.
  useEffect(() => {
    if (!running) {
      startedAt.current = null;
      setElapsedMs(0);
      return;
    }
    if (startedAt.current === null) startedAt.current = Date.now();
    const id = setInterval(() => {
      if (startedAt.current !== null) setElapsedMs(Date.now() - startedAt.current);
    }, 200);
    return () => clearInterval(id);
  }, [running]);

  // Refresh the context inspector when the draft or active project
  // changes. We debounce to avoid hammering the backend.
  useEffect(() => {
    const id = setTimeout(() => {
      if (!active || !draft.trim()) { setContextBundle(null); return; }
      setContextLoading(true);
      contextPreview({
        project_id: activeProject?.id ?? null,
        conversation_id: active.id,
        user_message: draft,
        max_tokens: 1500,
      }).then(setContextBundle).catch(() => setContextBundle(null)).finally(() => setContextLoading(false));
    }, 350);
    return () => clearTimeout(id);
  }, [draft, active?.id, activeProject?.id]);

  const onRename = async (c: Conversation) => {
    const title = await promptModal({
      title: "Rename conversation",
      label: "Title",
      defaultValue: c.title,
      submitLabel: "Rename",
    });
    if (title && title !== c.title) {
      await renameConversation(c.id, title);
      await refresh();
    }
  };

  const onArchive = async (c: Conversation) => {
    await archiveConversation(c.id, !c.archived);
    await refresh();
    if (active?.id === c.id) setActive(null);
  };

  const onDelete = async (c: Conversation) => {
    const ok = await confirmModal({
      title: "Delete conversation",
      description: `Delete "${c.title}"? This cannot be undone.`,
      submitLabel: "Delete conversation",
      destructive: true,
    });
    if (!ok) return;
    await deleteConversation(c.id);
    if (active?.id === c.id) setActive(null);
    await refresh();
  };

  const onSystemPromptEdit = async () => {
    if (!active) return;
    const next = await promptModal({
      title: "Edit system prompt",
      description: "Leave blank to clear the system prompt for this conversation.",
      label: "System prompt",
      defaultValue: active.system_prompt ?? "",
      submitLabel: "Save",
    });
    if (next === null) return;
    await setSystemPrompt(active.id, next.length > 0 ? next : null);
    await refresh();
  };

  const onSelectProfile = (id: string) => {
    setActiveProfileId(id || null);
  };

  const activityForDisplay = useMemo(() => activity.slice(-12), [activity]);

  return (
    <>
    <div className="grid h-full grid-cols-12 gap-4 p-4 min-h-0">
      <aside className="col-span-3 panel p-3 flex flex-col min-h-0">
        <div className="flex items-center justify-between">
          <div className="h-title">Conversations</div>
          <button className="btn btn-primary" onClick={onNewConversation}>New</button>
        </div>
        <div className="mt-3 space-y-1 overflow-y-auto flex-1 min-h-0">
          {conversations.length === 0 && <div className="text-xs text-slate-500 px-2">No conversations yet.</div>}
          {conversations.map((c) => (
            <button
              key={c.id}
              className={`w-full text-left rounded-lg p-2 transition ${active?.id === c.id ? "bg-cyan2-500/10 ring-1 ring-cyan2-500/30" : "hover:bg-white/5"}`}
              onClick={() => setActive(c)}
            >
              <div className="text-sm text-slate-100 truncate">{c.title || "Untitled"}</div>
              <div className="text-[11px] text-slate-500 font-mono">{new Date(c.updated_at).toLocaleString()}</div>
              <div className="mt-1 flex gap-1 text-[10px]">
                <span className="kbd" onClick={(e) => { e.stopPropagation(); onRename(c); }}>rename</span>
                <span className="kbd" onClick={(e) => { e.stopPropagation(); onArchive(c); }}>{c.archived ? "unarchive" : "archive"}</span>
                <span className="kbd" onClick={(e) => { e.stopPropagation(); onDelete(c); }}>delete</span>
              </div>
            </button>
          ))}
        </div>
        <div className="mt-3 border-t border-white/5 pt-3">
          <div className="h-sub">Active model</div>
          <select className="input mt-2" value={activeProfileId ?? ""} onChange={(e) => onSelectProfile(e.target.value)}>
            <option value="">(use conversation default)</option>
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>{p.display_name} · {p.model_id}</option>
            ))}
          </select>
        </div>
      </aside>

      <section className="col-span-6 panel p-4 flex flex-col min-h-0">
        <div className="flex items-center gap-3">
          <NexusGlyph activity={running ? "generating" : "idle"} size={56} />
          <div className="flex-1 min-w-0">
            <div className="h-title truncate">{active?.title ?? "Select or start a conversation"}</div>
            <div className="text-[11px] text-slate-500 font-mono truncate">
              {active?.system_prompt ? "system prompt set" : "default system prompt"}
              {active && ` · ${messages.length} message${messages.length === 1 ? "" : "s"}`}
            </div>
          </div>
          {active && <button className="btn" onClick={onSystemPromptEdit}>System prompt</button>}
        </div>

        <div className="mt-3 flex-1 min-h-0 overflow-y-auto pr-1 space-y-3">
          {messages.length === 0 && (
            <div className="text-sm text-slate-500 text-center py-12">
              Ask the assistant something. Use the sidebar to start a new conversation.
            </div>
          )}
          {messages.map((m) => <Bubble key={m.id} m={m} citations={citations[m.id] ?? []} />)}
        </div>

        {reasoning && (
          <details className="mt-2 text-[11px] text-slate-500">
            <summary className="cursor-pointer">Reasoning ({reasoning.length} chars)</summary>
            <pre className="mt-1 whitespace-pre-wrap font-mono text-slate-400 max-h-32 overflow-y-auto">{reasoning}</pre>
          </details>
        )}

        {error && <div className="mt-2 text-[11px] text-rose2-400 font-mono">{error}</div>}

        <div className="mt-3 flex items-center gap-3 flex-wrap">
          <label className="flex items-center gap-2 text-[11px] text-slate-400 cursor-pointer select-none" title="Include the Knowledge Vault context and cite the chunks that informed the answer.">
            <input
              type="checkbox"
              className="accent-cyan2-500"
              checked={useVault}
              onChange={(e) => setUseVault(e.target.checked)}
            />
            use vault
          </label>
          {activeProject && (
            <span className="chip text-cyan2-400 border-cyan2-500/40 bg-cyan2-500/10" title="Project context is prepended to every chat turn.">
              project: {activeProject.name}
              <button
                className="ml-1 text-slate-400 hover:text-rose2-400"
                title="Clear active project"
                onClick={async () => { await setActiveProject(null); setActiveProjectState(null); }}
              >×</button>
            </span>
          )}
        </div>
        {running && (
          <div
            className="mt-2 panel-soft p-2 px-3 text-[12px] text-slate-300 flex items-center gap-2"
            role="status"
            aria-live="polite"
            data-testid="chat-status"
          >
            <span className="inline-block w-2 h-2 rounded-full bg-cyan2-400 animate-pulse" aria-hidden />
            <span className="font-mono">
              {lastPhase === "generating" ? "Generating" : lastPhase === "finalizing" ? "Finalizing" : "Waiting for model"}
              {" · "}
              {(elapsedMs / 1000).toFixed(1)}s
            </span>
            {ttftMs != null && (
              <span className="font-mono text-slate-500" title="Time to first token reported by the backend">
                {" · TTFT "}
                {(ttftMs / 1000).toFixed(2)}s
              </span>
            )}
            <span className="ml-auto text-[10px] text-slate-500">
              Stop with Esc or the button →
            </span>
          </div>
        )}
        {attachments.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-2" role="list" aria-label="Attached files">
            {attachments.map((a) => (
              <span
                key={a.path}
                role="listitem"
                className="inline-flex items-center gap-2 rounded-lg border border-white/10 bg-ink-800/70 px-2 py-1 text-[11px] text-slate-300"
                title={`${a.path} · ${a.size_bytes} bytes${a.non_utf8_replaced ? " · non-UTF-8 bytes replaced" : ""}`}
              >
                <span aria-hidden className="text-cyan2-400">📎</span>
                <span className="font-mono truncate max-w-[16rem]">{a.filename}</span>
                <span className="text-slate-500">
                  {humanSize(a.size_bytes)}
                  {a.truncated ? " · truncated" : ""}
                </span>
                <button
                  type="button"
                  className="ml-1 text-slate-400 hover:text-rose2-400"
                  onClick={() => onRemoveAttachment(a.path)}
                  aria-label={`Remove ${a.filename}`}
                >
                  ×
                </button>
              </span>
            ))}
          </div>
        )}
        <div className="mt-2 flex items-end gap-2">
          <button
            type="button"
            className="btn"
            onClick={onPickAttachments}
            disabled={running || !active || attachments.length >= 8}
            title="Attach text or source files (text, markdown, code; ≤256 KB each, ≤8 per send)"
            aria-label="Attach files"
          >
            📎
          </button>
          <textarea
            className="input min-h-[3rem] max-h-40 resize-y"
            placeholder={active ? "Message NEXUS…  (Enter to send · Shift+Enter for newline)" : "Start a conversation first"}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                if (!running) onSend();
              }
              if (e.key === "Escape" && running) {
                e.preventDefault();
                onCancel();
              }
            }}
            disabled={!active}
          />
          {running ? (
            <button className="btn" onClick={onCancel}>Stop</button>
          ) : (
            <button className="btn btn-primary" onClick={onSend} disabled={(!draft.trim() && attachments.length === 0) || !active}>
              Send
            </button>
          )}
        </div>
        {(contextBundle || contextLoading) && (
          <div className="mt-2 max-h-64 overflow-y-auto">
            <ContextInspector
              bundle={contextBundle}
              loading={contextLoading}
              onJumpToProject={async (id) => { await setActiveProject(id); setActiveProjectState(await getActiveProject()); }}
              onJumpToMemory={(id) => { window.location.hash = `#memory-${id}`; }}
            />
          </div>
        )}
      </section>

      <aside className="col-span-3 flex flex-col gap-3 min-h-0">
        <ActivityTimeline events={activityForDisplay} running={running} />
        <div className="panel p-3 space-y-2 text-xs text-slate-400">
          <div className="h-title">Tips</div>
          <ul className="list-disc pl-4 space-y-1">
            <li>Reasoning appears above and is hidden by default.</li>
            <li>Markdown is rendered: <code>**bold**</code>, <code>`code`</code>, and fenced code blocks.</li>
            <li>Phase 4 adds grounded citations and the Knowledge Vault.</li>
          </ul>
        </div>
      </aside>
    </div>
    </>
  );
}

function Bubble({ m, citations }: { m: Message; citations?: Citation[] }) {
  const isUser = m.role === "user";
  const cits = citations ?? [];
  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
      <div className={`max-w-[85%] rounded-2xl px-4 py-3 ${isUser ? "bg-cyan2-500/10 ring-1 ring-cyan2-500/30 text-slate-100" : "bg-ink-800/70 ring-1 ring-white/5 text-slate-100"}`}>
        <div className="flex items-center gap-2 mb-1.5">
          <span className="text-[10px] uppercase tracking-wide text-slate-500 font-semibold">
            {m.role}
          </span>
          {m.status === "streaming" && <span className="text-[10px] text-cyan2-400 animate-breathe">streaming</span>}
          {m.status === "cancelled" && <span className="text-[10px] text-amber2-400">cancelled</span>}
          {m.status === "error" && <span className="text-[10px] text-rose2-400">error</span>}
          {m.latency_ms != null && <span className="ml-auto text-[10px] text-slate-500 font-mono">{m.latency_ms} ms</span>}
        </div>
        {isUser ? (
          <div className="whitespace-pre-wrap text-sm leading-relaxed">{m.content}</div>
        ) : (
          <>
            <MarkdownView text={m.content || (m.status === "streaming" ? "…" : "")} />
            {cits.length > 0 && <CitationsPanel citations={cits} />}
          </>
        )}
      </div>
    </div>
  );
}

function CitationsPanel({ citations }: { citations: Citation[] }) {
  return (
    <details className="mt-3 group">
      <summary className="cursor-pointer text-[11px] text-cyan2-400 hover:text-cyan2-300 flex items-center gap-2">
        <span className="kbd">{citations.length}</span>
        <span>Sources</span>
      </summary>
      <div className="mt-2 space-y-2">
        {citations.map((c) => (
          <div key={c.chunk_id} className="rounded-md border border-white/5 bg-ink-900/40 p-2">
            <div className="flex items-center gap-2 text-[11px]">
              <span className="kbd">[{c.index}]</span>
              <span className="font-medium text-slate-200 truncate flex-1" title={c.document_path}>{c.document_title}</span>
              <span className="text-slate-500 font-mono">{c.source}</span>
            </div>
            <div className="mt-1 text-[10px] text-slate-500 font-mono truncate" title={c.document_path}>{c.document_path}</div>
            <pre className="mt-1 text-[11px] text-slate-400 whitespace-pre-wrap font-mono leading-relaxed max-h-24 overflow-y-auto">{c.text}</pre>
          </div>
        ))}
      </div>
    </details>
  );
}