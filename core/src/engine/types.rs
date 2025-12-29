use std::collections::HashMap;
use std::sync::Arc;

use crate::backend::BackendStrategy;
use crate::config::AppConfig;
use crate::events_out::EventsOutTx;
use crate::gatekeeper::GatekeeperPlugin;
use crate::memory::MemoryPlugin;
use crate::runner::{PolicyPlugin, RunnerPlugin, RunnerSession, RunnerStartArgs};
use crate::state::StateManager;

pub struct RunSessionInput {
    pub session: Box<dyn RunnerSession>,
    pub run_id: String,
    pub control: crate::config::ControlConfig,
    pub policy: Option<Arc<dyn PolicyPlugin>>,
    pub capture_bytes: usize,
    pub events_out_tx: Option<EventsOutTx>,
    pub silent: bool,
    pub state_manager: Option<Arc<StateManager>>,
    pub state_session_id: Option<String>,
}

pub enum RunnerSpec {
    Backend {
        strategy: Box<dyn BackendStrategy>,
        backend_spec: String,
        base_envs: HashMap<String, String>,
        resume_id: Option<String>,
        model: Option<String>,
        stream: bool,
        stream_format: String,
    },
    Passthrough {
        runner: Box<dyn RunnerPlugin>,
        session_args: RunnerStartArgs,
    },
}

pub struct RunWithQueryArgs {
    pub user_query: String,
    pub cfg: AppConfig,
    pub runner: RunnerSpec,
    pub run_id: String,
    pub capture_bytes: usize,
    pub silent: bool,
    pub events_out_tx: Option<EventsOutTx>,
    pub state_manager: Option<Arc<StateManager>>,
    pub policy: Option<Arc<dyn PolicyPlugin>>,
    pub memory: Option<Arc<dyn MemoryPlugin>>,
    pub gatekeeper: Arc<dyn GatekeeperPlugin>,
    pub wrapper_start_data: Option<serde_json::Value>,
}
