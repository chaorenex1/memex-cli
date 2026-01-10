use chrono::{DateTime, Local};

use crate::runner::RunOutcome;
use crate::tool_event::ToolEvent;

use super::{GatekeeperDecision, InjectItem, SearchMatch};

pub trait GatekeeperPlugin: Send + Sync {
    fn name(&self) -> &str;

    /// Pre-run: Select QA items to inject into prompt
    ///
    /// This method should only use matches and internal config,
    /// not the RunOutcome (which doesn't exist yet in pre-run phase).
    fn prepare_inject(&self, matches: &[SearchMatch]) -> Vec<InjectItem>;

    /// Post-run: Full evaluation including hit refs, validation plans, candidate decision
    ///
    /// This method uses the actual RunOutcome to make quality assessments.
    /// For pre-run inject selection, use `prepare_inject` instead.
    fn evaluate(
        &self,
        now: DateTime<Local>,
        matches: &[SearchMatch],
        outcome: &RunOutcome,
        events: &[ToolEvent],
    ) -> GatekeeperDecision;
}
