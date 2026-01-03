use chrono::{DateTime, Utc};
use memex_core::api as core_api;

pub struct StandardGatekeeperPlugin {
    config: core_api::GatekeeperConfig,
}

impl StandardGatekeeperPlugin {
    pub fn new(config: core_api::GatekeeperConfig) -> Self {
        Self { config }
    }
}

impl core_api::GatekeeperPlugin for StandardGatekeeperPlugin {
    fn name(&self) -> &str {
        "standard"
    }

    fn evaluate(
        &self,
        now: DateTime<Utc>,
        matches: &[core_api::SearchMatch],
        outcome: &core_api::RunOutcome,
        events: &[core_api::ToolEvent],
    ) -> core_api::GatekeeperDecision {
        // Delegate to existing logic in src/gatekeeper/evaluate.rs
        // We might want to move that logic here eventually, but for now delegating is safer.
        core_api::Gatekeeper::evaluate(&self.config, now, matches, outcome, events)
    }
}
