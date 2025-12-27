// core/src/errors/memory_error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("memory service unavailable")]
    Unavailable,

    #[error("request timeout")]
    Timeout,

    #[error("unauthorized (check api key)")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("rate limited")]
    RateLimited,

    #[error("bad request: {message}")]
    BadRequest { message: String },

    #[error("unexpected status: {status}")]
    HttpStatus { status: u16, body_snippet: String },

    #[error("transport error")]
    Transport(#[source] anyhow::Error),

    #[error("decode/serde error")]
    Decode(#[source] anyhow::Error),
}

impl MemoryError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, MemoryError::Unavailable | MemoryError::Timeout | MemoryError::RateLimited)
    }
}
