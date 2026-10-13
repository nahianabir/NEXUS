//! Application configuration loaded from `<data_dir>/config.toml`.
//!
//! The configuration is intentionally small in Phase 1. New sections land with the
//! modules that need them so the file stays discoverable.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_general")]
    pub general: GeneralConfig,
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub ingestion: IngestionConfig,
    #[serde(default)]
    pub agent: AgentConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub data_dir: PathBuf,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_sidebar_collapsed")]
    pub sidebar_collapsed: bool,
    #[serde(default)]
    pub reduced_motion: bool,
    #[serde(default = "default_network")]
    pub allow_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvidersConfig {
    #[serde(default = "default_lmstudio_url")]
    pub lmstudio_base_url: String,
    #[serde(default)]
    pub lmstudio_managed: bool,
    #[serde(default = "default_llamacpp_url")]
    pub llamacpp_base_url: String,
    #[serde(default = "default_ollama_url")]
    pub ollama_base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestionConfig {
    #[serde(default = "default_chunk_size")]
    pub chunk_tokens: usize,
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: usize,
    #[serde(default = "default_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_max_file_mb")]
    pub max_file_mb: u64,
    #[serde(default)]
    pub embedding_enabled: bool,
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default = "default_permission")]
    pub default_permission: String,
    #[serde(default = "default_max_steps")]
    pub max_steps: u32,
    #[serde(default = "default_step_timeout_ms")]
    pub step_timeout_ms: u64,
    /// Directories the agent's `file.read` / `file.list` tools may
    /// touch. Empty by default — the user opts in.
    #[serde(default)]
    pub file_allowlist: Vec<String>,
    /// Command basenames the agent's `shell.run` tool may invoke.
    /// Empty by default.
    #[serde(default)]
    pub shell_allowlist: Vec<String>,
    /// Audit log retention in days. Events older than this are pruned
    /// on boot when `audit_prune_on_boot` is true. 0 = keep forever.
    #[serde(default)]
    pub audit_retention_days: u32,
    #[serde(default)]
    pub audit_prune_on_boot: bool,
}

fn default_general() -> GeneralConfig {
    GeneralConfig {
        data_dir: PathBuf::new(),
        theme: default_theme(),
        sidebar_collapsed: default_sidebar_collapsed(),
        reduced_motion: false,
        allow_network: false,
    }
}
fn default_theme() -> String { "dark".into() }
fn default_sidebar_collapsed() -> bool { false }
fn default_network() -> bool { false }
fn default_lmstudio_url() -> String { "http://127.0.0.1:1234/v1".into() }
fn default_llamacpp_url() -> String { "http://127.0.0.1:8081/v1".into() }
fn default_ollama_url() -> String { "http://127.0.0.1:11434".into() }
fn default_chunk_size() -> usize { 500 }
fn default_chunk_overlap() -> usize { 80 }
fn default_concurrency() -> usize { 2 }
fn default_max_file_mb() -> u64 { 64 }
fn default_embedding_model() -> String { "text-embedding-nomic-embed-text-v1.5".into() }
fn default_permission() -> String { "readonly".into() }
fn default_max_steps() -> u32 { 8 }
fn default_step_timeout_ms() -> u64 { 30_000 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: default_general(),
            providers: ProvidersConfig::default(),
            ingestion: IngestionConfig::default(),
            agent: AgentConfig::default(),
        }
    }
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            lmstudio_base_url: default_lmstudio_url(),
            lmstudio_managed: false,
            llamacpp_base_url: default_llamacpp_url(),
            ollama_base_url: default_ollama_url(),
        }
    }
}
impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            chunk_tokens: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
            max_concurrency: default_concurrency(),
            max_file_mb: default_max_file_mb(),
            embedding_enabled: false,
            embedding_model: default_embedding_model(),
        }
    }
}
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            default_permission: default_permission(),
            max_steps: default_max_steps(),
            step_timeout_ms: default_step_timeout_ms(),
            file_allowlist: Vec::new(),
            shell_allowlist: Vec::new(),
            audit_retention_days: 0,
            audit_prune_on_boot: false,
        }
    }
}

impl AppConfig {
    pub fn path_for(data_dir: &Path) -> PathBuf {
        data_dir.join("config.toml")
    }

    pub fn load(data_dir: &Path) -> AppResult<Self> {
        let p = Self::path_for(data_dir);
        if !p.exists() {
            let mut cfg = AppConfig::default();
            cfg.general.data_dir = data_dir.to_path_buf();
            cfg.save(data_dir)?;
            return Ok(cfg);
        }
        let text = std::fs::read_to_string(&p)?;
        let mut cfg: AppConfig = toml::from_str(&text).map_err(|e| AppError::Config(e.to_string()))?;
        if cfg.general.data_dir.as_os_str().is_empty() {
            cfg.general.data_dir = data_dir.to_path_buf();
        }
        Ok(cfg)
    }

    pub fn save(&self, data_dir: &Path) -> AppResult<()> {
        let p = Self::path_for(data_dir);
        let text = toml::to_string_pretty(self).map_err(|e| AppError::Config(e.to_string()))?;
        std::fs::write(p, text)?;
        Ok(())
    }
}
