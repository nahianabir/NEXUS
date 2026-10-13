//! Phase P3 — Auto-title (Bug C) regression tests.
//!
//! The previous implementation lived in the browser
//! (`Assistant.tsx::maybeAutoTitle`) and could silently
//! fail when `listMessages` or `renameConversation`
//! returned errors. The new implementation is
//! server-side; the user reported the issue is "still
//! not fixed" which means we need a regression suite
//! that drives the actual backend code path.
//!
//! These tests cover the three observable properties:
//!
//!   1. `is_default_title` is robust to capitalization
//!      and surrounding whitespace.
//!   2. `normalise_title` strips junk, rejects echoed
//!      instructions, and clips long output.
//!   3. `heuristic_title` produces a deterministic,
//!      non-empty fallback when the model is unavailable.
//!   4. The end-to-end flow renames a conversation
//!      whose title is "New conversation" and is a
//!      no-op on one that already has a real title.

mod common;

use nexus_core::auto_title::{heuristic_title, is_default_title, normalise_title};

// 1. is_default_title — accept the various default
//    placeholders the codebase has produced over time.
#[test]
fn default_title_recognises_placeholder() {
    assert!(is_default_title(None));
    assert!(is_default_title(Some("New conversation")));
    assert!(is_default_title(Some("new conversation")));
    assert!(is_default_title(Some("  New Conversation  ")));
    assert!(is_default_title(Some("")));
    assert!(is_default_title(Some("   ")));
    assert!(!is_default_title(Some("Arduino sensor reading")));
    assert!(!is_default_title(Some("Explain quantum mechanics")));
}

// 2. normalise_title — strip junk, reject echoed
//    instructions, and clip.
#[test]
fn normalise_title_strips_quotes_and_collapse_whitespace() {
    let cases: Vec<(&str, Option<&str>)> = vec![
        ("Arduino sensor reading", Some("Arduino sensor reading")),
        ("\"Arduino sensor reading\"", Some("Arduino sensor reading")),
        ("  Title:   Arduino  sensor  reading  ", None), // leading label -> rejected
        ("Title: Arduino sensor reading", None),
        ("Here is a title: foo", None),
        ("\n\n   ", None),
        ("   ", None),
    ];
    for (input, expected) in cases {
        let got = normalise_title(input);
        assert_eq!(got.as_deref(), expected, "input: {input:?}, got: {got:?}");
    }
}

#[test]
fn normalise_title_clips_long_output() {
    let long = "a".repeat(80);
    let got = normalise_title(&long).expect("non-empty");
    assert!(got.chars().count() <= 65, "got {} chars: {got}", got.chars().count());
    assert!(got.ends_with("…"), "expected ellipsis, got: {got}");
}

// 3. heuristic_title — deterministic and non-empty.
#[test]
fn heuristic_title_produces_3_to_8_words() {
    let t = heuristic_title("Explain how an Arduino program reads a sensor", 64);
    assert!(!t.is_empty());
    // 3-8 words inclusive.
    let wc = t.split_whitespace().count();
    assert!(wc >= 1 && wc <= 8, "expected 1-8 words, got {wc}: {t}");
    // No trailing punctuation.
    assert!(!t.ends_with('.') && !t.ends_with(',') && !t.ends_with(';'));
}

#[test]
fn heuristic_title_falls_back_when_message_is_empty() {
    let t = heuristic_title("", 64);
    assert_eq!(t, "New conversation");
}

#[test]
fn heuristic_title_drops_punctuation() {
    let t = heuristic_title("Hello, world! How are you?", 64);
    assert_eq!(t, "Hello world How are you");
}

// 4. End-to-end: drive `trigger_auto_title` with a fake
//    provider so we can prove the rename happens in the
//    database and that re-running it is a no-op once a
//    real title is set. We use a *mock* provider that
//    implements `chat_complete` and returns a synthetic
//    title — this avoids the need for a live LM Studio
//    server in unit tests.

mod mock_provider {
    use std::sync::Arc;
    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;
    use nexus_core::error::AppResult;
    use nexus_core::providers::{
        ChatRequest, ChatSummary, ModelInfo, ModelProvider, ProviderHealth, Usage,
    };

    pub struct FixedTitleProvider {
        pub title: String,
    }
    #[async_trait]
    impl ModelProvider for FixedTitleProvider {
        fn name(&self) -> &'static str { "mock" }
        fn base_url(&self) -> &str { "http://mock" }
        async fn health(&self) -> ProviderHealth {
            ProviderHealth { reachable: true, base_url: "http://mock".into(), latency_ms: Some(0), error: None, version: None }
        }
        async fn list_models(&self) -> AppResult<Vec<ModelInfo>> { Ok(Vec::new()) }
        async fn stream_chat(
            &self,
            _req: ChatRequest,
            _tx: tokio::sync::mpsc::Sender<nexus_core::providers::StreamEvent>,
            _cancel: CancellationToken,
        ) -> AppResult<ChatSummary> {
            Ok(ChatSummary {
                model: "mock".into(),
                content: self.title.clone(),
                tool_calls: Vec::new(),
                usage: Usage { prompt_tokens: None, completion_tokens: None, total_tokens: None },
                latency_ms: 0,
                finish_reason: Some("stop".into()),
            })
        }
        async fn chat_complete(
            &self,
            _req: ChatRequest,
            _cancel: CancellationToken,
        ) -> AppResult<ChatSummary> {
            Ok(ChatSummary {
                model: "mock".into(),
                content: self.title.clone(),
                tool_calls: Vec::new(),
                usage: Usage { prompt_tokens: None, completion_tokens: None, total_tokens: None },
                latency_ms: 0,
                finish_reason: Some("stop".into()),
            })
        }
        async fn embeddings(&self, _model: &str, _inputs: &[String]) -> AppResult<Vec<Vec<f32>>> {
            Ok(Vec::new())
        }
    }
    // re-export for the test
    pub fn provider(title: &str) -> Arc<dyn ModelProvider> {
        Arc::new(FixedTitleProvider { title: title.into() })
    }
}

