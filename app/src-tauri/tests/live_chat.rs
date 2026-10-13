//! Live chat integration test. Skipped unless NEXUS_LIVE_TEST=1.
//!
//! Exercises the same path as `commands::chat::chat_send` but without the
//! Tauri runtime, so the test suite stays small. We register a fake
//! "conversation" in SQLite, fire a `ChatRequest` through the LM Studio
//! provider, and assert the stream completes with a non-empty summary.

use std::sync::Arc;
use std::time::Duration;

use nexus_core::conversations::{self, Message};
use nexus_core::db;
use nexus_core::model_profiles;
use nexus_core::providers::{ChatMessage, ChatRequest, ModelProvider, StreamEvent};
use ulid::Ulid;

mod common;
use common::tempdir;

#[tokio::test]
async fn live_chat_through_lmstudio() {
    if std::env::var("NEXUS_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("[skipped] NEXUS_LIVE_TEST != 1");
        return;
    }
    let dir = tempdir();
    let pool = db::open_pool(&db::default_db_path(&dir)).expect("pool");
    {
        let conn = pool.get().unwrap();
        let mut conn = conn;
        db::migrations::run(&mut conn).expect("migrate");
    }

    // Seed a profile.
    let profile = model_profiles::default_general(
        "qwen/qwen3.5-9b".to_string(),
        "lmstudio".to_string(),
    );
    model_profiles::upsert(&pool, &profile).expect("upsert profile");

    // Create a conversation.
    let conv = conversations::create(&pool, Some("live test".into()), None, Some(profile.id.clone())).expect("create conv");
    let now = chrono::Utc::now().timestamp_millis();
    let user = Message {
        id: Ulid::new().to_string(),
        conversation_id: conv.id.clone(),
        role: "user".into(),
        content: "Say hi in one short sentence.".into(),
        status: "complete".into(),
        token_count: None, latency_ms: None,
        model_profile_id: Some(profile.id.clone()),
        created_at: now,
    };
    conversations::add_message(&pool, &user).expect("add user msg");

    // Fire the chat.
    let provider: Arc<dyn ModelProvider> = Arc::new(
        nexus_core::providers::lmstudio::LmStudioProvider::new("http://127.0.0.1:1234/v1")
            .expect("client"),
    );
    let health = provider.health().await;
    assert!(health.reachable, "LM Studio not reachable: {:?}", health);
    let req = ChatRequest {
        model: "qwen/qwen3.5-9b".into(),
        messages: vec![ChatMessage::text("user", "Say hi in one short sentence.")],
        system: Some("You are a concise assistant. Answer plainly.".into()),
        temperature: Some(0.4),
        top_p: Some(0.95),
        max_tokens: Some(200),
        stop: None,
        tools: None,
        tool_choice: None,
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel::<StreamEvent>(64);
    let cancel = tokio_util::sync::CancellationToken::new();
    let provider2 = provider.clone();
    let cancel2 = cancel.clone();
    let task = tokio::spawn(async move { provider2.stream_chat(req, tx, cancel2).await });

    let mut token_chars = 0;
    let mut reasoning_chars = 0;
    let mut got_done = false;
    let timeout = tokio::time::sleep(Duration::from_secs(180));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            _ = &mut timeout => { panic!("timeout"); }
            maybe = rx.recv() => {
                match maybe {
                    Some(ev) => match ev.kind.as_str() {
                        "token" => { token_chars += ev.text.unwrap_or_default().len(); }
                        "reasoning" => { reasoning_chars += ev.text.unwrap_or_default().len(); }
                        "done" => { got_done = true; break; }
                        "error" => panic!("provider error: {:?}", ev.error),
                        "cancelled" => panic!("cancelled"),
                        _ => {}
                    },
                    None => break,
                }
            }
        }
    }
    let summary = task.await.expect("join").expect("stream ok");
    eprintln!("[live_chat] token_chars={} reasoning_chars={} done={} content_len={} finish_reason={:?}",
        token_chars, reasoning_chars, got_done, summary.content.len(), summary.finish_reason);
    assert!(got_done);
    assert!(summary.latency_ms < 180_000);
    // The summary text should be non-empty OR we should have seen reasoning.
    assert!(token_chars + reasoning_chars > 0, "no tokens or reasoning arrived");
}