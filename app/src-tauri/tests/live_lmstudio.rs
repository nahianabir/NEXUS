//! Live integration test against a running LM Studio. This test is **skipped**
//! unless the `NEXUS_LIVE_TEST=1` env var is set AND a model is loaded.
//!
//! It is intentionally not a `#[test]` so it does not run by default; a
//! separate `tests/live_lmstudio_smoke.rs` file in the same crate folder is
//! invoked explicitly. The script `D:\NEXUS\.scratch\live_test.ps1` runs it.

use std::time::Duration;

use nexus_core::providers::ModelProvider;

#[tokio::test]
async fn live_lmstudio_stream() {
    if std::env::var("NEXUS_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("[skipped] NEXUS_LIVE_TEST != 1");
        return;
    }
    let p = nexus_core::providers::lmstudio::LmStudioProvider::new("http://127.0.0.1:1234/v1")
        .expect("client");
    let h = p.health().await;
    assert!(h.reachable, "LM Studio must be reachable on 127.0.0.1:1234; got {:?}", h);
    let models = p.list_models().await.expect("list models");
    assert!(!models.is_empty(), "expected at least one model");
    let model = models[0].id.clone();
    eprintln!("[live] using model {model}");

    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let cancel = tokio_util::sync::CancellationToken::new();
    let req = nexus_core::providers::ChatRequest {
        model: model.clone(),
        messages: vec![nexus_core::providers::ChatMessage::text("user", "Say hi in one short sentence.")],
        system: Some("You are a concise assistant.".into()),
        temperature: Some(0.4),
        top_p: None,
        max_tokens: Some(60),
        stop: None,
        tools: None,
        tool_choice: None,
    };
    let provider = std::sync::Arc::new(p);
    let provider2 = provider.clone();
    let cancel2 = cancel.clone();
    let task = tokio::spawn(async move {
        provider2.stream_chat(req, tx, cancel2).await
    });
    let mut tokens = 0;
    let mut reasoning = 0;
    let mut got_done = false;
    let timeout = tokio::time::sleep(Duration::from_secs(120));
    tokio::pin!(timeout);
    loop {
        tokio::select! {
            _ = &mut timeout => { panic!("timeout"); }
            maybe = rx.recv() => {
                match maybe {
                    Some(ev) => match ev.kind.as_str() {
                        "token" => { tokens += ev.text.unwrap_or_default().len(); }
                        "reasoning" => { reasoning += ev.text.unwrap_or_default().len(); }
                        "done" => { got_done = true; break; }
                        "error" => { panic!("provider error: {:?}", ev.error); }
                        "cancelled" => { panic!("cancelled"); }
                        _ => {}
                    },
                    None => break,
                }
            }
        }
    }
    let summary = task.await.expect("task").expect("stream result");
    eprintln!("[live] tokens chars={tokens} reasoning chars={reasoning} got_done={got_done}");
    eprintln!("[live] content len={}, finish_reason={:?}", summary.content.len(), summary.finish_reason);
    assert!(got_done, "expected done event");
    // We accept the answer being short or empty when the model only emits
    // reasoning; the important property is that the stream completed and
    // produced a summary.
    assert!(summary.latency_ms < 120_000);
}
