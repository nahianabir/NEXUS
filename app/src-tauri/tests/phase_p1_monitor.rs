//! Phase 1 (P1) regression tests — system monitor.
//!
//! These tests exercise `SystemMonitor` to verify the CPU
//! measurement behaves sensibly across multiple samples:
//!
//! * The first 1-2 samples are marked `cpu_available: false`
//!   because `sysinfo` requires CPU counters to be primed before a
//!   trustworthy delta can be read.
//! * After priming, `cpu_percent` is clamped to the documented
//!   `0.0..=100.0` range and `cpu_available: true`.
//! * The reported `cpu_basis` clearly explains what is being
//!   measured (system-wide, all logical processors) so the UI can
//!   display it next to the value.
//! * A long-lived monitor returns the `cpu_available: true` value
//!   for every sample after priming; creating a fresh monitor per
//!   request would perpetually report `cpu_available: false`.

use nexus_core::monitor::SystemMonitor;

#[test]
fn first_sample_is_unavailable() {
    let mut mon = SystemMonitor::new();
    let s = mon.sample();
    // The very first sample is still priming the CPU counters.
    // `cpu_percent` should be -1.0 and `cpu_available` should be
    // false so the frontend can show "priming…" rather than a
    // bogus 0% or 100% value.
    assert!(!s.cpu_available, "first sample must not be available");
    assert_eq!(s.cpu_percent, -1.0, "first sample must report -1.0 sentinel");
    assert!(!s.cpu_basis.is_empty(), "cpu_basis must be populated");
    assert!(s.cpu_basis.contains("system"), "cpu_basis should mention system-wide measurement");
}

#[test]
fn eventually_produces_available_sample() {
    let mut mon = SystemMonitor::new();
    let mut available = false;
    for _ in 0..6 {
        let s = mon.sample();
        if s.cpu_available {
            available = true;
            // Once available, the value must be clamped to the
            // documented 0.0..=100.0 range.
            assert!((0.0..=100.0).contains(&s.cpu_percent), "cpu_percent must be in 0..=100, got {}", s.cpu_percent);
        }
    }
    // A persistent monitor must reach the available state within a
    // handful of samples (the prime counter advances one step per
    // sample plus the initial prime in `new`).
    assert!(available, "after several samples the monitor must produce a cpu_available sample");
}

#[test]
fn memory_measurement_is_immediate() {
    // Memory does not need a prime cycle; the first sample should
    // already have plausible (possibly zero) totals. The contract
    // is that the call never errors and returns a non-negative
    // value.
    let mut mon = SystemMonitor::new();
    let s = mon.sample();
    // `mem_total_mb` is a `u64` so the >= 0 check is trivially true;
    // the contract we actually want is that the call returns a sane
    // non-negative pair. We still assert the upper bound.
    assert!(s.mem_used_mb <= s.mem_total_mb, "mem_used_mb cannot exceed mem_total_mb");
}

#[test]
fn collected_at_is_monotonic() {
    // `collected_at` is wall-clock millis; two samples must not
    // move backwards in time. (A clock jump backwards is out of
    // scope.)
    let mut mon = SystemMonitor::new();
    let s1 = mon.sample();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let s2 = mon.sample();
    assert!(s2.collected_at >= s1.collected_at, "collected_at must be monotonic");
}
