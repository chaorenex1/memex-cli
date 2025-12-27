use async_trait::async_trait;
use anyhow::Result;
use memex_core::memory::client::MemoryClient;
use memex_core::memory::parse_search_matches;
use memex_core::memory::models::{QASearchPayload, QAHitsPayload, QACandidatePayload, QAValidationPayload};
use memex_core::gatekeeper::SearchMatch;
use super::r#trait::MemoryPlugin;

pub struct MemoryServicePlugin {
    client: MemoryClient,
}

impl MemoryServicePlugin {
    pub fn new(base_url: String, api_key: String, timeout_ms: u64) -> Result<Self> {
        let client = MemoryClient::new(base_url, api_key, timeout_ms)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl MemoryPlugin for MemoryServicePlugin {
    fn name(&self) -> &str {
        "memory_service"
    }

    async fn search(&self, payload: QASearchPayload) -> Result<Vec<SearchMatch>> {
        let raw = self.client.search(payload).await?;
        parse_search_matches(&raw).map_err(|e: String| anyhow::anyhow!(e))
    }

    async fn record_hit(&self, payload: QAHitsPayload) -> Result<()> {
        self.client.send_hit(payload).await?;
        Ok(())
    }

    async fn record_candidate(&self, payload: QACandidatePayload) -> Result<()> {
        self.client.send_candidate(payload).await?;
        Ok(())
    }

    async fn record_validation(&self, payload: QAValidationPayload) -> Result<()> {
        self.client.send_validate(payload).await?;
        Ok(())
    }
}
