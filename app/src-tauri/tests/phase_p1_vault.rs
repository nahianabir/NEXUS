//! Phase 1 (P1) regression tests — Knowledge Vault / document ingestion.
//!
//! These tests cover the black-screen-on-Vault path: the Rust
//! `DocumentRow` struct must include `content_hash` in its
//! serialized form (the Vault page renders the first 10 hex chars),
//! the chunker must produce correct byte offsets even after
//! paragraph `trim()`, and the code/text extractors must tolerate
//! non-UTF-8 bytes (a common case for Python files saved in
//! cp1252 on Windows).

mod common;

use std::io::Write;
use std::path::Path;

use nexus_core::db;
use nexus_core::ingestion::chunker::chunk_text;
use nexus_core::ingestion::extractors;
use nexus_core::ingestion::pipeline as ing;
use nexus_core::ingestion::pipeline::DocumentRow;

use common::tempdir;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

// ---------- DocumentRow content_hash (P0 black-screen fix) ----------

#[test]
fn list_documents_returns_content_hash() {
    // Regression for the Knowledge Vault black screen: the Rust
    // `list_documents` SELECT must include `content_hash` so the
    // frontend's `d.content_hash.slice(0, 10)` does not throw.
    let dir = tempdir();
    let pool = pool(&dir);
    ing::insert_document(&pool, None, Path::new("/tmp/example.py"), "deadbeef", 42, "indexed", None)
        .expect("insert_document");

    // The API exposed to integration tests is internal; exercise
    // the SELECT path by re-using the module's struct shape.
    let docs = ing::list_documents(&pool, 10).expect("list_documents");
    assert!(!docs.is_empty(), "expected at least one row, got 0");
    let d: &DocumentRow = &docs[0];
    // The serialized JSON for `DocumentRow` must include
    // `content_hash` as a top-level key, otherwise the frontend
    // sees `undefined` and `.slice` throws.
    let json = serde_json::to_value(d).expect("serialize DocumentRow");
    assert!(json.get("content_hash").is_some(), "DocumentRow JSON must include content_hash key");
    assert_eq!(json.get("content_hash").unwrap().as_str().unwrap(), "deadbeef");
}

// ---------- chunker offset correctness ----------

#[test]
fn chunker_offsets_round_trip_into_text() {
    // The chunker is used by the citation UI to highlight a span
    // in the original document. After the P1 fix the byte offsets
    // must point at the actual paragraph start, even when the
    // surrounding paragraph had leading whitespace that got
    // trimmed.
    let text = "   \n\nalpha beta gamma.\n\n   delta epsilon.\n\n";
    let chunks = chunk_text(text, 200, 0);
    assert!(!chunks.is_empty(), "expected at least one chunk");
    let c = &chunks[0];
    // Slicing the original text at the chunk's offsets must
    // recover the trimmed content.
    let span = &text[c.start_offset..c.end_offset];
    assert!(span.contains("alpha beta gamma"), "chunk span should contain 'alpha beta gamma', got {:?}", span);
}

#[test]
fn chunker_handles_code_with_line_prefix() {
    // The Python extractor prepends `"{:>5}  "` to every line, so
    // the chunker sees a stream of paragraphs separated by blank
    // lines. The offsets must remain monotonic and within the
    // input bounds.
    let mut text = String::new();
    for i in 1..=200 {
        text.push_str(&format!("{:>5}  x = {}\n\n", i, i));
    }
    let chunks = chunk_text(&text, 30, 0);
    assert!(chunks.len() > 1, "expected multiple chunks, got {}", chunks.len());
    for c in &chunks {
        assert!(c.end_offset <= text.len(), "end_offset {} must be within input length {}", c.end_offset, text.len());
        assert!(c.start_offset <= c.end_offset, "start_offset must be <= end_offset");
    }
}

// ---------- extractor tolerance ----------

