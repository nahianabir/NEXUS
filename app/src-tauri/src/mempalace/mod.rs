//! MemPalace adapter abstraction. Phase C.
//!
//! Three concrete backends:
//!   * `Native`    — always-available, just delegates to the native
//!                    memories_v2 store. The default.
//!   * `MemPalace` — talks to a Python sidecar over stdin/stdout JSON-RPC
//!                    (one process per call, spawned by `mempalace::sidecar`).
//!                    Disabled until the user installs MemPalace and
//!                    configures the binary path in Settings.
//!   * `Disabled`  — explicit no-op (e.g. the user toggled the integration
//!                    off in Settings).
//!
//! The adapter trait is intentionally small: every operation the assistant
//! and Memory Center can request is named and typed. Unsupported operations
//! return `AdapterError::Unsupported` and the caller falls back to the
//! native backend.
//!
//! Live MemPalace tests are gated behind `--ignored` because we cannot
//! install the sidecar in this build environment. Run them with:
//!     `cargo test mempalace -- --ignored --nocapture`
//! after installing MemPalace per `docs/MEMPALACE_INTEGRATION.md`.

pub mod adapter;
pub mod native;
pub mod sidecar;
pub mod state;

pub use adapter::*;
pub use native::*;
pub use sidecar::*;
pub use state::*;
