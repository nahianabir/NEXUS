//! Phase 1 tests: config round-trip and basic path validation.

use nexus_core::config::AppConfig;

#[test]
fn config_round_trip() {
    let dir = tempdir();
    let cfg = AppConfig::load(&dir).expect("load default");
    assert_eq!(cfg.general.theme, "dark");
    assert!(!cfg.general.allow_network);
    assert_eq!(cfg.providers.lmstudio_base_url, "http://127.0.0.1:1234/v1");
    assert_eq!(cfg.ingestion.chunk_tokens, 500);
    assert_eq!(cfg.agent.default_permission, "readonly");

    // mutate + save + reload
    let mut cfg = cfg;
    cfg.general.allow_network = true;
    cfg.ingestion.chunk_tokens = 768;
    cfg.save(&dir).unwrap();

    let reloaded = AppConfig::load(&dir).unwrap();
    assert!(reloaded.general.allow_network);
    assert_eq!(reloaded.ingestion.chunk_tokens, 768);
}

#[test]
fn config_persists_data_dir() {
    let dir = tempdir();
    let cfg = AppConfig::load(&dir).unwrap();
    assert_eq!(cfg.general.data_dir, std::path::PathBuf::from(&dir));
}

fn tempdir() -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("nexus-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    base
}
