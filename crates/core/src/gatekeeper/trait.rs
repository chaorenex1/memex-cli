// core/src/gatekeeper/trait.rs
use async_trait::async_trait;
use serde_json::Value;
use crate::types::{GatekeeperMode, RedactLevel};
use crate::memory::r#trait::{SearchResponse, CandidateRequest, ValidateRequest};

#[derive(Clone, Debug)]
pub struct GatekeeperInput {
    pub mode: GatekeeperMode,
    pub redact_level: RedactLevel,

    pub user_query: String,          // current user request (cli args joined)
    pub injected_items: SearchResponse,
    pub final_stdout: String,        // captured stdout (decoded)
    pub final_stderr: String,        // captured stderr (decoded)
    
    // Execution context for validation
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Clone, Debug)]
pub struct GatekeeperDecision {
    pub should_write_candidate: bool,
    pub candidate: Option<CandidateRequest>, 
    
    pub should_validate: bool,
    pub validate: Vec<ValidateRequest>, // Can validate multiple injected items
    
    pub reasons: Vec<String>,
    pub signals: Value,              // structured scores for logging
}

#[async_trait]
pub trait Gatekeeper: Send + Sync {
    async fn evaluate(&self, input: GatekeeperInput) -> anyhow::Result<GatekeeperDecision>;
}
