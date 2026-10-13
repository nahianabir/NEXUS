// In-app replacement for `window.prompt` / `window.confirm`.
//
// The previous code used the Tauri/Wry built-in
// `prompt()` which renders a browser-style "tauri.localhost
// says" dialog. This component provides a consistent
// in-app modal that matches the NEXUS design language.
//
// Three surface modes:
//   - `prompt`  — a single labelled text field, with
//                 submit/cancel.
//   - `confirm` — a yes/no confirmation with a clear
//                 description of the action.
//   - `alert`   — a single dismissable notification.
//
// The dialog uses a React portal so it overlays the
// rest of the application cleanly without forcing
// fragment wrappers in the calling code.
//
// Callers do NOT mount the dialog directly. Instead the
// `usePromptDialog` hook owns a shared, module-level
// request store and a single PromptDialog instance
// mounted at document.body by `<PromptDialogRoot />` in
// `main.tsx`. Pages call `prompt({...})`, `confirm({...})`
// or `alert(...)` from the hook and the global dialog
// takes care of rendering.
import { useEffect, useRef, useState, type FormEvent } from "react";
import { createPortal } from "react-dom";

export type PromptMode = "prompt" | "confirm" | "alert";

export type PromptRequest = {
  mode: PromptMode;
  title: string;
  description?: string;
  label?: string;
  placeholder?: string;
  defaultValue?: string;
  submitLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  validate?: (value: string) => string;
};

type Listener = (request: PromptRequest | null) => void;

let currentRequest: PromptRequest | null = null;
const listeners = new Set<Listener>();

export function getCurrentPromptRequest(): PromptRequest | null {
  return currentRequest;
}

export function setCurrentPromptRequest(req: PromptRequest | null): void {
  currentRequest = req;
  for (const l of listeners) l(req);
}

function subscribe(l: Listener): () => void {
  listeners.add(l);
  return () => { listeners.delete(l); };
}

type Resolver = (v: string | null) => void;
let pendingResolver: Resolver | null = null;

function resolve(value: string | null): void {
  const r = pendingResolver;
  pendingResolver = null;
  setCurrentPromptRequest(null);
  r?.(value);
}

export function openPrompt(
  mode: PromptMode,
  opts: Omit<PromptRequest, "mode">,
): Promise<string | null> {
  if (pendingResolver) {
    const prev = pendingResolver;
    pendingResolver = null;
    prev(null);
  }
  return new Promise((resolvePromise) => {
    pendingResolver = resolvePromise;
    setCurrentPromptRequest({ ...opts, mode });
  });
}

export function PromptDialogRoot() {
  const [request, setRequest] = useState<PromptRequest | null>(getCurrentPromptRequest());

  useEffect(() => {
    return subscribe(setRequest);
  }, []);

  if (!request) return null;

  return <PromptDialog request={request} onResolve={resolve} />;
}

type Props = {
  request: PromptRequest;
  onResolve: (value: string | null) => void;
};

function PromptDialog({ request, onResolve }: Props) {
  const [value, setValue] = useState(request.defaultValue ?? "");
  const [touched, setTouched] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // Reset state whenever a new request comes in. We use
  // the request itself as the dep so the modal
  // re-mounts in spirit when a new prompt opens.
  useEffect(() => {
    setValue(request.defaultValue ?? "");
    setTouched(false);
    requestAnimationFrame(() => inputRef.current?.focus());
  }, [request]);

  const isConfirm = request.mode === "confirm";
  const isAlert = request.mode === "alert";
  const error = request.validate ? request.validate(value) : "";
  const canSubmit = isAlert || isConfirm ? true : !error;

  const close = (result: string | null) => onResolve(result);

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    setTouched(true);
    if (isConfirm || isAlert) {
      close("ok");
      return;
    }
    if (error) return;
    close(value);
  };

  return createPortal(
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-xs"
      role="dialog"
      aria-modal="true"
      onClick={(e) => {
        if (e.target === e.currentTarget) close(null);
      }}
    >
      <form
        onSubmit={onSubmit}
        className="panel p-5 w-[min(420px,90vw)] space-y-3 shadow-2xl"
      >
        <div className="space-y-1">
          <h2 className="h-title">{request.title}</h2>
          {request.description && (
            <p className="text-[12px] text-slate-400 leading-relaxed">
              {request.description}
            </p>
          )}
        </div>

        {isConfirm ? (
          <p className="text-sm text-slate-300">
            This action cannot be undone from this dialog.
          </p>
        ) : isAlert ? (
          <p className="text-sm text-slate-300">
            {request.description ?? "Acknowledged."}
          </p>
        ) : (
          <div className="space-y-1.5">
            {request.label && (
              <label className="text-[11px] uppercase tracking-[0.18em] text-slate-500">
                {request.label}
              </label>
            )}
            <input
              ref={inputRef}
              type="text"
              className="input"
              value={value}
              placeholder={request.placeholder}
              onChange={(e) => setValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.preventDefault();
                  close(null);
                }
              }}
              aria-invalid={touched && !!error}
            />
            {touched && error && (
              <div className="text-[11px] text-rose2-400">{error}</div>
            )}
          </div>
        )}

        <div className="flex justify-end gap-2 pt-1">
          <button
            type="button"
            className="btn"
            onClick={() => close(null)}
          >
            {request.cancelLabel ?? "Cancel"}
          </button>
          <button
            type="submit"
            className={request.destructive ? "btn" : "btn-primary"}
            disabled={!canSubmit}
          >
            {request.submitLabel ??
              (isConfirm ? "Confirm" : "OK")}
          </button>
        </div>
      </form>
    </div>,
    document.body,
  );
}
