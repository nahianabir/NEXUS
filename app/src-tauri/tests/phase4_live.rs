//! Live integration test for the Knowledge Vault ingest pipeline.
//!
//! Skipped unless `NEXUS_LIVE_TEST=1`. We create a small directory tree with
//! real text and PDF fixtures, kick off the orchestrator, then assert that
//! FTS5 retrieval returns a known phrase.
//!
//! This does NOT exercise LM Studio or embeddings — those are gated behind
//! the user's explicit "backfill embeddings" action and require a real
//! embedding model profile. The pipeline tests run with FTS5 alone.

use std::time::Duration;

use nexus_core::conversations;
use nexus_core::db;
use nexus_core::ingestion::orchestrator::{self, IngestOptions};
use nexus_core::ingestion::pipeline as pipe;
use nexus_core::ingestion::search;

mod common;
use common::tempdir;

#[tokio::test]
async fn live_vault_ingest_and_search() {
    if std::env::var("NEXUS_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("[skipped] NEXUS_LIVE_TEST != 1");
        return;
    }

    let dir = tempdir();
    let fixture = dir.join("fixtures");
    std::fs::create_dir_all(&fixture).unwrap();
    let doc_a = fixture.join("alpha.md");
    let doc_b = fixture.join("beta.md");
    let doc_c = fixture.join("readme.txt");
    std::fs::write(
        &doc_a,
        "# Alpha\n\nThe tau neutrino is a hypothetical particle that has never been observed.\n\nIt is theorised to interact only via the weak force and gravity.\n",
    ).unwrap();
    std::fs::write(
        &doc_b,
        "# Beta\n\nBananas are a healthy source of potassium and natural sugars.\n\nThey are grown in tropical regions across the world.\n",
    ).unwrap();
    std::fs::write(
        &doc_c,
        "Readme\n\nNEXUS is a local-first AI workbench.\nIt uses sqlite-vec for vector search and FTS5 for keyword search.\n",
    ).unwrap();

    let pool = db::open_pool(&db::default_db_path(&dir)).expect("pool");
    {
        let conn = pool.get().unwrap();
        let mut conn = conn;
        db::migrations::run(&mut conn).expect("migrate");
    }

    let request = pipe::IngestSourceRequest {
        kind: "directory".into(),
        path: fixture.display().to_string(),
        display_name: Some("fixtures".into()),
    };
    let options = IngestOptions {
        chunk_tokens: 200,
        chunk_overlap: 30,
        max_file_bytes: 64 * 1024 * 1024,
        embedding_profile_id: None,
    };
    let (_job_id, mut rx, handle) = orchestrator::spawn(pool.clone(), request, options, None);

    // Drain the progress channel.
    let mut last_progress = String::new();
    loop {
        match tokio::time::timeout(Duration::from_secs(20), rx.recv()).await {
            Ok(Some(p)) => {
                last_progress = format!("{} {} ({} {} chunks)", last_progress, p.stage, p.files_done, p.chunks);
                if p.stage == "done" || p.stage == "done-with-errors" { break; }
            }
            _ => break,
        }
    }
    let result = handle.await.expect("drop").expect("ingest ok");
    eprintln!("[live_vault] ingest: files_seen={} files_indexed={} chunks={} embedded={}",
        result.stats.files_seen, result.stats.files_indexed, result.stats.chunks, result.embedded_chunks);
    assert!(result.stats.files_indexed >= 3, "expected 3+ indexed files");
    assert!(result.stats.chunks >= 3, "expected 3+ chunks");

    let fts_hits = search::search_fts(&pool, "tau neutrino", 5).expect("fts");
    eprintln!("[live_vault] fts hits for 'tau neutrino': {}", fts_hits.len());
    assert!(!fts_hits.is_empty(), "expected fts hits for 'tau neutrino'");
    let hybrid = search::search_hybrid(pool.clone(), "potassium bananas", None, 5).await.expect("hybrid");
    eprintln!("[live_vault] hybrid hits for 'potassium bananas': {}", hybrid.hits.len());
    assert!(!hybrid.hits.is_empty(), "expected hybrid hits for 'potassium bananas'");

    let empty_hits = search::search_hybrid(pool.clone(), "zzznotawordzzz", None, 5).await.expect("hybrid empty");
    assert!(empty_hits.hits.is_empty(), "expected zero hits for nonsense query");

    // Make sure the orchestrator-based test exercises conversations too
    // (so we know the basic chat DB stays alive alongside the vault).
    let conv = conversations::create(&pool, Some("vault test".into()), None, None).unwrap();
    assert!(!conv.id.is_empty());

    eprintln!("[live_vault] OK");
}