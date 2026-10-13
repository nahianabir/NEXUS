// Reusable React error boundary.
//
// Wraps a route or any subtree. On a thrown render it
// shows an actionable error card with a back-to-home
// button and a copyable diagnostic reference, instead of
// the historical "black screen" failure.
//
// The component intentionally catches the error locally
// instead of forwarding it. The parent (App-level)
// boundary is the last line of defence.
//
// Props:
//   - `title`: optional header text.
//   - `fallbackRoute`: where the back-to-home button goes.
//   - `onError`: optional side-effect (logging, telemetry
//     without secrets).
import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";
import { Link } from "react-router-dom";

type Props = {
  children: ReactNode;
  title?: string;
  fallbackRoute?: string;
  onError?: (error: Error, info: ErrorInfo) => void;
};

type State = {
  hasError: boolean;
  message: string;
  ref: string;
  stack?: string;
};

function shortRef(): string {
  // A short, deterministic per-incident reference. The
  // user can paste this into a bug report.
  const t = Date.now().toString(36);
  const r = Math.floor(Math.random() * 0xffff).toString(36);
  return `nx-${t}-${r}`;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, message: "", ref: "" };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return {
      hasError: true,
      message: error?.message || "Unknown error",
      stack: error?.stack,
      ref: shortRef(),
    };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    // The full error is logged to the console so that the
    // user can copy it via DevTools without us displaying
    // a raw stack trace in the UI.
    // eslint-disable-next-line no-console
    console.error("[nexus] route error", this.state.ref, error, info);
    this.props.onError?.(error, info);
  }

  private handleRetry = (): void => {
    this.setState({ hasError: false, message: "", ref: "", stack: undefined });
  };

  private handleCopy = (): void => {
    const text = [
      `NEXUS error ref: ${this.state.ref}`,
      `Route: ${this.props.fallbackRoute ?? "(root)"}`,
      `Title: ${this.props.title ?? "(untitled)"}`,
      `Message: ${this.state.message}`,
      this.state.stack ?? "(no stack)",
    ].join("\n");
    if (navigator.clipboard) {
      navigator.clipboard.writeText(text).catch(() => {});
    }
  };

  render() {
    if (!this.state.hasError) return this.props.children;
    const fallback = this.props.fallbackRoute ?? "/";
    return (
      <div className="p-6 max-w-3xl mx-auto" role="alert" aria-live="polite">
        <div className="panel p-5 space-y-3">
          <div className="flex items-center gap-2">
            <span className="chip bg-rose2-500/10 text-rose2-400 border-rose2-500/30">
              Error
            </span>
            <span className="text-sm text-slate-300">
              {this.props.title ?? "This screen could not be displayed."}
            </span>
          </div>
          <p className="text-sm text-slate-400">
            {this.state.message || "An unexpected error occurred."}
          </p>
          <p className="text-[11px] text-slate-500 font-mono">
            Reference: {this.state.ref}
          </p>
          <div className="flex gap-2 pt-1">
            <button className="btn-primary" onClick={this.handleRetry}>
              Retry
            </button>
            <Link to={fallback} className="btn">
              Back to Home
            </Link>
            <button className="btn" onClick={this.handleCopy}>
              Copy diagnostic
            </button>
          </div>
        </div>
      </div>
    );
  }
}
