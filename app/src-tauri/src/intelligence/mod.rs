//! Document Intelligence. Phase J.
//!
//! Pure-Rust helpers for working with text-heavy documents:
//!   * Markdown / pipe-table extraction (`tables::extract`)
//!   * CSV / TSV export (`csv::to_csv`)
//!   * Document comparison (line-level diff summary)
//!
//! These do not depend on external services; they are designed to
//! run on chunks the vault has already produced. A future iteration
//! can add proper PDF table extraction (camelot / pdfplumber) and
//! OCR (tesseract) as optional sidecar subprocesses; the trait
//! abstraction in `tables` keeps the rest of the system ready for
//! that.

pub mod tables;
pub mod csv;
pub mod diff;
