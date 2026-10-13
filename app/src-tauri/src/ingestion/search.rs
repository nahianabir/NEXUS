//! Hybrid search: FTS5 + vector + Reciprocal Rank Fusion. Either signal can
//! be disabled (e.g. the user has not enabled embeddings), in which case we
//! fall back to FTS5 alone.

use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db::SqlitePool;
use crate::error::AppResult;
use crate::ingestion::pipeline::{get_chunk, ChunkRow};
use crate::ingestion::vector;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub chunk: ChunkRow,
    pub score: f64,
    pub source: String, // "fts" | "vector" | "hybrid" | "docname"
}

/// Tokenize a free-form user query into FTS5-safe terms.
///
/// The previous implementation stripped every non-alphanumeric
/// character, which collapsed filenames like `Arduino.txt` into
/// the single token `Arduinotxt` and made them unsearchable. The
/// new implementation:
///
/// * Splits on whitespace *and* on common filename separators
///   (`.`, `-`, `_` are kept *inside* tokens so `Arduino.txt`
///   becomes two tokens `Arduino` and `txt`).
/// * Lowercases the tokens so the FTS5 unicode61 tokenizer can
///   match them case-insensitively (the index is
///   `tokenize='porter unicode61'`).
/// * Quotes each term in double quotes so FTS5 reserved
///   characters (e.g. `:`, `-`, `^`) are treated literally.
/// * Appends a `*` to the last term so prefix matching still
///   works (e.g. "Ardu" matches "Arduino").
/// * Strips FTS5 operators that the user might have pasted by
///   accident (`AND`, `OR`, `NOT`, `NEAR`).
pub fn tokenize_query(query: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let lower = query.to_ascii_lowercase();
    // Split on whitespace; then within each whitespace-delimited
    // word, also split on `.` `-` `_` so `Arduino.txt` ->
    // `Arduino` + `txt`. We deliberately keep the split
    // results order so a phrase like "my Arduino.txt file" yields
    // `["my", "arduino", "txt", "file"]`.
    for w in lower.split_whitespace() {
        // Split on filename separators but keep them as
        // separate terms. We *do not* split inside the
        // word for very short separators like `_` to avoid
        // `camel_case` becoming `camel`, `case`. We do split
        // on `.` and `-` because they are almost always
        // filename / version separators in user queries.
        let mut current = String::new();
        for c in w.chars() {
            if c == '.' || c == '-' {
                if !current.is_empty() {
                    push_token(&mut out, &current);
                    current.clear();
                }
            } else if c.is_alphanumeric() {
                current.push(c);
            } else {
                // Punctuation that is not a separator: emit
                // what we have and drop the punctuation.
                if !current.is_empty() {
                    push_token(&mut out, &current);
                    current.clear();
                }
            }
        }
        if !current.is_empty() {
            push_token(&mut out, &current);
        }
    }
    out
}

fn push_token(out: &mut Vec<String>, raw: &str) {
    let t = raw.trim();
    if t.is_empty() { return; }
    // Strip FTS5 reserved words. We compare lower-case; if
    // the original query happened to use them in a meaningful
    // way, the user can use the explicit search syntax. This
    // just protects against "AND" in a sentence producing an
    // FTS5 syntax error.
    match t {
        "and" | "or" | "not" | "near" => return,
        _ => {}
    }
    // Drop common English stop words so they do not
    // pollute the FTS5 query. These are tokens that
    // appear in almost every sentence and add nothing to
    // a search. We deliberately keep the list small —
    // every stop word we drop is one more chance for an
    // uncommon query to miss a real hit.
    if is_query_stop_word(t) {
        return;
    }
    out.push(format!("\"{}\"", t));
}

fn is_query_stop_word(t: &str) -> bool {
    matches!(t,
        "a" | "an" | "the" | "and" | "or" | "but" | "if" | "then" | "else" |
        "is" | "are" | "was" | "were" | "be" | "been" | "being" |
        "have" | "has" | "had" | "do" | "does" | "did" | "doing" |
        "of" | "to" | "in" | "on" | "at" | "by" | "for" | "with" | "about" |
        "this" | "that" | "these" | "those" |
        "from" | "as" | "into" | "out" | "up" | "down" | "over" | "under" |
        "i" | "you" | "we" | "they" | "he" | "she" | "it" |
        "my" | "your" | "our" | "their" | "his" | "her" | "its" |
        "what" | "which" | "who" | "whom" |
        "tell" | "show" | "explain" | "describe" | "say" | "please" | "help" |
        "read" | "open" | "give" | "find" | "get" | "use" | "using"
    )
}

