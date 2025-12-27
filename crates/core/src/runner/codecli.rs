use async_trait::async_trait;
use crate::runner::r#trait::{Runner, RunnerSpec, StreamSpec, RunnerOutput, RunnerSession, Captured};
use crate::types::TraceContext;
use tokio::io::{AsyncRead, AsyncWrite};

pub struct CodecliRunner;

#[async_trait]
impl Runner for CodecliRunner {
    async fn run(
        &self,
        _trace: &TraceContext,
        _spec: RunnerSpec,
        _stream: StreamSpec,
        _stdin: Option<Box<dyn AsyncRead + Unpin + Send>>,
        _stdout: Option<Box<dyn AsyncWrite + Unpin + Send>>,
        _stderr: Option<Box<dyn AsyncWrite + Unpin + Send>>,
    ) -> anyhow::Result<RunnerOutput> {
        // Placeholder implementation
        Ok(RunnerOutput {
            status_code: 0,
            captured: Captured {
                stdout: bytes::Bytes::new(),
                stderr: bytes::Bytes::new(),
            },
        })
    }

    async fn start_session(&self, _spec: RunnerSpec, _stream: StreamSpec) -> anyhow::Result<RunnerSession> {
        Err(anyhow::anyhow!("Not implemented"))
    }
}
