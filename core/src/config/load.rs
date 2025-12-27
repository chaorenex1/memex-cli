use std::path::Path;

use super::types::{AppConfig, MemoryProvider};

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
