//! Stable re-exports for consumers (`cli`, `plugins`, and external crates).
//!
//! Prefer importing from `memex_core::api` instead of reaching into internal modules.

pub use crate::backend::{BackendPlan, BackendStrategy};
pub use crate::config::{AppConfig, ControlConfig, LoggingConfig};
pub use crate::engine::{run_with_query, RunSessionInput, RunWithQueryArgs, RunnerSpec};
pub use crate::error::{CliError, RunnerError};
pub use crate::events_out::EventsOutTx;
pub use crate::gatekeeper::{GatekeeperDecision, GatekeeperPlugin, SearchMatch};
pub use crate::memory::MemoryPlugin;
pub use crate::runner::{
    run_session, PolicyAction, PolicyPlugin, RunOutcome, RunSessionArgs, RunnerEvent, RunnerPlugin,
    RunnerResult, RunnerSession, RunnerStartArgs, Signal,
};

