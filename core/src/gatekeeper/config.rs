use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::config::{AppConfig, GatekeeperProvider, StandardGatekeeperConfig};

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

    pub digest_head_chars: usize,
    pub digest_tail_chars: usize,
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
            digest_head_chars: 80,
            digest_tail_chars: 80,
        }
    }
}

impl From<StandardGatekeeperConfig> for GatekeeperConfig {
    fn from(c: StandardGatekeeperConfig) -> Self {
        Self {
            max_inject: c.max_inject,
            min_level_inject: c.min_level_inject,
            min_level_fallback: c.min_level_fallback,
            min_trust_show: c.min_trust_show,
            block_if_consecutive_fail_ge: c.block_if_consecutive_fail_ge,
            skip_if_top1_score_ge: c.skip_if_top1_score_ge,
            exclude_stale_by_default: c.exclude_stale_by_default,
            active_statuses: c.active_statuses,
            digest_head_chars: c.digest_head_chars,
            digest_tail_chars: c.digest_tail_chars,
        }
    }
}

impl AppConfig {
    pub fn gatekeeper_logic_config(&self) -> GatekeeperConfig {
        match &self.gatekeeper.provider {
            GatekeeperProvider::Standard(std_cfg) => std_cfg.clone().into(),
        }
    }
}
