// Settings bridge. The Rust side owns the schema; the Web side persists
// changes through `app_settings` via Tauri commands. Phase 1 reads/writes
// are still done by re-saving the whole file from the in-memory struct,
// so we round-trip through a `set_config` command that the Rust side
// exposes in later phases. For now we keep the file in localStorage so
// the UI is responsive even before the IPC endpoint is wired.

export type AppSettings = {
  general: {
    theme: string;
    sidebar_collapsed: boolean;
    reduced_motion: boolean;
    allow_network: boolean;
  };
  providers: {
    lmstudio_base_url: string;
    lmstudio_managed: boolean;
    llamacpp_base_url: string;
    ollama_base_url: string;
  };
  ingestion: {
    chunk_tokens: number;
    chunk_overlap: number;
    max_concurrency: number;
    max_file_mb: number;
    embedding_enabled: boolean;
    embedding_model: string;
  };
  agent: {
    default_permission: string;
    max_steps: number;
    step_timeout_ms: number;
    file_allowlist: string[];
    shell_allowlist: string[];
    audit_retention_days: number;
    audit_prune_on_boot: boolean;
  };
};

const DEFAULT: AppSettings = {
  general: { theme: "dark", sidebar_collapsed: false, reduced_motion: false, allow_network: false },
  providers: {
    lmstudio_base_url: "http://127.0.0.1:1234/v1",
    lmstudio_managed: false,
    llamacpp_base_url: "http://127.0.0.1:8081/v1",
    ollama_base_url: "http://127.0.0.1:11434",
  },
  ingestion: { chunk_tokens: 500, chunk_overlap: 80, max_concurrency: 2, max_file_mb: 64, embedding_enabled: false, embedding_model: "text-embedding-nomic-embed-text-v1.5" },
  agent: {
    default_permission: "readonly",
    max_steps: 8,
    step_timeout_ms: 30000,
    file_allowlist: [],
    shell_allowlist: [],
    audit_retention_days: 0,
    audit_prune_on_boot: false,
  },
};

const KEY = "nexus.settings.v1";

function read(): AppSettings {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return DEFAULT;
    return { ...DEFAULT, ...JSON.parse(raw) } as AppSettings;
  } catch {
    return DEFAULT;
  }
}

function write(next: AppSettings) {
  localStorage.setItem(KEY, JSON.stringify(next));
  // Fire a custom event so other components can react.
  window.dispatchEvent(new CustomEvent("nexus:settings", { detail: next }));
}

export const settings = {
  async get(): Promise<AppSettings> {
    // Wait for Rust to push canonical config if available; for Phase 1 we
    // use localStorage as the source of truth, but the schema mirrors the
    // Rust `AppConfig` struct exactly.
    return read();
  },
  async save(next: AppSettings): Promise<void> {
    write(next);
  },
};
