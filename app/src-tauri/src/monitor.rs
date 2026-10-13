//! Local system telemetry sampler. Cheap, single-threaded, and used to populate
//! the dashboard without polling the OS too aggressively.
//!
//! CPU measurement basis (documented for the frontend):
//!
//! * `cpu_percent` is the **system-wide** global CPU usage averaged across
//!   all logical processors, as reported by `sysinfo`. This matches the
//!   value shown in Windows Task Manager's "CPU" column at the bottom of
//!   the "Performance" tab.
//! * The first 1-2 samples after creation are reported with
//!   `cpu_percent: -1.0` and `cpu_available: false`. This is intentional:
//!   `sysinfo` requires the CPU counters to be primed before any
//!   meaningful delta can be computed. Showing `0%` or `100%` while the
//!   baseline is being established would be misleading; reporting
//!   `unavailable` is the truthful state.
//! * The monitor instance should be created **once** and reused across
//!   samples. Creating a fresh `SystemMonitor` per request resets the
//!   prime counter and the dashboard will perpetually show the
//!   un-primed value. `dashboard_summary` (the only caller) reuses
//!   the same monitor from `AppState`.

use serde::Serialize;
use sysinfo::{Pid, System};

#[derive(Debug, Clone, Serialize)]
pub struct SystemSample {
    /// System-wide global CPU usage, 0.0..=100.0. `-1.0` when the
    /// monitor has not yet produced a meaningful sample.
    pub cpu_percent: f32,
    /// Whether `cpu_percent` is a real, trustworthy value.
    pub cpu_available: bool,
    /// A short, human-readable label describing what `cpu_percent`
    /// actually measures, e.g. "system · all logical processors".
    pub cpu_basis: &'static str,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
    pub proc_mem_mb: u64,
    pub collected_at: i64,
}

pub struct SystemMonitor {
    sys: System,
    /// Number of times `refresh_cpu_usage` has been called. Used to
    /// discard the prime sample(s) before returning a value.
    cpu_primes: u32,
    /// Number of completed samples returned to the caller. Used to
    /// mark the very first sample as "still priming" since `sysinfo`
    /// typically needs two refreshes to give a stable value.
    samples_returned: u32,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_memory();
        // Prime #1: establish the initial CPU-time snapshot.
        // `global_cpu_usage()` cannot return a meaningful value until
        // at least one prior refresh has been recorded.
        sys.refresh_cpu_usage();
        Self {
            sys,
            cpu_primes: 1,
            samples_returned: 0,
        }
    }

    pub fn sample(&mut self) -> SystemSample {
        self.sys.refresh_memory();
        // Prime #2..N: each call advances the per-CPU time counters
        // so the next `global_cpu_usage()` can compute a delta. On
        // Windows the kernel updates these counters at ~1 Hz so the
        // caller must allow at least a few hundred milliseconds
        // between calls for a stable reading. The frontend dashboard
        // polls every 3 s which is well above that floor.
        self.sys.refresh_cpu_usage();
        self.cpu_primes += 1;

        // The first two refreshes are priming the counters. We have
        // already called refresh once in `new` and once just now
        // inside `sample`, so once `cpu_primes >= 3` we have at least
        // one full delta to read from.
        let (cpu_percent, cpu_available) = if self.cpu_primes >= 3 {
            let v = self.sys.global_cpu_usage().clamp(0.0, 100.0);
            (v, true)
        } else {
            // Still warming up; do not invent a value.
            (-1.0, false)
        };

        let me = sysinfo::get_current_pid().ok();
        let proc_mem = me
            .and_then(|p| self.sys.process(p).map(|pr| pr.memory()))
            .unwrap_or(0);

        self.samples_returned += 1;
        SystemSample {
            cpu_percent,
            cpu_available,
            cpu_basis: "system · all logical processors",
            mem_used_mb: self.sys.used_memory() / 1024 / 1024,
            mem_total_mb: self.sys.total_memory() / 1024 / 1024,
            proc_mem_mb: proc_mem / 1024 / 1024,
            collected_at: chrono::Utc::now().timestamp_millis(),
        }
    }
}

#[allow(dead_code)]
fn _ensure_imports(_: Pid) {}
