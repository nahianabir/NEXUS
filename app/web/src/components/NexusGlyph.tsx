// A small, pure-SVG, reduced-motion-aware visualization that the Command Center
// and the Assistant header share. Renders an idle "breathing" state by default
// and reflects an `activity` prop when something is happening.

import type { JSX } from "react";

type Props = {
  activity?: "idle" | "thinking" | "retrieving" | "generating" | "executing" | "error";
  size?: number;
  reducedMotion?: boolean;
};

export function NexusGlyph({ activity = "idle", size = 220, reducedMotion = false }: Props): JSX.Element {
  const cx = 110;
  const cy = 110;
  const baseR = 86;

  const ringClasses: Record<NonNullable<Props["activity"]>, string> = {
    idle: "opacity-90",
    thinking: "opacity-90",
    retrieving: "opacity-90",
    generating: "opacity-100",
    executing: "opacity-100",
    error: "opacity-100",
  };

  return (
    <div className="relative inline-block" style={{ width: size, height: size }}>
      <svg viewBox="0 0 220 220" className="absolute inset-0">
        <defs>
          <radialGradient id="ng-core" cx="50%" cy="50%" r="50%">
            <stop offset="0%" stopColor="#67e0ff" stopOpacity="0.9" />
            <stop offset="60%" stopColor="#1fa5d3" stopOpacity="0.35" />
            <stop offset="100%" stopColor="#1fa5d3" stopOpacity="0" />
          </radialGradient>
          <linearGradient id="ng-stroke" x1="0" x2="1" y1="0" y2="1">
            <stop offset="0%" stopColor="#67e0ff" />
            <stop offset="100%" stopColor="#9a78f0" />
          </linearGradient>
        </defs>

        {/* Fine grid backdrop */}
        <g opacity={0.18}>
          {Array.from({ length: 11 }).map((_, i) => (
            <line key={`v${i}`} x1={20 * i} y1="0" x2={20 * i} y2="220" stroke="rgba(255,255,255,0.12)" />
          ))}
          {Array.from({ length: 11 }).map((_, i) => (
            <line key={`h${i}`} x1="0" y1={20 * i} x2="220" y2={20 * i} stroke="rgba(255,255,255,0.12)" />
          ))}
        </g>

        {/* Outermost slow ring */}
        <circle
          cx={cx}
          cy={cy}
          r={baseR + 22}
          fill="none"
          stroke="url(#ng-stroke)"
          strokeWidth="0.8"
          strokeDasharray="2 6"
          className={reducedMotion ? "" : "animate-spin24 origin-center"}
          style={{ transformOrigin: `${cx}px ${cy}px` }}
          opacity={0.7}
        />

        {/* Mid ring */}
        <circle
          cx={cx}
          cy={cy}
          r={baseR + 4}
          fill="none"
          stroke="url(#ng-stroke)"
          strokeWidth="1.4"
          className={`${ringClasses[activity]} ${reducedMotion ? "" : "animate-spin12"}`}
          style={{ transformOrigin: `${cx}px ${cy}px` }}
        />

        {/* Inner breathing ring */}
        <circle
          cx={cx}
          cy={cy}
          r={baseR - 18}
          fill="none"
          stroke="rgba(103,224,255,0.55)"
          strokeWidth="0.6"
          className={reducedMotion ? "" : "animate-breathe"}
        />

        {/* Core */}
        <circle cx={cx} cy={cy} r={baseR - 32} fill="url(#ng-core)" />
        <circle
          cx={cx}
          cy={cy}
          r={6}
          fill={activity === "error" ? "#ef5f7c" : "#ffc66e"}
          className={reducedMotion ? "" : "animate-breathe"}
        />

        {/* Activity indicator dots around the ring */}
        {activity !== "idle" && (
          <g>
            {Array.from({ length: 8 }).map((_, i) => {
              const a = (i / 8) * Math.PI * 2 + (reducedMotion ? 0 : Date.now() / 1200);
              const x = cx + Math.cos(a) * (baseR + 4);
              const y = cy + Math.sin(a) * (baseR + 4);
              const color =
                activity === "error" ? "#ef5f7c"
                : activity === "retrieving" ? "#9a78f0"
                : activity === "executing" ? "#ffc66e"
                : "#67e0ff";
              return <circle key={i} cx={x} cy={y} r={2.2} fill={color} opacity={0.85} />;
            })}
          </g>
        )}
      </svg>
    </div>
  );
}
