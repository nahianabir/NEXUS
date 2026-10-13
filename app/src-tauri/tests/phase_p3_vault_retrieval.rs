//! Phase P3 — Knowledge Vault retrieval regression tests.
//!
//! The user reported that "Read the Arduino.txt from Knowledge
//! Vault" returned a generic "I don't have access" answer even
//! though the file was indexed. The root cause was twofold:
//!
//! 1. The FTS5 query builder stripped non-alphanumeric characters,
//!    so `Arduino.txt` was tokenised as the single nonsense token
//!    `Arduinotxt` and could not match the chunk text.
//! 2. There was no document-name hint extraction, so even when
//!    the chunk text did not contain the filename, the system had
//!    no way to surface the document the user explicitly named.
//!
//! These tests exercise the underlying library functions the chat
//! orchestrator calls, so a fix in the search layer is enough to
//! satisfy acceptance criterion A1 (Chat retrieves the actual
//! indexed contents of `Arduino.txt`).

mod common;

use nexus_core::ingestion::search;
use nexus_core::ingestion::vector;

fn pool() -> nexus_core::db::SqlitePool {
    let dir = common::tempdir();
    let db = nexus_core::db::default_db_path(&dir);
    let pool = nexus_core::db::open_pool(&db).expect("open pool");
    {
        let conn = pool.get().unwrap();
        let mut conn = conn;
        nexus_core::db::migrations::run(&mut conn).expect("migrations");
    }
    // Ensure the vec0 shadow table is set up if a vector
    // search is later needed by another test.
    let _ = vector::ensure_dimension_table(&pool, 384);
    pool
}