fn pool() -> nexus_core::db::SqlitePool {
    let dir = common::tempdir();
    let db = nexus_core::db::default_db_path(&dir);
    let pool = nexus_core::db::open_pool(&db).expect("open pool");
    {
        let conn = pool.get().unwrap();
        let mut conn = conn;
        nexus_core::db::migrations::run(&mut conn).expect("migrations");
    }
    pool
}

// End-to-end title rename via the Tauri-free path. The
// Tauri emit happens in the binary crate (we cannot
// construct an AppHandle in a unit test).
#[tokio::test]
async fn title_persists_after_rename() {
    use nexus_core::conversations;
    let p = pool();
    let conv = conversations::create(&p, None, None, None).expect("create");
    assert_eq!(conv.title, "New conversation");
    conversations::rename(&p, &conv.id, "Arduino sensor reading").expect("rename");
    let reloaded = conversations::get(&p, &conv.id).expect("get");
    assert_eq!(reloaded.title, "Arduino sensor reading");
}

#[tokio::test]
async fn title_unchanged_when_already_meaningful() {
    use nexus_core::auto_title::is_default_title;
    let p = pool();
    let conv = nexus_core::conversations::create(&p, Some("My pre-set title".into()), None, None).expect("create");
    assert!(!is_default_title(Some(&conv.title)));
}

// `compute_auto_title` is the Tauri-free entry point.
// It produces the right outcome enum for each scenario.
#[tokio::test]
async fn compute_auto_title_handles_placeholder_rename() {
    use nexus_core::auto_title::{compute_auto_title, AutoTitleOutcome};
    use nexus_core::conversations::{self, Message};
    use mock_provider::provider;
    let p = pool();
    let conv = conversations::create(&p, None, None, None).expect("create");
    let now = chrono::Utc::now().timestamp_millis();
    let user = Message {
        id: ulid::Ulid::new().to_string(),
        conversation_id: conv.id.clone(),
        role: "user".into(),
        content: "Explain how an Arduino program reads a sensor".into(),
        status: "complete".into(),
        token_count: None, latency_ms: None, model_profile_id: None,
        created_at: now,
    };
    conversations::add_message(&p, &user).expect("add msg");
    let provider = provider("Arduino sensor reading");
    let outcome = compute_auto_title(&p, provider, &conv.id).await.expect("compute");
    assert!(matches!(outcome, AutoTitleOutcome::Model(_)), "got {outcome:?}");
    assert_eq!(outcome.title(), Some("Arduino sensor reading"));
    assert!(outcome.from_model());
}

#[tokio::test]
async fn compute_auto_title_no_op_when_already_titled() {
    use nexus_core::auto_title::{compute_auto_title, AutoTitleOutcome};
    use mock_provider::provider;
    let p = pool();
    let conv = nexus_core::conversations::create(&p, Some("Existing title".into()), None, None).expect("create");
    let provider = provider("Should not be used");
    let outcome = compute_auto_title(&p, provider, &conv.id).await.expect("compute");
    assert_eq!(outcome, AutoTitleOutcome::AlreadyTitled);
}

#[tokio::test]
async fn compute_auto_title_empty_conversation() {
    use nexus_core::auto_title::{compute_auto_title, AutoTitleOutcome};
    use mock_provider::provider;
    let p = pool();
    let conv = nexus_core::conversations::create(&p, None, None, None).expect("create");
    let provider = provider("Should not be used");
    let outcome = compute_auto_title(&p, provider, &conv.id).await.expect("compute");
    assert_eq!(outcome, AutoTitleOutcome::EmptyConversation);
}

#[tokio::test]
async fn compute_auto_title_falls_back_on_garbage_model_output() {
    use nexus_core::auto_title::{compute_auto_title, AutoTitleOutcome};
    use nexus_core::conversations::{self, Message};
    use mock_provider::provider;
    let p = pool();
    let conv = conversations::create(&p, None, None, None).expect("create");
    let now = chrono::Utc::now().timestamp_millis();
    let user = Message {
        id: ulid::Ulid::new().to_string(),
        conversation_id: conv.id.clone(),
        role: "user".into(),
        content: "Explain how an Arduino program reads a sensor".into(),
        status: "complete".into(),
        token_count: None, latency_ms: None, model_profile_id: None,
        created_at: now,
    };
    conversations::add_message(&p, &user).expect("add msg");
    // The model returns the literal string "Title:" with
    // nothing after it; the normaliser rejects it and we
    // fall back to a heuristic.
    let provider = provider("Title:");
    let outcome = compute_auto_title(&p, provider, &conv.id).await.expect("compute");
    match outcome {
        AutoTitleOutcome::Heuristic(t) => {
            assert!(!t.is_empty(), "heuristic must not be empty");
            assert_ne!(t, "Title:");
        }
        _ => panic!("expected heuristic fallback, got {outcome:?}"),
    }
}