pub fn search_fts(pool: &SqlitePool, query: &str, limit: usize) -> AppResult<Vec<(String, f64)>> {
    if query.trim().is_empty() { return Ok(Vec::new()); }
    let mut tokens = tokenize_query(query);
    if tokens.is_empty() { return Ok(Vec::new()); }
    // Add a trailing `*` to the last token for prefix matching
    // so "Ardu" still matches "Arduino". We do this by
    // unwrapping the quotes we added in tokenize_query,
    // appending the `*`, and re-wrapping.
    if let Some(last) = tokens.last_mut() {
        // last is `"foo"`; turn it into `"foo"*`.
        if last.starts_with('"') && last.ends_with('"') && last.len() >= 2 {
            let inner = &last[1..last.len() - 1];
            *last = format!("\"{}\"*", inner);
        }
    }
    let q = tokens.join(" ");

    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    // We join against document_chunks to translate rowid -> chunk_id.
    let mut stmt = conn.prepare(
        "SELECT c.id, bm25(chunks_fts) AS score
         FROM chunks_fts
         JOIN document_chunks c ON c.rowid = chunks_fts.rowid
         WHERE chunks_fts MATCH ?1
         ORDER BY score LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![q, limit as i64], |r| {
        // bm25 returns negative numbers; invert so higher = better.
        let raw: f64 = r.get(1)?;
        Ok((r.get::<_, String>(0)?, -raw))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, score) = row?;
        out.push((id, score));
    }
    Ok(out)
}

/// Look up documents whose title or path matches a hint in
/// the user query (e.g. the literal filename `Arduino.txt`).
/// Returns the document IDs that match. Used to give the
/// retrieval a "document name hint" so a query like
/// "Read the Arduino.txt" finds the file even when the
/// chunk text does not literally contain the filename.
pub fn find_documents_by_name_hint(pool: &SqlitePool, query: &str) -> AppResult<Vec<String>> {
    // Extract a likely filename from the query: a token
    // containing a `.` (e.g. `Arduino.txt`), or a
    // sufficiently long alphanumeric token surrounded by
    // whitespace / punctuation.
    let hints: Vec<String> = extract_filename_hints(query);
    if hints.is_empty() { return Ok(Vec::new()); }
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let mut out: Vec<String> = Vec::new();
    for h in hints {
        // Match either by title (basename) or by full path.
        // `LOWER(...) LIKE LOWER(?)` is case-insensitive.
        let like = format!("%{}%", h);
        let mut stmt = conn.prepare(
            "SELECT id FROM documents
             WHERE LOWER(title) LIKE ?1 OR LOWER(path) LIKE ?1
             LIMIT 5",
        )?;
        let rows = stmt.query_map(params![like], |r| r.get::<_, String>(0))?;
        for row in rows {
            if let Ok(id) = row {
                if !out.contains(&id) { out.push(id); }
            }
        }
    }
    Ok(out)
}

/// Pull filename-like hints from a free-form query. A
/// filename is any token that contains a `.` (e.g.
/// `Arduino.txt`). A long token without an extension is
/// only honoured as a *file* hint if the user also
/// used a file-noun ("file", "document", "note", "spec")
/// in the same query — otherwise every sufficiently long
/// word in the user's sentence would pull in unrelated
/// documents.
pub fn extract_filename_hints(query: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let lower = query.to_ascii_lowercase();
    // Detect file-noun presence; only allow long-token
    // title hints when one is present.
    let has_file_noun = lower.split_whitespace().any(|w| {
        let t: String = w.chars().filter(|c| c.is_alphanumeric()).collect();
        matches!(t.as_str(), "file" | "files" | "document" | "documents" | "note" | "notes" | "spec" | "doc" | "docs" | "paper" | "papers" | "text" | "script" | "code")
    });
    for raw in lower.split_whitespace() {
        // Trim surrounding punctuation.
        let trimmed: String = raw.chars()
            .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
            .collect();
        if trimmed.is_empty() { continue; }
        // Token contains a `.` — strong signal: this is
        // a literal filename. Always honour.
        if trimmed.contains('.') {
            if !out.contains(&trimmed) { out.push(trimmed); }
            continue;
        }
        // Long alphanumeric token is a weak signal.
        // Only honour it when the user clearly is
        // referring to a file ("the arduino file") and
        // the token is unlikely to be a common English
        // word. We require 7+ chars to reduce false
        // positives ("arduino" is 7, "sensor" is 6 and
        // could appear in many contexts).
        if has_file_noun && trimmed.len() >= 7 && !is_stop_word(&trimmed) {
            if !out.contains(&trimmed) { out.push(trimmed); }
        }
    }
    out
}

fn is_stop_word(s: &str) -> bool {
    matches!(s, "the" | "from" | "with" | "this" | "that" | "what" | "does" | "tell" | "about" | "have" | "file" | "files" | "read" | "show" | "please")
}

