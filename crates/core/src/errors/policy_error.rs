// core/src/errors/policy_error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("policy config invalid: {0}")]
    InvalidConfig(String),

    #[error("approval required but no interactive input available")]
    NonInteractiveApprovalRequired,

    #[error("approval i/o error")]
    ApprovalIo(#[source] std::io::Error),
}
