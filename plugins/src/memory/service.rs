use super::r#trait::MemoryPlugin;
use anyhow::Result;
use async_trait::async_trait;
use memex_core::api as core_api;

pub struct MemoryServicePlugin {
    client: core_api::MemoryClient,
}

impl MemoryServicePlugin {
    pub fn new(base_url: String, api_key: String, timeout_ms: u64) -> Result<Self> {
        let client = core_api::MemoryClient::new(base_url, api_key, timeout_ms)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl MemoryPlugin for MemoryServicePlugin {
    fn name(&self) -> &str {
        "memory_service"
    }

    async fn search(
        &self,
        payload: core_api::QASearchPayload,
    ) -> Result<Vec<core_api::SearchMatch>> {
        let raw = self.client.search(payload).await?;
        core_api::parse_search_matches(&raw).map_err(|e: String| anyhow::anyhow!(e))
    }

    async fn record_hit(&self, payload: core_api::QAHitsPayload) -> Result<()> {
        self.client.send_hit(payload).await?;
        Ok(())
    }

    async fn record_candidate(&self, payload: core_api::QACandidatePayload) -> Result<()> {
        self.client.send_candidate(payload).await?;
        Ok(())
    }

    async fn record_validation(&self, payload: core_api::QAValidationPayload) -> Result<()> {
        self.client.send_validate(payload).await?;
        Ok(())
    }
}
