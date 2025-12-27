// core/src/errors/core_error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("pipeline aborted: {reason}")]
    Aborted { reason: String },

    #[error("gatekeeper rejected candidate: {reason}")]
    GatekeeperRejected { reason: String },

    #[error("tool request denied: {tool} ({reason})")]
    ToolDenied { tool: String, reason: String },

    #[error("dependency error: {0}")]
    Dependency(#[from] DependencyError),
}

#[derive(Debug, Error)]
pub enum DependencyError {
    #[error("runner error: {0}")]
    Runner(#[from] crate::errors::runner_error::RunnerError),

    #[error("memory error: {0}")]
    Memory(#[from] crate::errors::memory_error::MemoryError),

    #[error("policy error: {0}")]
    Policy(#[from] crate::errors::policy_error::PolicyError),

    #[error("parse error: {0}")]
    Parse(#[from] crate::errors::parse_error::ParseError),

    #[error("redaction error: {0}")]
    Redact(#[from] crate::errors::redact_error::RedactError),
}
