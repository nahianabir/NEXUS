//! Phase 4 tests: hash dedup, chunker, FTS5 search, hybrid retrieval.

mod common;

use std::fs;
use std::io::Write;

use nexus_core::db;
use nexus_core::ingestion::chunker::chunk_text;
use nexus_core::ingestion::extractors;
use nexus_core::ingestion::hash;
use nexus_core::ingestion::pipeline as ing;
use nexus_core::ingestion::search;
use ulid::Ulid;

use common::tempdir;

fn pool(dir: &std::path::Path) -> nexus_core::db::SqlitePool {
    let pool = db::open_pool(&db::default_db_path(dir)).expect("pool");
    let conn = pool.get().unwrap();
    let mut conn = conn;
    db::migrations::run(&mut conn).expect("migrate");
    pool
}

#[test]
fn hash_is_stable() {
    let dir = tempdir();
    let p = dir.join("a.txt");
    let mut f = fs::File::create(&p).unwrap();
    f.write_all(b"hello world").unwrap();
    let (h1, _) = hash::hash_file(&p).unwrap();
    let (h2, _) = hash::hash_file(&p).unwrap();
    assert_eq!(h1, h2);
    assert_eq!(h1.len(), 64);
}

#[test]
fn extractor_handles_text_md_json_csv_code_ipynb() {
    let dir = tempdir();
    let cases: &[(&str, &str, &str)] = &[
        ("a.txt", "hello world", "text/plain"),
        ("a.md", "# Title\n\nbody", "text/plain"),
        ("a.json", "{\"x\":1,\"y\":\"z\"}", "application/json"),
        ("a.csv", "a,b,c\n1,2,3\n4,5,6\n", "text/csv"),
        ("a.py", "def f():\n    return 1\n", "text/x-code"),
        ("a.ipynb", "{\"cells\":[{\"cell_type\":\"code\",\"source\":[\"print(1)\"]}],\"metadata\":{\"kernelspec\":{\"language\":\"python\"}}}", "application/x-ipynb"),
    ];
    for (name, content, expected_mime) in cases {
        let p = dir.join(name);
        fs::write(&p, content).unwrap();
        let e = extractors::extract(&p).unwrap();
        let expected_substr = match *name {
            "a.json" => "z",
            "a.ipynb" => "print",
            "a.csv" => "1\t2\t3",
            "a.txt" => "hello",
            "a.md" => "body",
            "a.py" => "return",
            _ => "",
        };
        assert!(!expected_substr.is_empty() && e.text.contains(expected_substr),
            "extract {name} produced: {}", e.text);
        assert_eq!(e.mime_type, *expected_mime, "mime for {name}");
    }
}

#[test]
fn chunker_splits_long_text() {
    let para = "alpha beta gamma delta. ";
    // Build 50 paragraphs separated by blank lines so the chunker has to
    // pack them across multiple chunks.
    let text = std::iter::repeat(para).take(50).collect::<Vec<_>>().join("\n\n");
    let chunks = chunk_text(&text, 200, 30);
    assert!(chunks.len() > 1, "expected multiple chunks, got {}", chunks.len());
    assert!(chunks[0].text.len() <= 1000);
}

#[test]
fn fts_search_finds_known_phrase() {
    let dir = tempdir();
    let pool = pool(&dir);
    // Insert a document + a chunk directly.
    let doc_id = Ulid::new().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO documents (id, source_id, title, path, size_bytes, content_hash, status, created_at, updated_at)
             VALUES (?1, NULL, 'doc', 'doc.md', 0, 'h', 'indexed', ?2, ?2)",
            rusqlite::params![doc_id, now],
        ).unwrap();
        let chunk_id = Ulid::new().to_string();
        conn.execute(
            "INSERT INTO document_chunks (id, document_id, ordinal, text, token_estimate, start_offset, end_offset)
             VALUES (?1, ?2, 0, 'Nebuchadnezzar built the hanging gardens of Babylon', 8, 0, 50)",
            rusqlite::params![chunk_id, doc_id],
        ).unwrap();
    }
    let hits = search::search_fts(&pool, "hanging gardens", 10).unwrap();
    assert!(!hits.is_empty(), "expected FTS5 hit, got {hits:?}");
    let first_id = &hits[0].0;
    let chunk = ing::get_chunk(&pool, first_id).unwrap().unwrap();
    assert!(chunk.text.contains("hanging"));
}

#[test]
fn ingest_skips_duplicate_by_hash() {
    let dir = tempdir();
    let p = dir.join("note.txt");
    fs::write(&p, "duplicate content").unwrap();
    let pool = pool(&dir);

    let req = ing::IngestSourceRequest { kind: "file".into(), path: p.display().to_string(), display_name: None };
    let cancel = tokio_util::sync::CancellationToken::new();
    let s1 = futures::executor::block_on(ing::ingest_source(pool.clone(), req.clone(), 200, 30, 1024 * 64, cancel.clone())).unwrap();
    let s2 = futures::executor::block_on(ing::ingest_source(pool.clone(), req, 200, 30, 1024 * 64, cancel)).unwrap();
    assert_eq!(s1.files_indexed, 1);
    assert_eq!(s2.files_indexed, 0);
    assert_eq!(s2.files_skipped, 1);
}