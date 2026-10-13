//! Common test helpers. Phase 1 only needs the config round-trip.

#![allow(dead_code)]

pub fn tempdir() -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("nexus-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&base).unwrap();
    base
}
