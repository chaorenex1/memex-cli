// core/src/runner/trait.rs
use async_trait::async_trait;
use bytes::Bytes;
use tokio::io::{AsyncRead, AsyncWrite};
use crate::types::TraceContext;
use crate::tool_events::r#trait::ToolEvent;

#[derive(Clone, Debug)]
pub struct RunnerSpec {
    pub program: String,        // e.g. "codecli"
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct StreamSpec {
    pub stream_stdout: bool,     // whether forward to parent stdout
    pub stream_stderr: bool,
    pub max_capture_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct Captured {
    pub stdout: Bytes,
    pub stderr: Bytes,
}

#[derive(Clone, Debug)]
pub struct RunnerOutput {
    pub status_code: i32,        // normalized: process exit code; signal -> mapped
    pub captured: Captured,
}


pub struct RunnerSession {
    pub status: tokio::sync::oneshot::Receiver<i32>, // child exit code when ends
    pub control_tx: tokio::sync::mpsc::Sender<ControlCommand>,
    pub event_rx: tokio::sync::mpsc::Receiver<ParsedEvent>, // tool events + diagnostics
}


pub enum ControlCommand {
    StdinJsonl(serde_json::Value),   // write line to child stdin
    Abort { reason: String },        // convenience -> policy.abort
}

pub enum ParsedEvent {
    Tool(ToolEvent),
    ParseError { line: String, err: String },
    OutputLine { stream: &'static str, line: String }, // optional for debugging
}


#[async_trait]
pub trait Runner: Send + Sync {
    async fn run(
        &self,
        trace: &TraceContext,
        spec: RunnerSpec,
        stream: StreamSpec,
        // optional explicit stdio handles for advanced integrations
        stdin: Option<Box<dyn AsyncRead + Unpin + Send>>,
        stdout: Option<Box<dyn AsyncWrite + Unpin + Send>>,
        stderr: Option<Box<dyn AsyncWrite + Unpin + Send>>,
    ) -> anyhow::Result<RunnerOutput>;
    async fn start_session(&self, spec: RunnerSpec, stream: StreamSpec) -> anyhow::Result<RunnerSession>;
}




