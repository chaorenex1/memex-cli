// core/src/errors/config_error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config file not found: {0}")]
    NotFound(String),

    #[error("config parse error")]
    Parse(#[source] anyhow::Error),

    #[error("config validation error: {0}")]
    Validation(String),

    #[error("env var invalid: {key}")]
    EnvInvalid { key: String, #[source] source: anyhow::Error },
}
