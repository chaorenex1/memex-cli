use crate::config::AppConfig;
use crate::error::RunnerError;
use crate::events_out::{start_events_out, EventsOutTx};
use crate::state::StateManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppContext {
    cfg: AppConfig,
    state_manager: Option<Arc<StateManager>>,
    events_out: Option<EventsOutTx>,
}

impl AppContext {
    pub async fn new(
        cfg: AppConfig,
        state_manager: Option<Arc<StateManager>>,
    ) -> Result<Self, RunnerError> {
        let events_out = start_events_out(&cfg.events_out)
            .await
            .map_err(RunnerError::Spawn)?;
        Ok(Self {
            cfg,
            state_manager,
            events_out,
        })
    }

    pub fn cfg(&self) -> &AppConfig {
        &self.cfg
    }

    pub fn state_manager(&self) -> Option<Arc<StateManager>> {
        self.state_manager.clone()
    }

    pub fn events_out(&self) -> Option<EventsOutTx> {
        self.events_out.clone()
    }
}