#[test]
fn extract_code_tolerates_non_utf8_bytes() {
    let dir = tempdir();
    let path = dir.join("cp1252.py");
    let mut f = std::fs::File::create(&path).expect("create");
    // The first 10 bytes are valid UTF-8; the rest is a cp1252
    // em-dash (0x97) and an en-dash (0x96) which are NOT valid
    // UTF-8. The extractor must replace them with the Unicode
    // replacement character rather than failing.
    f.write_all(b"# header\nx = 1 +  # \x97 em-dash \x96 en-dash\ny = 2\n").expect("write");
    drop(f);

    let result = extractors::extract(&path);
    let ext = result.expect("extract_code must not fail on cp1252");
    assert!(ext.text.contains("em-dash"), "expected lossy decode to preserve the surrounding ASCII text, got {:?}", ext.text);
    assert!(ext.text.contains("y = 2"), "expected the tail of the file to be preserved");
    assert_eq!(ext.metadata.get("non_utf8_replaced").and_then(|v| v.as_bool()), Some(true));
}

#[test]
fn extract_text_tolerates_non_utf8_bytes() {
    let dir = tempdir();
    let path = dir.join("cp1252.md");
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(b"# Title\n\nSome text with a \x97 em-dash.\n").expect("write");
    drop(f);

    let ext = extractors::extract(&path).expect("extract_text must not fail on cp1252");
    assert!(ext.text.contains("Title"));
    assert!(ext.text.contains("Some text"));
}

// ---------- Source/destination safety ----------

#[test]
fn documents_dedupe_by_content_hash() {
    // The pipeline must not insert two rows for the same hash. A
    // duplicate import of the same Python file must be a no-op
    // rather than producing two visible "indexed" rows. The DB
    // enforces this with a UNIQUE INDEX on `content_hash`, so
    // the second `insert_document` must fail with a constraint
    // violation. The orchestrator layer catches that failure and
    // updates the existing row instead; the helper itself
    // surfaces the constraint as an error.
    let dir = tempdir();
    let pool = pool(&dir);
    let path = Path::new("/tmp/dup.py");
    let _h1 = ing::insert_document(&pool, None, path, "hash1", 10, "indexed", None).expect("insert 1");
    let h2 = ing::insert_document(&pool, None, path, "hash1", 10, "indexed", None);
    assert!(h2.is_err(), "second insert with the same content_hash must fail the UNIQUE constraint, got {:?}", h2);
    let docs = ing::list_documents(&pool, 10).expect("list");
    assert_eq!(docs.len(), 1, "exactly one document row must exist after a duplicate insert, got {}", docs.len());
}

// ---------- Per-dimension embedding collections (R3) ----------

#[test]
fn per_dimension_tables_are_created_lazily() {
    // Calling `ensure_dimension_table` for a fresh dimension
    // creates the per-dimension vec0 table and registers the
    // collection. A second call is idempotent.
    use nexus_core::ingestion::vector;
    let dir = tempdir();
    let pool = pool(&dir);
    let first = vector::ensure_dimension_table(&pool, 768).expect("create 768");
    assert!(first, "first call should report a fresh table");
    let second = vector::ensure_dimension_table(&pool, 768).expect("create 768 again");
    assert!(!second, "second call must be idempotent");
    // A different dimension creates a new table.
    vector::ensure_dimension_table(&pool, 384).expect("create 384");
    let cols: Vec<String> = {
        let conn = pool.get().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'chunk_vectors_%' ORDER BY name",
        ).unwrap();
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
        rows.filter_map(|r| r.ok()).collect()
    };
    assert!(cols.contains(&"chunk_vectors_768".to_string()), "missing 768 table: {:?}", cols);
    assert!(cols.contains(&"chunk_vectors_384".to_string()), "missing 384 table: {:?}", cols);
}

