use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::events_out::EventsOutConfig;
use crate::gatekeeper::GatekeeperConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_project_id")]
    pub project_id: String,

    #[serde(default)]
    pub control: ControlConfig,

    #[serde(default)]
    pub policy: PolicyConfig,

    #[serde(default)]
    pub memory: MemoryConfig,

    #[serde(default)]
    pub events_out: EventsOutConfig,

    #[serde(default)]
    pub gatekeeper: GatekeeperConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            project_id: "default-project".to_string(),
            control: ControlConfig::default(),
            policy: PolicyConfig::default(),
            memory: MemoryConfig::default(),
            events_out: EventsOutConfig::default(),
            gatekeeper: GatekeeperConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlConfig {
    #[serde(default = "default_fail_mode")]
    pub fail_mode: String,

    #[serde(default = "default_decision_timeout_ms")]
    pub decision_timeout_ms: u64,

    #[serde(default = "default_abort_grace_ms")]
    pub abort_grace_ms: u64,
}

fn default_fail_mode() -> String {
    "closed".to_string()
}

fn default_decision_timeout_ms() -> u64 {
    300_000
}

fn default_abort_grace_ms() -> u64 {
    5_000
}

impl Default for ControlConfig {
    fn default() -> Self {
        Self {
            fail_mode: default_fail_mode(),
            decision_timeout_ms: default_decision_timeout_ms(),
            abort_grace_ms: default_abort_grace_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    #[serde(default = "default_policy_mode")]
    pub mode: String,

    #[serde(default = "default_policy_action")]
    pub default_action: String,

    #[serde(default)]
    pub allowlist: Vec<PolicyRule>,

    #[serde(default = "default_denylist")]
    pub denylist: Vec<PolicyRule>,
}

fn default_policy_mode() -> String {
    "auto".to_string()
}

fn default_policy_action() -> String {
    "deny".to_string()
}

fn default_denylist() -> Vec<PolicyRule> {
    vec![
        PolicyRule {
            tool: "shell.exec".into(),
            action: Some("exec".into()),
            reason: Some("shell is denied by default".into()),
        },
        PolicyRule {
            tool: "net.http".into(),
            action: Some("net".into()),
            reason: Some("network is denied by default".into()),
        },
    ]
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            mode: default_policy_mode(),
            default_action: default_policy_action(),
            allowlist: vec![
                PolicyRule {
                    tool: "fs.read".into(),
                    action: Some("read".into()),
                    reason: Some("read is allowed".into()),
                },
                PolicyRule {
                    tool: "git.*".into(),
                    action: None,
                    reason: Some("git commands allowed".into()),
                },
            ],
            denylist: default_denylist(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub tool: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_memory_enabled")]
    pub enabled: bool,
    #[serde(default = "default_memory_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    #[serde(default = "default_search_limit")]
    pub search_limit: u32,
    #[serde(default = "default_min_score")]
    pub min_score: f32,
}

fn default_memory_enabled() -> bool {
    true
}

fn default_memory_url() -> String {
    "https://memory.internal".to_string()
}

fn default_timeout_ms() -> u64 {
    10_000
}

fn default_search_limit() -> u32 {
    6
}

fn default_min_score() -> f32 {
    0.2
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: default_memory_enabled(),
            base_url: default_memory_url(),
            api_key: "".to_string(),
            timeout_ms: default_timeout_ms(),
            search_limit: default_search_limit(),
            min_score: default_min_score(),
        }
    }
}

pub fn load_default() -> anyhow::Result<AppConfig> {
    let mut cfg: AppConfig = if Path::new("config.toml").exists() {
        let s = std::fs::read_to_string("config.toml")?;
        toml::from_str::<AppConfig>(&s)?
    } else {
        AppConfig::default()
    };
    if let Ok(v) = std::env::var("MEM_CODECLI_MEMORY_URL") {
        if !v.trim().is_empty() {
            cfg.memory.base_url = v;
        }
    }
    if let Ok(v) = std::env::var("MEM_CODECLI_MEMORY_API_KEY") {
        if !v.trim().is_empty() {
            cfg.memory.api_key = v;
        }
    }

    Ok(cfg)
}
