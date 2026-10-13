//! NEXUS core library. This is the cross-cutting logic — config, database,
//! ingestion, providers, agent loop, telemetry — that is shared between the
//! Tauri binary and the test suite.
//!
//! Tauri-specific bootstrap (`main.rs`) and Tauri commands (`commands` module
//! in the binary crate) are not exposed because they pull in WebView2; they
//! live in the binary.

pub mod app_state;
pub mod agent;
pub mod audit;
pub mod auto_title;
pub mod config;
pub mod context;
pub mod conversations;
pub mod data_dir;
pub mod db;
pub mod error;
pub mod ingestion;
pub mod intelligence;
pub mod kg;
pub mod logging;
pub mod mempalace;
pub mod memory;
pub mod memory_v2;
pub mod model_profiles;
pub mod experiments;
pub mod research;
pub mod monitor;
pub mod projects;
pub mod providers;
pub mod router;
pub mod timeline;
pub mod checkpoints;
pub mod radar;
pub mod radar_feedback;
pub mod hypotheses;
pub mod archaeologist;
pub mod serendipity;
pub mod impact;
pub mod tribunal;
pub mod vault_export;
pub mod sandbox;