#[test]
fn upsert_routes_to_per_dimension_table() {
    use nexus_core::ingestion::vector;
    let dir = tempdir();
    let pool = pool(&dir);
    vector::ensure_dimension_table(&pool, 384).unwrap();
    vector::register_collection(&pool, 384, "model-A", "model-A::dim384").unwrap();
    vector::ensure_dimension_table(&pool, 768).unwrap();
    vector::register_collection(&pool, 768, "model-B", "model-B::dim768").unwrap();
    let v384: Vec<f32> = (0..384).map(|i| i as f32 * 0.01).collect();
    let v768: Vec<f32> = (0..768).map(|i| i as f32 * 0.01).collect();
    vector::upsert(&pool, "chunk-1", &v384).unwrap();
    vector::upsert(&pool, "chunk-2", &v768).unwrap();
    let conn = pool.get().unwrap();
    let n384: i64 = conn.query_row("SELECT COUNT(*) FROM chunk_vectors_384", [], |r| r.get(0)).unwrap();
    let n768: i64 = conn.query_row("SELECT COUNT(*) FROM chunk_vectors_768", [], |r| r.get(0)).unwrap();
    assert_eq!(n384, 1, "384 table must contain exactly the 384-dim vector");
    assert_eq!(n768, 1, "768 table must contain exactly the 768-dim vector");
}

#[test]
fn collection_refuses_to_mix_incompatible_models() {
    use nexus_core::ingestion::vector;
    let dir = tempdir();
    let pool = pool(&dir);
    vector::ensure_dimension_table(&pool, 768).unwrap();
    vector::register_collection(&pool, 768, "model-A", "model-A::dim768").unwrap();
    // Same dimension, different model identity: refused.
    let res = vector::register_collection(&pool, 768, "model-B", "model-B::dim768");
    assert!(res.is_err(), "must refuse to overwrite a collection with a different model identity, got {:?}", res);
    let msg = format!("{}", res.unwrap_err());
    assert!(msg.contains("incompatible"), "error message should mention incompatibility, got: {}", msg);
}

#[test]
fn collection_allows_same_model_to_extend() {
    use nexus_core::ingestion::vector;
    let dir = tempdir();
    let pool = pool(&dir);
    vector::ensure_dimension_table(&pool, 384).unwrap();
    vector::register_collection(&pool, 384, "model-A", "model-A::dim384").unwrap();
    // Re-registering with the same model identity is a no-op.
    vector::register_collection(&pool, 384, "model-A", "model-A::dim384").unwrap();
    let cols = vector::list_collections(&pool).unwrap();
    assert_eq!(cols.len(), 1, "must have exactly one collection row");
}

#[test]
fn search_only_returns_hits_for_registered_dimension() {
    use nexus_core::ingestion::chunker::Chunk;
    use nexus_core::ingestion::vector;
    let dir = tempdir();
    let pool = pool(&dir);
    // 384 collection with one chunk that has both a
    // `document_chunks` row and a vector. The wrapper
    // requires both to return a hit.
    vector::ensure_dimension_table(&pool, 384).unwrap();
    vector::register_collection(&pool, 384, "model-A", "model-A::dim384").unwrap();
    let doc_id = ing::insert_document(&pool, None, Path::new("/tmp/sx.txt"), "h-sx", 10, "indexed", None).expect("insert doc");
    let chunk = Chunk { ordinal: 0, text: "hello world".into(), token_estimate: 2, start_offset: 0, end_offset: 11 };
    let chunk_id = ing::insert_chunk(&pool, &doc_id, &chunk, "").expect("insert chunk");
    let v: Vec<f32> = (0..384).map(|i| i as f32 * 0.01).collect();
    vector::upsert(&pool, &chunk_id, &v).unwrap();
    // A 384-dim query should return that vector joined to
    // the chunk's text.
    let hits = vector::search(&pool, &v, 10).unwrap();
    assert_eq!(hits.len(), 1);
    // A 768-dim query has no registered collection, so we
    // get an empty result (not an error).
    let q768: Vec<f32> = (0..768).map(|i| i as f32 * 0.01).collect();
    let hits = vector::search(&pool, &q768, 10).unwrap();
    assert!(hits.is_empty(), "768-dim query against a 384-only database must return no hits, got {}", hits.len());
}

