//! Codebase Archaeologist. S.3.3.
//!
//! Read-only filesystem scan. The archaeologist walks a
//! user-supplied directory, builds an index of files (path,
//! size, mtime, kind, sha256), and can answer a few derived
//! questions:
//!
//!   * `index(root, opts)` — walks the tree and returns an
//!     `IndexSummary` with the file list and aggregate stats.
//!   * `find_by_name(index, query)` — case-insensitive search.
//!   * `head(path, n)` — read the first N lines of a file
//!     (capped at 1 MiB to avoid OOM).
//!   * `summarize(path, opts)` — return the head plus the
//!     sha256, size, and a short "language guess" based on
//!     extension.
//!
//! All operations are read-only. The archaeologist never
//! creates, modifies, or deletes any file. It does not follow
//! symlinks (it would loop otherwise) and it honours a small
//! set of exclusions (`.git`, `node_modules`, `target`,
//! `dist`, `__pycache__`, `.venv`).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};

pub const MAX_FILE_BYTES: u64 = 1_048_576; // 1 MiB
pub const MAX_FILES_PER_INDEX: usize = 50_000;
pub const MAX_DIR_DEPTH: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Markdown,
    Toml,
    Yaml,
    Json,
    Sql,
    Shell,
    Text,
    Binary,
    Other,
}

impl FileKind {
    pub fn from_path(p: &Path) -> Self {
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        let lowered = name.to_ascii_lowercase();
        if lowered == "cargo.toml" { return Self::Toml; }
        if lowered == "package.json" || lowered == "tsconfig.json" {
            return Self::Json;
        }
        match p.extension().and_then(|s| s.to_str()) {
            Some("rs") => Self::Rust,
            Some("ts") | Some("tsx") => Self::TypeScript,
            Some("js") | Some("jsx") | Some("mjs") | Some("cjs") => Self::JavaScript,
            Some("py") => Self::Python,
            Some("md") | Some("markdown") => Self::Markdown,
            Some("toml") => Self::Toml,
            Some("yaml") | Some("yml") => Self::Yaml,
            Some("json") => Self::Json,
            Some("sql") => Self::Sql,
            Some("sh") | Some("bash") => Self::Shell,
            Some("txt") => Self::Text,
            _ => Self::Other,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Markdown => "markdown",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::Json => "json",
            Self::Sql => "sql",
            Self::Shell => "shell",
            Self::Text => "text",
            Self::Binary => "binary",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size_bytes: u64,
    pub mtime_ms: i64,
    pub kind: FileKind,
    /// SHA-256 of the file contents, hex-encoded. `None` if the
    /// file was too large to hash or the read failed.
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSummary {
    pub root: String,
    pub total_files: usize,
    pub total_bytes: u64,
    pub by_kind: std::collections::BTreeMap<String, usize>,
    pub files: Vec<FileEntry>,
    /// File paths that were skipped (permission denied, too big, symlink loop).
    pub skipped: Vec<String>,
    pub indexed_at: i64,
}

#[derive(Debug, Clone)]
pub struct IndexOptions {
    /// Whether to compute SHA-256 for each file. False for very
    /// large repos where the hash cost is too high.
    pub hash: bool,
    /// Maximum file size to include in the index. Files above
    /// this size are recorded with a sentinel and skipped
    /// from the head() / summarize() calls.
    pub max_file_bytes: u64,
    /// Maximum number of files to index.
    pub max_files: usize,
    /// Maximum recursion depth.
    pub max_depth: usize,
    /// Substrings whose presence in any path component causes
    /// the entry to be skipped.
    pub exclusions: Vec<String>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            hash: true,
            max_file_bytes: MAX_FILE_BYTES,
            max_files: MAX_FILES_PER_INDEX,
            max_depth: MAX_DIR_DEPTH,
            exclusions: vec![
                ".git".into(),
                "node_modules".into(),
                "target".into(),
                "dist".into(),
                "__pycache__".into(),
                ".venv".into(),
                ".next".into(),
                "build".into(),
            ],
        }
    }
}

/// Walk `root` and return an index.
pub fn index(root: &Path, opts: &IndexOptions) -> AppResult<IndexSummary> {
    let root = std::fs::canonicalize(root)
        .map_err(|e| AppError::Invalid(format!("archaeologist root: {e}")))?;
    if !root.is_dir() {
        return Err(AppError::Invalid(format!(
            "archaeologist root is not a directory: {}",
            root.display()
        )));
    }
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    let mut total_bytes: u64 = 0;
    let now = chrono::Utc::now().timestamp_millis();
    walk(
        &root,
        &root,
        opts,
        0,
        &mut files,
        &mut skipped,
        &mut total_bytes,
    )?;
    let mut by_kind: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for f in &files {
        *by_kind.entry(f.kind.as_str().to_string()).or_insert(0) += 1;
    }
    Ok(IndexSummary {
        root: root.display().to_string(),
        total_files: files.len(),
        total_bytes,
        by_kind,
        files,
        skipped,
        indexed_at: now,
    })
}

fn walk(
    root: &Path,
    dir: &Path,
    opts: &IndexOptions,
    depth: usize,
    files: &mut Vec<FileEntry>,
    skipped: &mut Vec<String>,
    total_bytes: &mut u64,
) -> AppResult<()> {
    if depth > opts.max_depth { return Ok(()); }
    if files.len() >= opts.max_files { return Ok(()); }
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            skipped.push(format!("{}: read_dir: {e}", dir.display()));
            return Ok(());
        }
    };
    for entry in read.flatten() {
        if files.len() >= opts.max_files { return Ok(()); }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if opts.exclusions.iter().any(|x| name == x.as_str()) {
            continue;
        }
        // Symlink safety: never follow.
        let ft = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ft.is_symlink() { continue; }
        let path = entry.path();
        if ft.is_dir() {
            walk(root, &path, opts, depth + 1, files, skipped, total_bytes)?;
            continue;
        }
        if !ft.is_file() { continue; }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                skipped.push(format!("{}: metadata: {e}", path.display()));
                continue;
            }
        };
        if meta.len() > opts.max_file_bytes {
            skipped.push(format!("{}: too large ({} bytes)", path.display(), meta.len()));
            continue;
        }
        let kind = FileKind::from_path(&path);
        let mtime_ms = meta.modified().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let sha = if opts.hash { hash_file(&path).ok() } else { None };
        let rel = pathdiff(&root, &path);
        files.push(FileEntry {
            path: rel,
            size_bytes: meta.len(),
            mtime_ms,
            kind,
            sha256: sha,
        });
        *total_bytes = total_bytes.saturating_add(meta.len());
    }
    Ok(())
}

