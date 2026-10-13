// Top-level layout: sidebar + main + optional right inspector + status bar.
// Routes are registered for the 12 modules the spec calls out; Phase 1 ships
// real pages for the ones with meaningful data (Command Center, Model Manager,
// System Monitor, Settings) and stub pages for the rest with a clear "coming
// in Phase N" notice that links back to the roadmap.

import { useEffect, useMemo, useState } from "react";
import { NavLink, Route, Routes, useLocation } from "react-router-dom";
import { Icon } from "./components/Icon";
import { CommandPalette } from "./components/CommandPalette";
import { CommandCenterPage } from "./pages/CommandCenter";
import { AssistantPage } from "./pages/Assistant";
import { KnowledgePage } from "./pages/Knowledge";
import { ResearchPage } from "./pages/Research";
import { DocumentsPage } from "./pages/Documents";
import { ProjectsPage } from "./pages/Projects";
import { AgentPage } from "./pages/Agent";
import { ExperimentsPage } from "./pages/Experiments";
import { TasksPage } from "./pages/Tasks";
import { ModelsPage } from "./pages/Models";
import { MonitorPage } from "./pages/Monitor";
import { SettingsPage } from "./pages/Settings";
import { MemoryCenterPage } from "./pages/MemoryCenter";
import { KnowledgeGraphPage } from "./pages/KnowledgeGraph";
import { TimelinePage } from "./pages/Timeline";
import { CheckpointsPage } from "./pages/Checkpoints";
import { OpportunityRadarPage } from "./pages/OpportunityRadar";
import { OrchestratorPage } from "./pages/Orchestrator";
import { SidebarItem, listSidebar, dashboardSummary, openMainWindow } from "./lib/ipc";
import { ErrorBoundary } from "./components/ErrorBoundary";

