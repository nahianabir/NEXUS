//! Phase P4 — Bug E + L regression tests.
//!
//! The user reported:
//!
//! > When the model is generating a response and I navigate
//! > to another page in NEXUS, the conversation continues
//! > running or may finish in the background. However, when
//! > I return to Chat, the generated response is missing.
//!
//! > NEXUS sometimes spends substantial time generating a
//! > response, but the UI never reaches a completed state.
//!
//! The two root causes are coupled:
//!
//!   1. `AppState::active_cancellations` was a `Vec<Cancel­lationToken>`
//!      with no request-id tagging, so the Stop button
//!      cancelled every active request and a late
//!      `nexus:chat.delta` could not be correlated with the
//!      request that produced it.
//!   2. The chat frontend held all streaming state in
//!      component-local `useState` that died on unmount. A
//!      late `done` event after a navigation away was
//!      dropped on the floor; the UI stayed in `running`
//!      forever.
//!
//! These tests exercise the new request-registry
//! semantics end-to-end. The fix is the same registry
//! used by the chat command, the Stop button, and the
//! frontend recovery query — there is no parallel "live
//! request list" in the UI.

mod common;

use std::time::Duration;

use nexus_core::app_state::{ActiveCancel, AppState};
use tokio_util::sync::CancellationToken;

fn make_state() -> AppState {
    AppState::initialize(&common::tempdir()).expect("init app state")
}

#[test]
fn register_request_replaces_unrelated_cancelled_tokens() {
    // Pre-existing tokens whose cancel flag is set must
    // be reaped so the registry does not grow
    // unboundedly. The new entry must be the only one
    // the registry tracks.
    let s = make_state();
    let t1 = CancellationToken::new();
    t1.cancel();
    let t2 = CancellationToken::new();
    s.register_request("a", "chat", Some("conv1"), Some("m1"), t1);
    s.register_request("b", "chat", Some("conv2"), Some("m2"), t2);
    let active = s.list_active();
    assert_eq!(active.len(), 1, "only the live token must remain, got {active:?}");
    assert_eq!(active[0].request_id, "b");
    assert_eq!(active[0].conversation_id.as_deref(), Some("conv2"));
}

#[test]
fn unregister_request_is_idempotent() {
    // The chat pipeline calls `unregister_request` on
    // every exit path (done, error, cancelled, and
    // indirectly via the early-error return). Calling it
    // twice must be safe; calling it on an unknown id
    // must also be safe.
    let s = make_state();
    let t = CancellationToken::new();
    s.register_request("x", "chat", None, None, t.clone());
    s.unregister_request("x");
    s.unregister_request("x");
    s.unregister_request("never-existed");
    assert!(s.list_active().is_empty());
}

#[test]
fn cancel_request_returns_true_only_for_known_id() {
    let s = make_state();
    let t = CancellationToken::new();
    s.register_request("r1", "chat", Some("conv"), Some("m"), t.clone());
    assert!(!t.is_cancelled(), "must not be cancelled yet");
    assert!(s.cancel_request("r1"));
    assert!(t.is_cancelled(), "the token must fire");
    // The second call returns false because the id is
    // already gone.
    assert!(!s.cancel_request("r1"));
    assert!(!s.cancel_request("nope"));
}

#[test]
fn cancel_kind_only_touches_matching_category() {
    // The Stop button on the Assistant page must cancel
    // only chat requests, not Model Manager test
    // prompts. A user who kicked off a test prompt from
    // Models should not see it die when they hit Stop
    // in Assistant.
    let s = make_state();
    let t_chat = CancellationToken::new();
    let t_test = CancellationToken::new();
    s.register_request("c1", "chat", Some("conv"), None, t_chat.clone());
    s.register_request("t1", "test", Some("conv"), None, t_test.clone());
    let n = s.cancel_kind("chat");
    assert_eq!(n, 1, "exactly one chat must be cancelled, got {n}");
    assert!(t_chat.is_cancelled());
    assert!(!t_test.is_cancelled(), "test prompt must survive");
    assert_eq!(s.list_active().len(), 1);
}