/// Look up every chunk for a given document ID. Used to
/// inject the full document when a name hint matches but the
/// FTS5 search returned nothing.
pub fn chunks_for_document(pool: &SqlitePool, document_id: &str) -> AppResult<Vec<ChunkRow>> {
    let conn = pool.get().map_err(|e| crate::error::AppError::Internal(format!("pool: {e}")))?;
    let mut stmt = conn.prepare(
        "SELECT c.id, c.document_id, c.ordinal, c.text, c.page, c.section,
                c.start_offset, c.end_offset, d.title, d.path
         FROM document_chunks c
         JOIN documents d ON d.id = c.document_id
         WHERE c.document_id = ?1
         ORDER BY c.ordinal
         LIMIT 50",
    )?;
    let rows = stmt.query_map(params![document_id], |r| Ok(ChunkRow {
        id: r.get(0)?,
        document_id: r.get(1)?,
        ordinal: r.get(2)?,
        text: r.get(3)?,
        page: r.get(4)?,
        section: r.get(5)?,
        start_offset: r.get(6)?,
        end_offset: r.get(7)?,
        document_title: r.get(8)?,
        document_path: r.get(9)?,
    }))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub hits: Vec<SearchHit>,
    pub used_vector: bool,
    /// True if the search was forced to retrieve a specific
    /// document by name hint rather than by semantic /
    /// keyword match. The chat UI can use this to label the
    /// citations accordingly.
    pub used_docname_hint: bool,
    /// The document IDs that were matched by name hint, if
    /// any. Useful for diagnostics and for the chat to
    /// surface a "this came from the file you mentioned"
    /// marker in the answer.
    pub docname_hint_doc_ids: Vec<String>,
}

pub async fn search_hybrid(
    pool: SqlitePool,
    query: &str,
    query_embedding: Option<Vec<f32>>,
    limit: usize,
) -> AppResult<SearchResult> {
    let fts = search_fts(&pool, query, limit.max(50))?;
    let mut used_vector = false;
    let vector_hits: Vec<(String, f64)> = if let Some(qe) = query_embedding.as_ref() {
        used_vector = true;
        let raw = vector::search(&pool, qe, limit.max(50))?;
        raw.into_iter().map(|h| (h.chunk_id, 1.0 / (1.0 + h.distance as f64))).collect()
    } else {
        Vec::new()
    };

    // Reciprocal Rank Fusion with k = 60.
    let mut scores: std::collections::HashMap<String, (f64, String)> = std::collections::HashMap::new();
    for (rank, (id, _s)) in fts.iter().enumerate() {
        let rrf = 1.0 / (60.0 + (rank as f64) + 1.0);
        scores.entry(id.clone())
            .and_modify(|(score, src)| { *score += rrf; if src.as_str() == "fts" { *src = "hybrid".into(); } })
            .or_insert((rrf, "fts".into()));
    }
    for (rank, (id, _s)) in vector_hits.iter().enumerate() {
        let rrf = 1.0 / (60.0 + (rank as f64) + 1.0);
        scores.entry(id.clone())
            .and_modify(|(score, src)| { *score += rrf; if src.as_str() == "vector" { *src = "hybrid".into(); } })
            .or_insert((rrf, "vector".into()));
    }

    // Document-name hint: if the user mentioned a filename
    // in the query, also fetch all chunks of that document
    // so the model can answer questions like "Read the
    // Arduino.txt". The FTS5 search alone will not match
    // because the chunk text does not contain the literal
    // filename. We merge the hinted-document chunks into
    // the result with a synthetic "docname" source so the
    // chat can label them.
    let mut used_docname_hint = false;
    let mut docname_hint_doc_ids: Vec<String> = Vec::new();
    let hint_doc_ids = find_documents_by_name_hint(&pool, query)?;
    if !hint_doc_ids.is_empty() {
        used_docname_hint = true;
        docname_hint_doc_ids = hint_doc_ids.clone();
        for doc_id in hint_doc_ids {
            let doc_chunks = chunks_for_document(&pool, &doc_id)?;
            for chunk in doc_chunks {
                scores.entry(chunk.id.clone())
                    .and_modify(|(score, src)| {
                        // Strongly boost the score so the
                        // hinted chunks appear at the top
                        // when the user named a file
                        // explicitly.
                        *score += 0.5;
                        if src.as_str() == "fts" || src.as_str() == "vector" {
                            *src = "hybrid".into();
                        }
                    })
                    .or_insert((0.5, "docname".into()));
            }
        }
    }
    // Now build the combined list so the docname-hinted chunks
    // are included in the final ranking.
    let mut combined: Vec<(String, f64, String)> = scores.into_iter().map(|(id, (s, src))| (id, s, src)).collect();
    combined.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    combined.truncate(limit);

    let mut hits = Vec::new();
    for (id, score, source) in combined {
        if let Some(Some(chunk)) = Some(get_chunk(&pool, &id)?) {
            hits.push(SearchHit { chunk, score, source });
        }
    }
    Ok(SearchResult { hits, used_vector, used_docname_hint, docname_hint_doc_ids })
}