//! Deprecated shim.
//!
//! This file used to contain the full config implementation.
//! The canonical implementation is now split into:
//! - `types.rs` (data structures + defaults)
//! - `load.rs`  (IO: load_default + env overrides)
//!
//! Keep this file minimal to avoid duplicated definitions.

pub use super::load::load_default;
pub use super::types::*;

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
    #[serde(default = "default_policy_provider")]
    #[serde(flatten)]
    pub provider: PolicyProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider")]
pub enum PolicyProvider {
    #[serde(rename = "config")]
    Config(ConfigPolicyConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigPolicyConfig {
    #[serde(default = "default_policy_mode")]
    pub mode: String,

    #[serde(default = "default_policy_action")]
    pub default_action: String,

    #[serde(default)]
    pub allowlist: Vec<PolicyRule>,

    #[serde(default = "default_denylist")]
    pub denylist: Vec<PolicyRule>,
}

fn default_policy_provider() -> PolicyProvider {
    PolicyProvider::Config(ConfigPolicyConfig::default())
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

impl Default for ConfigPolicyConfig {
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

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            provider: default_policy_provider(),
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
    
    #[serde(flatten)]
    pub provider: MemoryProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider")]
pub enum MemoryProvider {
    #[serde(rename = "service")]
    Service(MemoryServiceConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryServiceConfig {
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
            provider: MemoryProvider::Service(MemoryServiceConfig {
                base_url: default_memory_url(),
                api_key: "".to_string(),
                timeout_ms: default_timeout_ms(),
                search_limit: default_search_limit(),
                min_score: default_min_score(),
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider")]
pub enum RunnerConfig {
    #[serde(rename = "codecli")]
    CodeCli(CodeCliRunnerConfig),
    #[serde(rename = "replay")]
    Replay(ReplayRunnerConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReplayRunnerConfig {
    pub events_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodeCliRunnerConfig {
    // Local runner configuration fields can be added here
}

impl Default for RunnerConfig {
    fn default() -> Self {
        RunnerConfig::CodeCli(CodeCliRunnerConfig::default())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatekeeperConfig {
    #[serde(default = "default_gatekeeper_provider")]
    #[serde(flatten)]
    pub provider: GatekeeperProvider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider")]
pub enum GatekeeperProvider {
    #[serde(rename = "standard")]
    Standard(StandardGatekeeperConfig),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardGatekeeperConfig {
    #[serde(default = "default_max_inject")]
    pub max_inject: usize,
    #[serde(default = "default_min_level_inject")]
    pub min_level_inject: i32,
    #[serde(default = "default_min_level_fallback")]
    pub min_level_fallback: i32,
    #[serde(default = "default_min_trust_show")]
    pub min_trust_show: f32,
    #[serde(default = "default_block_if_consecutive_fail_ge")]
    pub block_if_consecutive_fail_ge: i32,
    #[serde(default = "default_skip_if_top1_score_ge")]
    pub skip_if_top1_score_ge: f32,
    #[serde(default = "default_exclude_stale_by_default")]
    pub exclude_stale_by_default: bool,
    #[serde(default = "default_active_statuses")]
    pub active_statuses: std::collections::HashSet<String>,
}

// NOTE: Gatekeeper 配置的转换实现迁移到 crate::gatekeeper 模块，
// 以避免 core::config 反向依赖业务模块。

fn default_max_inject() -> usize { 3 }
fn default_min_level_inject() -> i32 { 2 }
fn default_min_level_fallback() -> i32 { 1 }
fn default_min_trust_show() -> f32 { 0.40 }
fn default_block_if_consecutive_fail_ge() -> i32 { 3 }
fn default_skip_if_top1_score_ge() -> f32 { 0.85 }
fn default_exclude_stale_by_default() -> bool { true }
fn default_active_statuses() -> std::collections::HashSet<String> {
    ["active".to_string(), "verified".to_string()].into_iter().collect()
}

fn default_gatekeeper_provider() -> GatekeeperProvider {
    GatekeeperProvider::Standard(StandardGatekeeperConfig::default())
}

impl Default for StandardGatekeeperConfig {
    fn default() -> Self {
        Self {
            max_inject: default_max_inject(),
            min_level_inject: default_min_level_inject(),
            min_level_fallback: default_min_level_fallback(),
            min_trust_show: default_min_trust_show(),
            block_if_consecutive_fail_ge: default_block_if_consecutive_fail_ge(),
            skip_if_top1_score_ge: default_skip_if_top1_score_ge(),
            exclude_stale_by_default: default_exclude_stale_by_default(),
            active_statuses: default_active_statuses(),
        }
    }
}

impl Default for GatekeeperConfig {
    fn default() -> Self {
        Self {
            provider: default_gatekeeper_provider(),
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
    
    if let Ok(v) = std::env::var("MEM_CODECLI_PROJECT_ID") {
        if !v.trim().is_empty() {
            cfg.project_id = v;
        }
    }
    
    let MemoryProvider::Service(ref mut svc_cfg) = cfg.memory.provider;
    if let Ok(v) = std::env::var("MEM_CODECLI_MEMORY_URL") {
        if !v.trim().is_empty() {
            svc_cfg.base_url = v;
        }
    }
    if let Ok(v) = std::env::var("MEM_CODECLI_MEMORY_API_KEY") {
        if !v.trim().is_empty() {
            svc_cfg.api_key = v;
        }
    }

    Ok(cfg)
}