/// Insert a document and a single chunk, returning the
/// document id. The chunk is written to the FTS5 index via
/// the trigger defined in migration 0004.
fn insert_doc(pool: &nexus_core::db::SqlitePool, title: &str, path: &str, text: &str) -> String {
    use ulid::Ulid;
    let now = chrono::Utc::now().timestamp_millis();
    let doc_id = Ulid::new().to_string();
    let chunk_id = Ulid::new().to_string();
    // The content_hash column is UNIQUE. Use a hash derived
    // from the path so the same document can be inserted
    // once per test without violating the constraint.
    let mut hasher = sha2::Sha256::new();
    use sha2::Digest;
    hasher.update(path.as_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let conn = pool.get().unwrap();
    conn.execute(
        "INSERT INTO documents (id, source_id, title, path, size_bytes, content_hash, status, created_at, updated_at)
         VALUES (?1, NULL, ?2, ?3, 0, ?4, 'indexed', ?5, ?5)",
        rusqlite::params![doc_id, title, path, hash, now],
    ).expect("insert doc");
    conn.execute(
        "INSERT INTO document_chunks (id, document_id, ordinal, text, token_estimate, start_offset, end_offset)
         VALUES (?1, ?2, 0, ?3, 8, 0, ?4)",
        rusqlite::params![chunk_id, doc_id, text, text.len() as i64],
    ).expect("insert chunk");
    doc_id
}

// ------------------------------------------------------------------
// 1. FTS5 query tokenisation: "Arduino.txt" must become a query
//    that matches chunks containing the word "Arduino" without
//    the period stripping collapsing it to "Arduinotxt".
// ------------------------------------------------------------------
#[test]
fn fts_tokenize_preserves_filename_components() {
    let tokens = search::tokenize_query("Read the Arduino.txt from Knowledge Vault");
    // We expect Arduino, txt as separate tokens, NOT
    // Arduinotxt. Stop words ("the", "from") are dropped.
    assert!(tokens.iter().any(|t| t.contains("arduino")), "must include 'arduino' as a token, got {:?}", tokens);
    assert!(tokens.iter().any(|t| t.contains("txt")), "must include 'txt' as a token, got {:?}", tokens);
    assert!(!tokens.iter().any(|t| t.contains("arduinotxt")), "must NOT collapse to 'Arduinotxt', got {:?}", tokens);
}

// ------------------------------------------------------------------
// 2. The hybrid search path (the one the chat actually uses)
//    must retrieve the Arduino document when the user names
//    it, even when the chunk text does not contain the
//    filename.
// ------------------------------------------------------------------
#[tokio::test]
async fn search_finds_chunks_when_query_includes_filename() {
    let p = pool();
    let _doc = insert_doc(
        &p,
        "Arduino.txt",
        "/Users/Abir/notes/Arduino.txt",
        "void setup() { pinMode(13, OUTPUT); } void loop() { digitalWrite(13, HIGH); }",
    );
    let res = search::search_hybrid(
        p.clone(),
        "Read the Arduino.txt from Knowledge Vault and tell me what it does",
        None,
        5,
    ).await.expect("hybrid");
    assert!(!res.hits.is_empty(), "hybrid search must retrieve Arduino.txt via the docname hint, got {:?}", res.hits);
    assert!(res.used_docname_hint, "the docname hint path must be flagged as used, got {:?}", res);
    assert!(res.hits[0].chunk.document_title.contains("Arduino"), "first hit must come from Arduino.txt, got {:?}", res.hits[0].chunk);
}

#[tokio::test]
async fn search_finds_chunks_by_filename_hint() {
    let p = pool();
    let _doc = insert_doc(
        &p,
        "Arduino.txt",
        "/Users/Abir/notes/Arduino.txt",
        "void setup() { pinMode(13, OUTPUT); } void loop() { digitalWrite(13, HIGH); }",
    );
    let res = search::search_hybrid(
        p.clone(),
        "Read the Arduino.txt from Knowledge Vault and tell me what it does",
        None,
        5,
    ).await.expect("hybrid");
    assert!(!res.hits.is_empty(), "hybrid search must return at least one hit, got {:?}", res.hits);
    assert!(res.used_docname_hint, "docname hint must be marked as used, got {:?}", res);
    assert!(!res.docname_hint_doc_ids.is_empty(), "docname hint must surface at least one document id, got {:?}", res);
    // The hit must be from the Arduino.txt document.
    let first = &res.hits[0];
    assert!(first.chunk.document_title.contains("Arduino"), "first hit must come from Arduino.txt, got {:?}", first.chunk.document_title);
}

#[tokio::test]
async fn search_returns_no_hits_for_unrelated_query() {
    let p = pool();
    let _doc = insert_doc(
        &p,
        "Arduino.txt",
        "/Users/Abir/notes/Arduino.txt",
        "void setup() { pinMode(13, OUTPUT); } void loop() { digitalWrite(13, HIGH); }",
    );
    let res = search::search_hybrid(
        p.clone(),
        "completely unrelated to electronics or microcontrollers",
        None,
        5,
    ).await.expect("hybrid");
    // The query is unrelated so the result is empty.
    assert!(res.hits.is_empty(), "unrelated query must not surface Arduino, got {:?}", res.hits);
    assert!(!res.used_docname_hint, "docname hint must NOT be used for an unrelated query");
}

#[tokio::test]
async fn search_handles_typed_extension_separately() {
    // Simulates the user typing "Arduino" (no extension) and
    // matching via the title hint path. The user uses the
    // word "file" which signals file-noun intent.
    let p = pool();
    let _doc = insert_doc(
        &p,
        "Arduino.txt",
        "/Users/Abir/notes/Arduino.txt",
        "void setup() { pinMode(13, OUTPUT); } void loop() { digitalWrite(13, HIGH); }",
    );
    let res = search::search_hybrid(
        p.clone(),
        "what is the Arduino file",
        None,
        5,
    ).await.expect("hybrid");
    assert!(!res.hits.is_empty(), "long-token title hint should retrieve the file, got {:?}", res.hits);
}

#[tokio::test]
async fn stop_words_are_dropped_from_query() {
    let tokens = search::tokenize_query("the quick brown fox jumps over the lazy dog");
    // The "the" must be gone but the rest must remain.
    assert!(!tokens.iter().any(|t| t == "\"the\""), "stop word 'the' must be dropped, got {:?}", tokens);
    assert!(tokens.iter().any(|t| t == "\"quick\""), "got {:?}", tokens);
    assert!(tokens.iter().any(|t| t == "\"brown\""), "got {:?}", tokens);
    assert!(tokens.iter().any(|t| t == "\"fox\""), "got {:?}", tokens);
}

#[tokio::test]
async fn empty_query_returns_no_hits() {
    let p = pool();
    let res = search::search_hybrid(p.clone(), "   ", None, 5).await.expect("hybrid");
    assert!(res.hits.is_empty());
}

#[tokio::test]
async fn find_documents_by_name_hint_uses_partial_match() {
    let p = pool();
    let _doc = insert_doc(
        &p,
        "Arduino.txt",
        "/Users/Abir/notes/Arduino.txt",
        "void setup() {}",
    );
    let ids = search::find_documents_by_name_hint(&p, "open Arduino.txt please").expect("hint");
    assert!(!ids.is_empty(), "name hint must find Arduino.txt, got {:?}", ids);
}

#[tokio::test]
async fn find_documents_by_name_hint_does_not_match_unrelated() {
    let p = pool();
    let _doc = insert_doc(
        &p,
        "Arduino.txt",
        "/Users/Abir/notes/Arduino.txt",
        "void setup() {}",
    );
    let ids = search::find_documents_by_name_hint(&p, "tell me about bananas").expect("hint");
    assert!(ids.is_empty(), "must not find Arduino for an unrelated query, got {:?}", ids);
}

#[tokio::test]
async fn second_document_generalisation_also_works() {
    // The user explicitly asked for a regression test with
    // a *different* document and a *different* question, to
    // ensure the implementation generalises and is not
    // hardcoded for "Arduino.txt". We index a second file
    // and ask a question about it; the retrieval must
    // return the right document.
    let p = pool();
    let _arduino = insert_doc(
        &p,
        "Arduino.txt",
        "/Users/Abir/notes/Arduino.txt",
        "void setup() { pinMode(13, OUTPUT); }",
    );
    let _python = insert_doc(
        &p,
        "fibonacci.py",
        "/Users/Abir/code/fibonacci.py",
        "def fib(n):\n    if n < 2: return n\n    return fib(n-1) + fib(n-2)",
    );
    let res = search::search_hybrid(
        p.clone(),
        "Read the fibonacci.py file and tell me what it does",
        None,
        5,
    ).await.expect("hybrid");
    assert!(!res.hits.is_empty(), "second document must be retrievable, got {:?}", res.hits);
    assert!(res.hits[0].chunk.document_title.contains("fibonacci"), "first hit must be fibonacci.py, got {:?}", res.hits[0].chunk.document_title);
}