export default function App() {
  const [items, setItems] = useState<SidebarItem[]>([]);
  const [collapsed, setCollapsed] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [status, setStatus] = useState<string>("Initializing");
  const loc = useLocation();

  useEffect(() => {
    listSidebar()
      .then(setItems)
      .catch((e) => setStatus(`Failed to load sidebar: ${e}`));
    dashboardSummary()
      .then((d) => setStatus(`Idle · ${d.app.name} v${d.app.version} · ${d.app.build_target}`))
      .catch((e) => setStatus(`Core unavailable: ${e}`));
    // Reveal the main window after the first paint.
    openMainWindow().catch(() => {});
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen((v) => !v);
      } else if (e.key === "Escape") {
        setPaletteOpen(false);
      } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "b") {
        e.preventDefault();
        setCollapsed((c) => !c);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const groups = useMemo(() => {
    const byGroup = new Map<string, SidebarItem[]>();
    for (const it of items) {
      const list = byGroup.get(it.group) ?? [];
      list.push(it);
      byGroup.set(it.group, list);
    }
    return Array.from(byGroup.entries());
  }, [items]);

  return (
    <div className="flex h-full w-full overflow-hidden">
      <aside
        className={`${collapsed ? "w-14" : "w-64"} shrink-0 border-r border-white/5 bg-ink-900/60 backdrop-blur-xs transition-all`}
      >
        <div className="flex h-14 items-center gap-2 px-3 border-b border-white/5">
          <div className="h-6 w-6 rounded-md bg-gradient-to-br from-cyan2-500 to-violet2-500 shadow-glow" />
          {!collapsed && (
            <div className="leading-tight">
              <div className="text-sm font-semibold tracking-wide">NEXUS</div>
              <div className="text-[10px] uppercase tracking-[0.18em] text-slate-500">Local AI Workbench</div>
            </div>
          )}
          <button
            className="ml-auto rounded-md p-1.5 text-slate-400 hover:bg-white/5"
            onClick={() => setCollapsed((c) => !c)}
            title={collapsed ? "Expand (Ctrl+B)" : "Collapse (Ctrl+B)"}
          >
            <Icon name="compass" />
          </button>
        </div>
        <nav className="px-2 py-3 space-y-4 overflow-y-auto h-[calc(100%-3.5rem)]">
          {groups.map(([group, list]) => (
            <div key={group}>
              {!collapsed && <div className="px-2 pb-1 text-[10px] uppercase tracking-[0.18em] text-slate-500">{group}</div>}
              <div className="space-y-0.5">
                {list.map((it) => (
                  <NavLink
                    key={it.id}
                    to={it.route}
                    className={({ isActive }) => `sidebar-item ${isActive ? "active" : ""}`}
                    end={it.route === "/"}
                  >
                    <Icon name={it.icon} />
                    {!collapsed && <span className="truncate">{it.label}</span>}
                  </NavLink>
                ))}
              </div>
            </div>
          ))}
        </nav>
      </aside>

      <main className="flex-1 min-w-0 flex flex-col">
        <header className="h-14 flex items-center gap-3 border-b border-white/5 bg-ink-900/40 px-4">
          <div className="text-sm text-slate-300">
            {items.find((i) => i.route === loc.pathname)?.label ?? "NEXUS"}
          </div>
          <div className="ml-auto flex items-center gap-2">
            <button
              className="btn"
              onClick={() => setPaletteOpen(true)}
              title="Open Command Palette (Ctrl+K)"
            >
              <span className="text-slate-400">Search & run</span>
              <span className="kbd">Ctrl</span>
              <span className="kbd">K</span>
            </button>
          </div>
        </header>

        <div className="flex-1 min-h-0 overflow-y-auto">
          <ErrorBoundary title="NEXUS route error" fallbackRoute="/">
            <Routes>
              <Route path="/" element={<CommandCenterPage />} />
              <Route path="/assistant" element={
                <ErrorBoundary title="Assistant" fallbackRoute="/"><AssistantPage /></ErrorBoundary>
              } />
              <Route path="/knowledge" element={
                <ErrorBoundary title="Knowledge Vault" fallbackRoute="/"><KnowledgePage /></ErrorBoundary>
              } />
              <Route path="/research" element={
                <ErrorBoundary title="Research" fallbackRoute="/"><ResearchPage /></ErrorBoundary>
              } />
              <Route path="/documents" element={
                <ErrorBoundary title="Documents" fallbackRoute="/"><DocumentsPage /></ErrorBoundary>
              } />
              <Route path="/projects" element={
                <ErrorBoundary title="Projects" fallbackRoute="/"><ProjectsPage /></ErrorBoundary>
              } />
              <Route path="/timeline" element={
                <ErrorBoundary title="Timeline" fallbackRoute="/"><TimelinePage /></ErrorBoundary>
              } />
              <Route path="/checkpoints" element={
                <ErrorBoundary title="Checkpoints" fallbackRoute="/"><CheckpointsPage /></ErrorBoundary>
              } />
              <Route path="/radar" element={
                <ErrorBoundary title="Opportunity Radar" fallbackRoute="/"><OpportunityRadarPage /></ErrorBoundary>
              } />
              <Route path="/memory" element={
                <ErrorBoundary title="Memory Center" fallbackRoute="/"><MemoryCenterPage /></ErrorBoundary>
              } />
              <Route path="/graph" element={
                <ErrorBoundary title="Knowledge Graph" fallbackRoute="/"><KnowledgeGraphPage /></ErrorBoundary>
              } />
              <Route path="/agent" element={
                <ErrorBoundary title="Agent" fallbackRoute="/"><AgentPage /></ErrorBoundary>
              } />
              <Route path="/experiments" element={
                <ErrorBoundary title="Experiments" fallbackRoute="/"><ExperimentsPage /></ErrorBoundary>
              } />
              <Route path="/tasks" element={
                <ErrorBoundary title="Tasks" fallbackRoute="/"><TasksPage /></ErrorBoundary>
              } />
              <Route path="/models" element={
                <ErrorBoundary title="Model Manager" fallbackRoute="/"><ModelsPage /></ErrorBoundary>
              } />
              <Route path="/orchestrator" element={
                <ErrorBoundary title="Orchestrator" fallbackRoute="/"><OrchestratorPage /></ErrorBoundary>
              } />
              <Route path="/monitor" element={
                <ErrorBoundary title="System Monitor" fallbackRoute="/"><MonitorPage /></ErrorBoundary>
              } />
              <Route path="/settings" element={
                <ErrorBoundary title="Settings" fallbackRoute="/"><SettingsPage /></ErrorBoundary>
              } />
            </Routes>
          </ErrorBoundary>
        </div>

        <footer className="h-9 shrink-0 border-t border-white/5 bg-ink-900/50 px-3 text-[12px] text-slate-400 flex items-center gap-3">
          <span className="h-1.5 w-1.5 rounded-full bg-cyan2-500 animate-breathe" />
          <span className="truncate">{status}</span>
          <span className="ml-auto font-mono text-[11px] text-slate-500">NEXUS 0.1.0 · local-first</span>
        </footer>
      </main>

      <CommandPalette
        open={paletteOpen}
        items={items}
        onClose={() => setPaletteOpen(false)}
      />
    </div>
  );
}
