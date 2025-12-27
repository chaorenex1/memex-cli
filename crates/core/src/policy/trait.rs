// core/src/policy/trait.rs
use async_trait::async_trait;
use serde_json::Value;
use crate::types::{ToolName, AuditMode};

#[derive(Clone, Debug)]
pub enum ToolAction {
    Read,
    Write,
    Net,
    Exec,
}

#[derive(Clone, Debug)]
pub struct ToolRequest {
    pub tool: ToolName,
    pub action: ToolAction,
    pub args: Value,                 // raw args (redacted later)
    pub rationale: Option<String>,    // model-provided reason if available
}

#[derive(Clone, Debug)]
pub enum PolicyDecisionKind {
    Allow,
    Deny,
    Ask,
}

#[derive(Clone, Debug)]
pub struct PolicyDecision {
    pub kind: PolicyDecisionKind,
    pub reason: String,
    pub rule_id: Option<String>,
}

#[async_trait]
pub trait PolicyEngine: Send + Sync {
    async fn decide(&self, mode: AuditMode, req: ToolRequest) -> anyhow::Result<PolicyDecision>;
}

#[async_trait]
pub trait Approver: Send + Sync {
    fn approve(&self, prompt: &str) -> anyhow::Result<bool>;
}


