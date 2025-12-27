// core/src/errors/runner_error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("failed to spawn process: {program}")]
    Spawn { program: String, #[source] source: std::io::Error },

    #[error("io error while streaming: {stream}")]
    StreamIo { stream: &'static str, #[source] source: std::io::Error },

    #[error("process exited unexpectedly: code={code}")]
    UnexpectedExit { code: i32 },

    #[error("process killed by signal: {signal}")]
    Signal { signal: String },

    #[error("stdout decode error (utf-8)")]
    StdoutDecode(#[source] std::string::FromUtf8Error),

    #[error("stderr decode error (utf-8)")]
    StderrDecode(#[source] std::string::FromUtf8Error),
}
