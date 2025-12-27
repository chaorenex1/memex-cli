use crate::config::{PolicyConfig, PolicyRule};
use crate::tool_event::ToolEvent;

#[derive(Clone)]
pub struct PolicyEngine {
    cfg: PolicyConfig,
}

#[derive(Debug)]
pub struct PolicyDecision {
    pub decision: &'static str,
    pub reason: String,
    pub rule_id: Option<String>,
}

impl PolicyEngine {
    pub fn new(cfg: PolicyConfig) -> Self {
        Self { cfg }
    }

    pub fn decide(&self, req: &ToolEvent) -> PolicyDecision {
        if self.cfg.mode == "off" {
            return PolicyDecision {
                decision: "allow",
                reason: "policy off".into(),
                rule_id: Some("policy.off".into()),
            };
        }

        if let Some((idx, rule)) = find_match(&self.cfg.denylist, req) {
            return PolicyDecision {
                decision: "deny",
                reason: rule.reason.clone().unwrap_or_else(|| "denied by rule".into()),
                rule_id: Some(format!("denylist[{}]", idx)),
            };
        }

        if let Some((idx, rule)) = find_match(&self.cfg.allowlist, req) {
            return PolicyDecision {
                decision: "allow",
                reason: rule.reason.clone().unwrap_or_else(|| "allowed by rule".into()),
                rule_id: Some(format!("allowlist[{}]", idx)),
            };
        }

        let d = self.cfg.default_action.as_str();
        if d == "allow" {
            PolicyDecision {
                decision: "allow",
                reason: "allowed by default_action".into(),
                rule_id: Some("default.allow".into()),
            }
        } else {
            PolicyDecision {
                decision: "deny",
                reason: "denied by default_action".into(),
                rule_id: Some("default.deny".into()),
            }
        }
    }
}

fn find_match<'a>(rules: &'a [PolicyRule], req: &ToolEvent) -> Option<(usize, &'a PolicyRule)> {
    let tool_name = req.tool.as_deref().unwrap_or("");
    for (i, r) in rules.iter().enumerate() {
        if !tool_match(&r.tool, tool_name) {
            continue;
        }
        if let Some(a) = &r.action {
            let ra = a.as_str();
            let qa = req.action.as_deref().unwrap_or("");
            if ra != qa {
                continue;
            }
        }
        return Some((i, r));
    }
    None
}

fn tool_match(pat: &str, tool: &str) -> bool {
    if pat == "*" {
        return true;
    }
    if pat.ends_with('*') {
        let prefix = pat.trim_end_matches('*');
        return tool.starts_with(prefix);
    }
    tool.starts_with(pat)
}
