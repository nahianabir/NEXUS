//! Ingestion pipeline for the Knowledge Vault.
//!
//! The pipeline is: discover -> hash -> extract -> chunk -> embed -> index.
//! Each stage writes its progress to the `documents` and `document_chunks`
//! tables; the UI can poll `documents.status` to render progress.

pub mod extractors;
pub mod chunker;
pub mod citation;
pub mod hash;
pub mod orchestrator;
pub mod pipeline;
pub mod vector;
pub mod search;
