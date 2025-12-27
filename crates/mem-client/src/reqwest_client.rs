use async_trait::async_trait;
use memex_core::memory::r#trait::{
    MemoryClient, SearchRequest, SearchResponse, HitRequest, CandidateRequest, ValidateRequest
};
use memex_core::types::ProjectId;

pub struct HttpMemoryClient;

impl HttpMemoryClient {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl MemoryClient for HttpMemoryClient {
    async fn search(&self, _req: SearchRequest) -> anyhow::Result<SearchResponse> {
        Ok(SearchResponse { items: vec![] })
    }
    async fn hit(&self, _req: HitRequest) -> anyhow::Result<()> {
        Ok(())
    }
    async fn candidate(&self, _req: CandidateRequest) -> anyhow::Result<()> {
        Ok(())
    }
    async fn validate(&self, _req: ValidateRequest) -> anyhow::Result<()> {
        Ok(())
    }
    async fn expire(&self, _project_id: ProjectId, _batch_size: u32) -> anyhow::Result<()> {
        Ok(())
    }
}
