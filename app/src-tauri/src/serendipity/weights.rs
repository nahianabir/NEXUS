//! Serendipity scoring weights.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SerendipityWeights {
    /// Memory has a real `importance` field, so this is
    /// typically 0.0. The actual `importance` value is
    /// added on top.
    pub dormant_memory_factor: f64,
    pub dormant_note_factor: f64,
    pub dormant_experiment_factor: f64,
    pub forgotten_hypothesis_base: f64,
    pub forgotten_hypothesis_confidence_factor: f64,
    pub cold_checkpoint_factor: f64,
}

impl Default for SerendipityWeights {
    fn default() -> Self {
        Self {
            dormant_memory_factor: 0.0,
            dormant_note_factor: 0.2,
            dormant_experiment_factor: 0.3,
            forgotten_hypothesis_base: 0.5,
            forgotten_hypothesis_confidence_factor: 0.5,
            cold_checkpoint_factor: 0.1,
        }
    }
}
