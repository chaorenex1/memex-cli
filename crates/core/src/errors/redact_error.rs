// core/src/errors/redact_error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RedactError {
    #[error("regex pattern invalid")]
    Regex(#[source] regex::Error),

    #[error("redaction failed")]
    Failed(#[source] anyhow::Error),
}
