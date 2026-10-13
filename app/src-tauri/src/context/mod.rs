//! Unified context builder. Phase E.
//!
//! The builder assembles, in priority order, the chunks of context the
//! assistant should see for a given user message:
//!
//!   1. Active project context (instructions, goals, notes).
//!   2. Approved native memories (optionally scoped to project).
//!   3. MemPalace adapter (if the user has activated it).
//!   4. Knowledge Vault (FTS5 + vector, with RRF fusion).
//!   5. Knowledge graph neighborhood (entities + edges + evidence).
//!
//! Each item carries its source. Nothing is invented; if the graph
//! has nothing to say about the entities named in the query, the
//! `graph` section is empty rather than fabricated.
//!
//! The builder returns a `ContextBundle` with a `system_block`
//! formatted in plain text and a typed list of items for the
//! activity inspector.

pub mod builder;
pub mod types;

pub use builder::*;
pub use types::*;
