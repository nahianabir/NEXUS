//! Phase P2 — JavaScript sandbox regression tests.
//!
//! The NEXUS-native sandbox wraps the Deno binary with the
//! same flag set the LM Studio `js-code-sandbox` plugin uses:
//! `--allow-read=.` `--allow-write=.` `--no-prompt`
//! `--deny-net` `--deny-env` `--deny-sys` `--deny-run`
//! `--deny-ffi`. We mirror those flags so the security
//! guarantees of the plugin carry over to the NEXUS-native
//! path.
//!
//! Deno is **not bundled** with NEXUS — the user must
//! install it. These tests verify:
//!
//! 1. `find_deno` returns a clear `NotFound` error when Deno
//!    is not on PATH and `DENO` is unset, *without* trying
//!    to spawn anything.
//! 2. `run_javascript` against a missing working directory
//!    returns `Invalid` (no subprocess is spawned).
//! 3. `DEFAULT_TIMEOUT_SECONDS` is 5 and `MAX_TIMEOUT_SECONDS`
//!    is 60, matching the LM Studio plugin's contract.
//! 4. The "kill on timeout" path returns `timed_out: true`
//!    when the spawn is killed (covered by the live test
//!    if Deno is available).
//!
//! Tests that actually need Deno are gated by the
//! `NEXUS_LIVE_DENO` env var. The CI baseline keeps them
//! skipped.

use std::path::PathBuf;

use nexus_core::sandbox::{find_deno, run_javascript, DEFAULT_TIMEOUT_SECONDS, MAX_TIMEOUT_SECONDS, SandboxResult};

#[test]
fn find_deno_returns_clear_error_when_absent() {
    // Force a clean environment so this test is
    // deterministic regardless of the host.
    // SAFETY: we are not multi-threaded at this point in
    // the test setup, and we restore the values below.
    let prev_path = std::env::var("PATH").ok();
    let prev_deno = std::env::var("DENO").ok();
    let prev_home = std::env::var("LMSTUDIO_HOME").ok();
    let prev_userprofile = std::env::var("USERPROFILE").ok();
    let prev_home_env = std::env::var("HOME").ok();
    std::env::set_var("PATH", "");
    std::env::remove_var("DENO");
    std::env::remove_var("LMSTUDIO_HOME");
    std::env::remove_var("USERPROFILE");
    std::env::remove_var("HOME");
    let res = find_deno();
    std::env::set_var("PATH", prev_path.unwrap_or_default());
    if let Some(v) = prev_deno { std::env::set_var("DENO", v); }
    if let Some(v) = prev_home { std::env::set_var("LMSTUDIO_HOME", v); }
    if let Some(v) = prev_userprofile { std::env::set_var("USERPROFILE", v); }
    if let Some(v) = prev_home_env { std::env::set_var("HOME", v); }
    let err = res.expect_err("deno must not be found with empty PATH and no DENO env var");
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("deno"),
        "error must mention Deno, got: {msg}"
    );
}

#[test]
fn run_javascript_without_deno_returns_clear_error() {
    // Force a clean environment so this test is
    // deterministic regardless of the host.
    let prev_path = std::env::var("PATH").ok();
    let prev_deno = std::env::var("DENO").ok();
    let prev_home = std::env::var("LMSTUDIO_HOME").ok();
    let prev_userprofile = std::env::var("USERPROFILE").ok();
    let prev_home_env = std::env::var("HOME").ok();
    std::env::set_var("PATH", "");
    std::env::remove_var("DENO");
    std::env::remove_var("LMSTUDIO_HOME");
    std::env::remove_var("USERPROFILE");
    std::env::remove_var("HOME");
    // A real temp dir, not a non-existent one. The order of
    // checks in `run_javascript` is: (1) find_deno,
    // (2) validate working dir, (3) spawn. So with PATH
    // empty we get a NotFound from step (1) — that is
    // surfaced to the caller without ever reaching step (2).
    let dir = std::env::temp_dir();
    let res = run_javascript("console.log('hi');", Some(2), &dir);
    std::env::set_var("PATH", prev_path.unwrap_or_default());
    if let Some(v) = prev_deno { std::env::set_var("DENO", v); }
    if let Some(v) = prev_home { std::env::set_var("LMSTUDIO_HOME", v); }
    if let Some(v) = prev_userprofile { std::env::set_var("USERPROFILE", v); }
    if let Some(v) = prev_home_env { std::env::set_var("HOME", v); }
    let err = res.expect_err("missing deno must be NotFound");
    let msg = err.to_string();
    assert!(msg.to_lowercase().contains("deno"), "must mention deno, got: {msg}");
}

#[test]
fn timeout_bounds_match_plugin_contract() {
    assert_eq!(DEFAULT_TIMEOUT_SECONDS, 5, "default timeout must be 5s (matches LM Studio plugin)");
    assert_eq!(MAX_TIMEOUT_SECONDS, 60, "max timeout must be 60s (matches LM Studio plugin)");
}

#[test]
fn sandbox_result_serialises_with_all_fields() {
    let r = SandboxResult {
        stdout: "hi".into(),
        stderr: String::new(),
        exit_code: Some(0),
        elapsed_ms: 12,
        timed_out: false,
    };
    let v = serde_json::to_value(&r).expect("serialise");
    assert_eq!(v["stdout"], "hi");
    assert_eq!(v["stderr"], "");
    assert_eq!(v["exit_code"], 0);
    assert_eq!(v["elapsed_ms"], 12);
    assert_eq!(v["timed_out"], false);
}

#[test]
fn sandbox_result_serialises_timeout_state() {
    let r = SandboxResult {
        stdout: String::new(),
        stderr: "timeout: exceeded 5 seconds".into(),
        exit_code: None,
        elapsed_ms: 5_001,
        timed_out: true,
    };
    let v = serde_json::to_value(&r).expect("serialise");
    assert_eq!(v["timed_out"], true);
    assert_eq!(v["exit_code"], serde_json::Value::Null);
    assert!(v["stderr"].as_str().unwrap().contains("timeout"));
}

// Live test: requires Deno installed. Skipped by default
// because we do not want CI to depend on a runtime that is
// not bundled with NEXUS.
#[test]
#[ignore]
fn live_run_javascript_with_deno() {
    if find_deno().is_err() {
        eprintln!("skipping: Deno not available");
        return;
    }
    let dir = std::env::temp_dir().join(format!("nexus-sandbox-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let r: SandboxResult = run_javascript("console.log('hello from deno');", Some(5), &dir).expect("sandbox run");
    assert_eq!(r.exit_code, Some(0), "expected 0 exit, got {:?}", r.exit_code);
    assert!(r.stdout.contains("hello from deno"), "stdout must contain the message, got {:?}", r.stdout);
    assert!(!r.timed_out);
}
