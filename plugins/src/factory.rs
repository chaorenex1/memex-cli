use anyhow::Result;

use memex_core::config::{AppConfig, GatekeeperProvider, MemoryProvider, PolicyProvider, RunnerConfig};
use memex_core::runner::RunnerPlugin;

use crate::gatekeeper::StandardGatekeeperPlugin;
use crate::memory::service::MemoryServicePlugin;
use crate::policy::config_rules::ConfigPolicyPlugin;
use crate::runner::codecli::CodeCliRunnerPlugin;
use crate::runner::replay::ReplayRunnerPlugin;

pub fn build_memory(cfg: &AppConfig) -> Result<Option<Box<dyn memex_core::memory::MemoryPlugin>>> {
    if !cfg.memory.enabled {
        return Ok(None);
    }

    match &cfg.memory.provider {
        MemoryProvider::Service(svc_cfg) => Ok(Some(Box::new(MemoryServicePlugin::new(
            svc_cfg.base_url.clone(),
            svc_cfg.api_key.clone(),
            svc_cfg.timeout_ms,
        )?))),
    }
}

pub fn build_runner(cfg: &AppConfig) -> Box<dyn RunnerPlugin> {
    match &cfg.runner {
        RunnerConfig::CodeCli(_) => Box::new(CodeCliRunnerPlugin::new()),
        RunnerConfig::Replay(r_cfg) => Box::new(ReplayRunnerPlugin::new(r_cfg.events_file.clone())),
    }
}

pub fn build_policy(
    cfg: &AppConfig,
) -> Option<Box<dyn memex_core::runner::PolicyPlugin>> {
    match &cfg.policy.provider {
        PolicyProvider::Config(_) => Some(Box::new(ConfigPolicyPlugin::new(cfg.policy.clone()))),
    }
}

pub fn build_gatekeeper(
    cfg: &AppConfig,
) -> Box<dyn memex_core::gatekeeper::GatekeeperPlugin> {
    match &cfg.gatekeeper.provider {
        GatekeeperProvider::Standard(std_cfg) => Box::new(StandardGatekeeperPlugin::new(std_cfg.clone().into())),
    }
}
