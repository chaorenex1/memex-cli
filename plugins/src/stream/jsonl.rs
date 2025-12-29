use memex_core::api as core_api;

pub struct JsonlStreamStrategy;

impl core_api::StreamStrategy for JsonlStreamStrategy {
    fn apply(&self, cfg: &mut core_api::AppConfig) -> core_api::StreamPlan {
        // For JSONL mode we force wrapper/tool events to stdout and suppress raw stdout/stderr.
        cfg.events_out.enabled = true;
        cfg.events_out.path = "stdout:".to_string();

        core_api::StreamPlan { silent: true }
    }
}
