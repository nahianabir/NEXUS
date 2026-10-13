// VirtualList. Phase K.
//
// A tiny, dependency-free virtualised list. Items are rendered in
// a window of `overscan` above and below the visible viewport. The
// caller controls the row height; the list is not measured
// dynamically. Sufficient for the Audit page, the chat sidebar,
// and any other long list in the app.

import { useEffect, useRef, useState } from "react";

type Props<T> = {
  items: T[];
  rowHeight: number;
  height: number;                 // pixels of the visible scroll area
  overscan?: number;              // extra rows to render above/below
  render: (item: T, index: number) => React.ReactNode;
  getKey: (item: T, index: number) => string | number;
  className?: string;
};

export function VirtualList<T>({ items, rowHeight, height, overscan = 6, render, getKey, className }: Props<T>) {
  const ref = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  useEffect(() => {
    if (!ref.current) return;
    const onScroll = () => setScrollTop(ref.current?.scrollTop ?? 0);
    ref.current.addEventListener("scroll", onScroll);
    return () => ref.current?.removeEventListener("scroll", onScroll);
  }, []);
  const total = items.length;
  const visibleCount = Math.ceil(height / rowHeight) + overscan * 2;
  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const end = Math.min(total, start + visibleCount);
  const top = start * rowHeight;
  return (
    <div ref={ref} className={`overflow-y-auto ${className ?? ""}`} style={{ height }}>
      <div style={{ height: total * rowHeight, position: "relative" }}>
        <div style={{ position: "absolute", top, left: 0, right: 0 }}>
          {items.slice(start, end).map((it, i) => {
            const idx = start + i;
            return (
              <div key={getKey(it, idx)} style={{ height: rowHeight }}>
                {render(it, idx)}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