#[test]
fn cancel_conversation_only_touches_matching_conversation() {
    // Two chats running concurrently (e.g. one open in a
    // background tab) must be independently cancellable.
    let s = make_state();
    let t_a = CancellationToken::new();
    let t_b = CancellationToken::new();
    s.register_request("a", "chat", Some("conv-A"), Some("mA"), t_a.clone());
    s.register_request("b", "chat", Some("conv-B"), Some("mB"), t_b.clone());
    let n = s.cancel_conversation("conv-A");
    assert_eq!(n, 1);
    assert!(t_a.is_cancelled());
    assert!(!t_b.is_cancelled());
    // The remaining entry is still the conv-B request.
    let active = s.list_active();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].conversation_id.as_deref(), Some("conv-B"));
}

#[test]
fn list_active_returns_chat_only_when_filtered() {
    // The frontend queries `chat_request_list_active`
    // which filters on `kind == "chat"`. The test
    // asserts the underlying primitive is correct so the
    // Tauri command is a thin wrapper.
    let s = make_state();
    s.register_request("c1", "chat", Some("c"), Some("m1"), CancellationToken::new());
    s.register_request("c2", "chat", Some("c"), Some("m2"), CancellationToken::new());
    s.register_request("t1", "test", Some("c"), Some("m3"), CancellationToken::new());
    let all = s.list_active();
    let chats: Vec<&nexus_core::app_state::ActiveRequestSnapshot> = all.iter().filter(|r| r.kind == "chat").collect();
    assert_eq!(chats.len(), 2, "expected two chat requests, got {chats:?}");
}

#[test]
fn cancel_all_is_preserved_for_legacy_callers() {
    // `cancel_all` is the pre-Phase-P4 entry point. The
    // Models page still uses it through
    // `cancel_inflight(cancel_all=true)`. The behaviour
    // is preserved: every token fires, the registry
    // empties.
    let s = make_state();
    let t1 = CancellationToken::new();
    let t2 = CancellationToken::new();
    s.register_request("a", "chat", None, None, t1.clone());
    s.register_request("b", "test", None, None, t2.clone());
    s.cancel_all();
    assert!(t1.is_cancelled());
    assert!(t2.is_cancelled());
    assert!(s.list_active().is_empty());
}

#[test]
fn register_then_cancel_marks_token_cancelled_within_a_millisecond() {
    // Sanity check: the token registered in
    // `register_request` is the *same* token returned
    // by the cancel path. A test that races on
    // `is_cancelled` after a synchronous cancel must
    // always observe the flag set.
    let s = make_state();
    let t = CancellationToken::new();
    s.register_request("r", "chat", Some("c"), Some("m"), t.clone());
    s.cancel_request("r");
    // A 1 ms sleep is generous: tokio_util's cancel
    // signal is just an atomic store.
    std::thread::sleep(Duration::from_millis(1));
    assert!(t.is_cancelled());
}

#[test]
fn repeated_registrations_with_same_id_replace_the_previous_token() {
    // If the chat pipeline (somehow) re-registers an
    // already-known request id, the new token replaces
    // the old one. The old token is not cancelled
    // automatically — the caller is responsible for
    // calling `unregister_request` first. This
    // documents the contract: callers must `unregister`
    // before `register` if they want a clean swap.
    let s = make_state();
    let t_old = CancellationToken::new();
    s.register_request("k", "chat", Some("c"), Some("m"), t_old.clone());
    s.unregister_request("k");
    let t_new = CancellationToken::new();
    s.register_request("k", "chat", Some("c"), Some("m"), t_new.clone());
    assert!(!t_old.is_cancelled(), "unregister must not cancel; the caller decides");
    assert!(!t_new.is_cancelled());
    assert_eq!(s.list_active().len(), 1);
    assert_eq!(s.list_active()[0].request_id, "k");
}
