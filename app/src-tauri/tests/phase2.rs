//! Phase 2 tests. These are pure unit tests that exercise the providers'
//! construction and the stream parser. They do not require a live backend.

mod common;

use common::tempdir;
use nexus_core::providers::stream::{FrameParser, RawFrame};
use nexus_core::providers::{ModelProvider, ProviderKind, ProviderSet};
use std::sync::Arc;

#[test]
fn frame_parser_sse_data() {
    let mut buf = String::new();
    let frames = FrameParser::push(&mut buf, &bytes::Bytes::from_static(b"data: {\"a\":1}\n\n"));
    assert_eq!(frames.len(), 1);
    match &frames[0] {
        RawFrame::SseData(s) => assert_eq!(s, "{\"a\":1}"),
        _ => panic!("expected SseData"),
    }
    assert!(buf.is_empty());
}

#[test]
fn frame_parser_sse_done() {
    let mut buf = String::new();
    let frames = FrameParser::push(&mut buf, &bytes::Bytes::from_static(b"data: [DONE]\n\n"));
    assert_eq!(frames.len(), 1);
    matches!(frames[0], RawFrame::SseDone);
}

#[test]
fn frame_parser_partial_buffer() {
    let mut buf = String::new();
    let _ = FrameParser::push(&mut buf, &bytes::Bytes::from_static(b"data: {\"x\""));
    assert_eq!(buf, "data: {\"x\"");
    let frames = FrameParser::push(&mut buf, &bytes::Bytes::from_static(b":1}\n\n"));
    assert_eq!(frames.len(), 1);
    match &frames[0] {
        RawFrame::SseData(s) => assert_eq!(s, "{\"x\":1}"),
        _ => panic!("expected SseData"),
    }
}

#[test]
fn frame_parser_ndjson() {
    let mut buf = String::new();
    let frames = FrameParser::push(&mut buf, &bytes::Bytes::from_static(b"{\"a\":1}\n{\"b\":2}\n"));
    assert_eq!(frames.len(), 2);
    matches!(&frames[0], RawFrame::NdjsonLine(_));
    matches!(&frames[1], RawFrame::NdjsonLine(_));
}

#[tokio::test]
async fn lmstudio_provider_health_unreachable() {
    // Point at a guaranteed-closed port on the loopback.
    let p = nexus_core::providers::lmstudio::LmStudioProvider::new("http://127.0.0.1:1/v1").unwrap();
    let h = p.health().await;
    assert!(!h.reachable);
    assert!(h.error.is_some());
}

#[tokio::test]
async fn ollama_provider_normalises_v1_suffix() {
    let p = nexus_core::providers::ollama::OllamaProvider::new("http://127.0.0.1:11434/v1").unwrap();
    assert_eq!(p.base_url(), "http://127.0.0.1:11434");
}

#[test]
fn provider_set_default_active() {
    let lms: Arc<dyn nexus_core::providers::ModelProvider> = Arc::new(
        nexus_core::providers::lmstudio::LmStudioProvider::new("http://127.0.0.1:1234/v1").unwrap(),
    );
    let lcp: Arc<dyn nexus_core::providers::ModelProvider> = Arc::new(
        nexus_core::providers::llamacpp::LlamaCppProvider::new("http://127.0.0.1:8081/v1").unwrap(),
    );
    let oll: Arc<dyn nexus_core::providers::ModelProvider> = Arc::new(
        nexus_core::providers::ollama::OllamaProvider::new("http://127.0.0.1:11434").unwrap(),
    );
    let set = ProviderSet::new(lms, lcp, oll, ProviderKind::LmStudio);
    assert_eq!(set.active_kind(), ProviderKind::LmStudio);
    set.set_active(ProviderKind::Ollama);
    assert_eq!(set.active_kind(), ProviderKind::Ollama);
}

#[test]
fn config_round_trip_unchanged() {
    let dir = tempdir();
    let cfg = nexus_core::config::AppConfig::load(&dir).unwrap();
    cfg.save(&dir).unwrap();
    let re = nexus_core::config::AppConfig::load(&dir).unwrap();
    assert_eq!(re.providers.lmstudio_base_url, cfg.providers.lmstudio_base_url);
}