fn pathdiff(root: &Path, p: &Path) -> String {
    match p.strip_prefix(root) {
        Ok(rel) => rel.display().to_string(),
        Err(_) => p.display().to_string(),
    }
}

fn hash_file(p: &Path) -> AppResult<String> {
    let bytes = std::fs::read(p)
        .map_err(|e| AppError::Internal(format!("read {}: {e}", p.display())))?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(format!("{:x}", h.finalize()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadResult {
    pub path: String,
    pub size_bytes: u64,
    pub total_lines: usize,
    pub head: String,
    pub truncated: bool,
    pub sha256: Option<String>,
}

pub fn head(path: &Path, n: usize) -> AppResult<HeadResult> {
    let n = n.clamp(1, 10_000);
    let bytes = std::fs::read(path)
        .map_err(|e| AppError::Invalid(format!("archaeologist head: {e}")))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(AppError::Invalid(format!(
            "archaeologist head: file too large ({} bytes, max {MAX_FILE_BYTES})",
            bytes.len()
        )));
    }
    let text = String::from_utf8_lossy(&bytes);
    let total_lines = text.lines().count();
    let head_lines: Vec<&str> = text.lines().take(n).collect();
    let truncated = total_lines > head_lines.len();
    let head_text = head_lines.join("\n");
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(HeadResult {
        path: path.display().to_string(),
        size_bytes: bytes.len() as u64,
        total_lines,
        head: head_text,
        truncated,
        sha256: Some(format!("{:x}", h.finalize())),
    })
}

pub fn find_by_name<'a>(index: &'a IndexSummary, query: &str) -> Vec<&'a FileEntry> {
    let q = query.to_ascii_lowercase();
    index.files.iter()
        .filter(|f| f.path.to_ascii_lowercase().contains(&q))
        .collect()
}

pub fn resolve_under(root: &Path, rel: &str) -> AppResult<PathBuf> {
    // The Archaeologist only operates under `root`; reject
    // anything that would escape via `..`.
    let p = root.join(rel);
    let p = std::fs::canonicalize(&p)
        .map_err(|e| AppError::Invalid(format!("archaeologist resolve: {e}")))?;
    let canon_root = std::fs::canonicalize(root)
        .map_err(|e| AppError::Invalid(format!("archaeologist root: {e}")))?;
    if !p.starts_with(&canon_root) {
        return Err(AppError::Invalid(format!(
            "archaeologist resolve: '{rel}' escapes root"
        )));
    }
    Ok(p)
}
