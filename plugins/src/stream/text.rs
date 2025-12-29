use memex_core::api as core_api;

pub struct TextStreamStrategy;

impl core_api::StreamStrategy for TextStreamStrategy {
    fn apply(&self, _cfg: &mut core_api::AppConfig) -> core_api::StreamPlan {
        core_api::StreamPlan { silent: false }
    }
}