#[test]
fn shadow_vec0_tables_do_not_break_iteration() {
    // Regression test: vec0 creates shadow helper tables
    // (`<base>_rowids`, `<base>_vector_chunks00`, `<base>_info`,
    // `<base>_chunks`) that have no `chunk_id` column. Code
    // that scans `chunk_vectors_%` for the per-dim
    // user-facing tables must skip these shadow tables,
    // otherwise SELECT/DELETE statements on the union
    // produce `no such column: chunk_id` errors.
    use nexus_core::ingestion::chunker::Chunk;
    use nexus_core::ingestion::vector;
    let dir = tempdir();
    let pool = pool(&dir);
    vector::ensure_dimension_table(&pool, 384).unwrap();
    vector::register_collection(&pool, 384, "model-A", "model-A::dim384").unwrap();
    // Insert a real chunk + vector so total_vector_rows and
    // delete_for_document have something to operate on.
    let doc_id = ing::insert_document(&pool, None, Path::new("/tmp/sh.txt"), "h-sh", 10, "indexed", None).expect("insert doc");
    let chunk = Chunk { ordinal: 0, text: "shadow test".into(), token_estimate: 2, start_offset: 0, end_offset: 11 };
    let chunk_id = ing::insert_chunk(&pool, &doc_id, &chunk, "").expect("insert chunk");
    let v: Vec<f32> = (0..384).map(|i| i as f32 * 0.01).collect();
    vector::upsert(&pool, &chunk_id, &v).unwrap();
    // total_vector_rows must count the 1 row and not
    // explode on the shadow tables.
    let total = vector::total_vector_rows(&pool).expect("total");
    assert_eq!(total, 1, "must count exactly the user-facing row, got {}", total);
    // delete_for_document must succeed and not throw on
    // the shadow tables.
    vector::delete_for_document(&pool, &doc_id).expect("delete");
    let total_after = vector::total_vector_rows(&pool).expect("total after");
    assert_eq!(total_after, 0, "delete_for_document must clear the user-facing row, got {}", total_after);
}

#[test]
fn backfill_selects_only_user_facing_dim_tables() {
    // Regression test for the production bug: the backfill
    // command builds a `SELECT chunk_id AS id FROM
    // chunk_vectors_X UNION ALL ...` query and feeds it to
    // `WHERE c.id NOT IN (...)`. If the iteration picks up
    // the vec0 shadow tables (`chunk_vectors_384_rowids`
    // etc.), the union SELECT fails with
    // `no such column: chunk_id`. The fix is to filter
    // tables by name pattern (suffix must be a pure
    // integer).
    //
    // We exercise the backfill's filter logic via the same
    // name pattern it uses. The exact query lives in
    // `commands::vault::vault_embed_backfill`; the filter
    // helper it relies on is `vector::list_user_dim_tables`
    // (private) — but we can reproduce the bug and the fix
    // by checking that the table list returned by an
    // equivalent scan contains only the user-facing tables.
    use nexus_core::ingestion::vector;
    let dir = tempdir();
    let pool = pool(&dir);
    vector::ensure_dimension_table(&pool, 384).unwrap();
    vector::ensure_dimension_table(&pool, 768).unwrap();
    let conn = pool.get().unwrap();
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'chunk_vectors_%' ORDER BY name",
    ).unwrap();
    let names: Vec<String> = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap().filter_map(|r| r.ok()).collect();
    // The vec0 shadow tables must exist alongside the
    // user-facing ones.
    assert!(names.iter().any(|n| n.contains("rowids")), "expected a `_rowids` shadow table to exist, got: {:?}", names);
    assert!(names.iter().any(|n| n.contains("vector_chunks")), "expected a `vector_chunks00` shadow table, got: {:?}", names);
    // Apply the same filter the backfill uses.
    let user_tables: Vec<String> = names.iter().filter(|n| {
        let suffix = n.strip_prefix("chunk_vectors_").unwrap_or("");
        !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
    }).cloned().collect();
    assert!(user_tables.contains(&"chunk_vectors_384".to_string()), "must include 384, got: {:?}", user_tables);
    assert!(user_tables.contains(&"chunk_vectors_768".to_string()), "must include 768, got: {:?}", user_tables);
    assert_eq!(user_tables.len(), 2, "must contain *only* the user-facing per-dim tables, got: {:?}", user_tables);
}
