//! Model Router. Phase F.
//!
//! The router picks a configured `ModelProfile` for each task. It
//! extends the existing model_profiles table with capability metadata,
//! lifecycle state, and a routing policy. It is intentionally local:
//! no remote orchestration service, no agent-process tree — NEXUS is
//! single-user, single-machine, and the only thing the router has to
//! choose between is the set of profiles the user has configured
//! against the local LM Studio / llama.cpp / Ollama endpoints.
//!
//! The lifecycle is:
//!   Unloaded → Loading → Ready → Busy → Unloading → Unloaded
//!                  ↘        ↘       ↘
//!                   Failure states.
//!
//! Heuristics:
//!   * `general` role is the default for general conversation.
//!   * `coding` role is preferred for code (matches a task classifier).
//!   * `embedding` is used only for embedding calls.
//!   * One heavyweight generation model loaded at a time; switching
//!     first unloads the active one (in a real llama.cpp / Ollama
//!     deployment this would call /unload; for the LM Studio
//!     backend we record the transition but cannot force the backend
//!     to drop a model — that is why the lifecycle is observable and
//!     fallbacks exist).
//!   * Hard constraint: do not load two large profiles in parallel.

pub mod capabilities;
pub mod lifecycle;
pub mod policy;
pub mod state;

// Re-export data types only. Submodule functions (list, get, etc.)
// would clash on names and must be accessed via the full path.
pub use capabilities::Capabilities;
pub use lifecycle::{State, Status as LifecycleStatus};
pub use policy::{DecisionRecord, RejectedCandidate, RouteRequest, RoutingDecision, TaskKind};
