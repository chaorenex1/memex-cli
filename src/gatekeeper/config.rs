use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatekeeperConfig {
    pub max_inject: usize,
    pub min_level_inject: i32,
    pub min_level_fallback: i32,
    pub min_trust_show: f32,
    pub block_if_consecutive_fail_ge: i32,
    pub skip_if_top1_score_ge: f32,
    pub exclude_stale_by_default: bool,
    pub active_statuses: HashSet<String>,
}

impl Default for GatekeeperConfig {
    fn default() -> Self {
        Self {
            max_inject: 3,
            min_level_inject: 2,
            min_level_fallback: 1,
            min_trust_show: 0.40,
            block_if_consecutive_fail_ge: 3,
            skip_if_top1_score_ge: 0.85,
            exclude_stale_by_default: true,
            active_statuses: ["active".to_string(), "verified".to_string()]
                .into_iter()
                .collect(),
        }
    }
}
