use chrono::{DateTime, Utc};

use crate::runner::RunOutcome;
use crate::tool_event::ToolEvent;

use super::{GatekeeperDecision, SearchMatch};

pub trait GatekeeperPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn evaluate(
        &self,
        now: DateTime<Utc>,
        matches: &[SearchMatch],
        outcome: &RunOutcome,
        events: &[ToolEvent],
    ) -> GatekeeperDecision;
}
