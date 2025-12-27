use async_trait::async_trait;
use memex_core::policy::r#trait::{PolicyEngine as PolicyEngineTrait, PolicyDecision, PolicyDecisionKind, ToolRequest};
use memex_core::types::AuditMode;

pub struct PolicyEngine;

impl PolicyEngine {
    pub fn allow_all() -> Self {
        Self
    }
}

#[async_trait]
impl PolicyEngineTrait for PolicyEngine {
    async fn decide(&self, _mode: AuditMode, _req: ToolRequest) -> anyhow::Result<PolicyDecision> {
        Ok(PolicyDecision {
            kind: PolicyDecisionKind::Allow,
            reason: "Allow all by default".to_string(),
            rule_id: None,
        })
    }
}
