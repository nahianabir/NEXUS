//! Radar scoring weights.
//!
//! Every opportunity's `score` is the sum of a small set of
//! typed weights. The weights are pinned here so the radar's
//! output is stable across runs and across builds, and so the
//! test suite can assert exact values.
//!
//! The default weights are chosen so that an "obvious" item
//! (e.g. an urgent task 3 days overdue) lands around 100 and a
//! "noisy" item (e.g. a recent 0.7-importance memory) lands
//! around 1.0 — but the unitless scale is not load-bearing;
//! only the relative ordering matters for the UI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Weights {
    pub overdue_urgent_task_base: f64,
    pub overdue_high_task_base: f64,
    pub overdue_day_factor: f64,
    pub stale_open_task_base: f64,
    pub stale_day_factor: f64,
    pub resumed_checkpoint_base: f64,
    pub resumed_pending_factor: f64,
    pub high_importance_memory_base: f64,
    pub high_importance_factor: f64,
    pub open_hypothesis_base: f64,
    pub open_hypothesis_confidence_factor: f64,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            overdue_urgent_task_base: 100.0,
            overdue_high_task_base: 60.0,
            overdue_day_factor: 5.0,
            stale_open_task_base: 25.0,
            stale_day_factor: 1.5,
            resumed_checkpoint_base: 30.0,
            resumed_pending_factor: 8.0,
            high_importance_memory_base: 0.5,
            high_importance_factor: 1.0,
            open_hypothesis_base: 5.0,
            open_hypothesis_confidence_factor: 20.0,
        }
    }
}
