// Small SVG icon set used across the sidebar and the AI visualization.
// Each icon is a single path, currentColor, so it inherits the surrounding text color.

import type { JSX } from "react";

type Props = { name: string; className?: string };

export function Icon({ name, className }: Props): JSX.Element {
  const common = "h-4 w-4 shrink-0";
  const cls = className ? `${common} ${className}` : common;
  switch (name) {
    case "brain":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M9 4a3 3 0 0 0-3 3v.5A3 3 0 0 0 4 10.5v.5A3 3 0 0 0 5 14a3 3 0 0 0 1 3v.5A3 3 0 0 0 9 20a3 3 0 0 0 3-3V7a3 3 0 0 0-3-3z" />
          <path d="M15 4a3 3 0 0 1 3 3v.5a3 3 0 0 1 2 3v.5a3 3 0 0 1-1 3 3 3 0 0 1-1 3v.5A3 3 0 0 1 15 20a3 3 0 0 1-3-3V7a3 3 0 0 1 3-3z" />
        </svg>
      );
    case "graph":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="6" cy="6" r="2" /><circle cx="18" cy="6" r="2" /><circle cx="6" cy="18" r="2" /><circle cx="18" cy="18" r="2" /><circle cx="12" cy="12" r="2" />
          <path d="M8 6h8M6 8v8M18 8v8M8 18h8M10.5 10.5l3 3M10.5 13.5l3-3" />
        </svg>
      );
    case "router":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <rect x="3" y="13" width="18" height="7" rx="1.5" />
          <path d="M7 16h.01M11 16h.01" />
          <path d="M7 13V9a5 5 0 0 1 10 0v4" />
          <path d="M9 9V6M15 9V6" />
        </svg>
      );
    case "compass":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="9" />
          <path d="M15.5 8.5 13 13l-4.5 2.5L11 11z" />
        </svg>
      );
    case "spark":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 3v4M12 17v4M3 12h4M17 12h4M5.5 5.5l2.8 2.8M15.7 15.7l2.8 2.8M5.5 18.5l2.8-2.8M15.7 8.3l2.8-2.8" />
        </svg>
      );
    case "vault":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <circle cx="14" cy="12" r="3.5" />
          <path d="M14 12h3M3 9h4M3 15h4" />
        </svg>
      );
    case "flask":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M9 3h6M10 3v5l-5 9a2 2 0 0 0 1.7 3h10.6A2 2 0 0 0 19 17l-5-9V3" />
          <path d="M7.5 14h9" />
        </svg>
      );
    case "doc":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
          <path d="M14 3v5h5" />
        </svg>
      );
    case "grid":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <rect x="3" y="3" width="7" height="7" rx="1.4" />
          <rect x="14" y="3" width="7" height="7" rx="1.4" />
          <rect x="3" y="14" width="7" height="7" rx="1.4" />
          <rect x="14" y="14" width="7" height="7" rx="1.4" />
        </svg>
      );
    case "robot":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <rect x="5" y="7" width="14" height="11" rx="2.4" />
          <path d="M12 3v4M9 12h.01M15 12h.01M9 15h6" />
        </svg>
      );
    case "chart":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M4 19h16M6 16V9M11 16V5M16 16v-7M21 16v-4" />
        </svg>
      );
    case "calendar":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <rect x="3" y="5" width="18" height="16" rx="2" />
          <path d="M3 9h18M8 3v4M16 3v4" />
        </svg>
      );
    case "chip":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <rect x="6" y="6" width="12" height="12" rx="1.6" />
          <path d="M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3" />
        </svg>
      );
    case "pulse":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M3 12h4l2-5 4 10 2-5h6" />
        </svg>
      );
    case "gear":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1A1.7 1.7 0 0 0 4.6 9a1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
        </svg>
      );
    case "history":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M3 12a9 9 0 1 0 3-6.7" />
          <path d="M3 3v6h6" />
          <path d="M12 7v5l3.5 2" />
        </svg>
      );
    case "resume":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <path d="M5 4l14 8-14 8z" />
          <path d="M5 4v16" />
        </svg>
      );
    case "radar":
      return (
        <svg viewBox="0 0 24 24" className={cls} fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="9" />
          <circle cx="12" cy="12" r="5" />
          <path d="M12 12L4 6" />
          <path d="M12 3v3" />
        </svg>
      );
    default:
      return <span className={cls}>•</span>;
  }
}
