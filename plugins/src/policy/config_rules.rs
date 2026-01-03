use async_trait::async_trait;
use memex_core::api as core_api;

pub struct ConfigPolicyPlugin {
    config: core_api::PolicyConfig,
}

impl ConfigPolicyPlugin {
    pub fn new(config: core_api::PolicyConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl core_api::PolicyPlugin for ConfigPolicyPlugin {
    fn name(&self) -> &str {
        "config"
    }

    async fn check(&self, event: &core_api::ToolEvent) -> core_api::PolicyAction {
        let core_api::PolicyProvider::Config(inner_cfg) = &self.config.provider;

        let tool_name = event.tool.as_deref().unwrap_or("unknown");
        let action_name = event.action.as_deref();

        // 1. Check denylist
        for rule in &inner_cfg.denylist {
            if rule_matches(rule, tool_name, action_name) {
                return core_api::PolicyAction::Deny {
                    reason: rule
                        .reason
                        .clone()
                        .unwrap_or_else(|| "Denied by rule".into()),
                };
            }
        }

        // 2. Check allowlist
        for rule in &inner_cfg.allowlist {
            if rule_matches(rule, tool_name, action_name) {
                return core_api::PolicyAction::Allow;
            }
        }

        // 3. Default action
        match inner_cfg.default_action.as_str() {
            "allow" => core_api::PolicyAction::Allow,
            "ask" => core_api::PolicyAction::Ask {
                prompt: format!("Allow tool {}?", tool_name),
            },
            _ => core_api::PolicyAction::Deny {
                reason: "Default deny".into(),
            },
        }
    }
}

fn rule_matches(rule: &core_api::PolicyRule, tool: &str, action: Option<&str>) -> bool {
    // Simple wildcard matching for now
    if rule.tool == "*" || rule.tool == tool {
        if let Some(rule_action) = &rule.action {
            if let Some(act) = action {
                return rule_action == "*" || rule_action == act;
            }
            return false; // Rule specifies action but event has none
        }
        return true; // Rule matches tool, no action specified (matches all)
    }

    // Handle "git.*" style
    if rule.tool.ends_with(".*") {
        let prefix = &rule.tool[..rule.tool.len() - 2];
        if tool.starts_with(prefix) {
            // We don't check action if tool matches wildcard prefix?
            // Logic depends on requirement. Assuming yes for now.
            return true;
        }
    }

    false
}
