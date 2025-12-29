use std::sync::Arc;

use tokio::sync::mpsc;

use crate::config::ControlConfig;
use crate::error::RunnerError;
use crate::events_out::EventsOutTx;

use super::runtime;
use super::traits::{PolicyPlugin, RunnerSession};
use super::types::RunnerResult;
use super::RunnerEvent;

pub struct RunSessionArgs<'a> {
    pub session: Box<dyn RunnerSession>,
    pub control: &'a ControlConfig,
    pub policy: Option<Arc<dyn PolicyPlugin>>,
    pub capture_bytes: usize,
    pub events_out: Option<EventsOutTx>,
    pub event_tx: Option<mpsc::UnboundedSender<RunnerEvent>>,
    pub run_id: &'a str,
    pub silent: bool,
}

pub async fn run_session(args: RunSessionArgs<'_>) -> Result<RunnerResult, RunnerError> {
    runtime::run_session_runtime(runtime::RunSessionRuntimeInput {
        session: args.session,
        control_cfg: args.control,
        policy: args.policy,
        capture_bytes: args.capture_bytes,
        events_out: args.events_out,
        event_tx: args.event_tx,
        run_id: args.run_id,
        silent: args.silent,
    })
    .await
}
