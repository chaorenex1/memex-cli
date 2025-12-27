use crate::tool_event::ToolEvent;

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub exit_code: i32,
    pub duration_ms: Option<u64>,
    pub stdout_tail: String,
    pub stderr_tail: String,
    pub tool_events: Vec<ToolEvent>,

    pub shown_qa_ids: Vec<String>,
    pub used_qa_ids: Vec<String>,
}
