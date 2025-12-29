use std::time::Duration;

use crate::tool_event::ToolEvent;

/// Frontend-facing events emitted by the runner loop.
///
/// This lives under `core::runner` (not `core::tui`) so `core` stays UI-agnostic:
/// TUI/CLI can consume these events, but `core` does not depend on any TUI code.
#[derive(Debug, Clone)]
pub enum RunnerEvent {
    ToolEvent(Box<ToolEvent>),
    AssistantOutput(String),
    RawStdout(String),
    RawStderr(String),
    StatusUpdate {
        tokens: u64,
        duration: Duration,
    },
    RunComplete {
        exit_code: i32,
    },
    Error(String),
}
