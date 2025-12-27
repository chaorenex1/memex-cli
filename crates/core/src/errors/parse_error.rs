// core/src/errors/parse_error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid json line")]
    InvalidJson(#[source] serde_json::Error),

    #[error("unknown event type: {0}")]
    UnknownEventType(String),

    #[error("missing required field: {0}")]
    MissingField(&'static str),

    #[error("schema mismatch: {0}")]
    SchemaMismatch(String),
}
