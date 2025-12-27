// core/src/tool_events/trait.rs
use async_trait::async_trait;
use crate::errors::parse_error::ParseError;

#[derive(Clone, Debug)]
pub enum ToolEventSource {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug)]
pub struct ToolEventEnvelope {
    pub raw_line: String,
    pub source: ToolEventSource,
}

#[derive(Clone, Debug)]
pub enum ToolEvent {
    ToolRequest(ToolRequestEvent),
    ToolResult(ToolResultEvent),
    ToolProgress(ToolProgressEvent),
}

#[derive(Clone, Debug)]
pub struct ToolRequestEvent {
    pub id: String,                 // event id
    pub tool: String,               // tool name
    pub action: String,             // read|write|net|exec
    pub args: serde_json::Value,
    pub rationale: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ToolResultEvent {
    pub id: String,                 // event id (same as request)
    pub ok: bool,
    pub output: serde_json::Value,  // result payload
    pub error: Option<String>,      // short message if ok=false
}

#[derive(Clone, Debug)]
pub struct ToolProgressEvent {
    pub id: String,
    pub stage: String,
    pub message: Option<String>,
    pub percent: Option<f32>,       // 0..100
}

#[async_trait]
pub trait ToolEventParser: Send + Sync {
    /// Parse a single line. Return Ok(None) if the line is not an event line.
    async fn parse_line(&self, env: ToolEventEnvelope) -> Result<Option<ToolEvent>, ParseError>;
}
