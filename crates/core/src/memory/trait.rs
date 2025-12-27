// core/src/memory/trait.rs
use async_trait::async_trait;
use serde_json::Value;
use crate::types::ProjectId;

#[derive(Clone, Debug)]
pub struct SearchRequest {
    pub project_id: ProjectId,
    pub query: String,
    pub limit: u32,
    pub min_score: f32,
}

#[derive(Clone, Debug)]
pub struct QAMatch {
    pub qa_id: String,
    pub question: String,
    pub answer: String,
    pub score: f32,
    pub tags: Vec<String>,
    pub metadata: Value,
}

#[derive(Clone, Debug)]
pub struct SearchResponse {
    pub items: Vec<QAMatch>,
}

#[derive(Clone, Debug)]
pub struct HitRef {
    pub qa_id: String,
    pub shown: Option<bool>,
    pub used: Option<bool>,
    pub message_id: Option<String>,
    pub context: Option<String>,
}

#[derive(Clone, Debug)]
pub struct HitRequest {
    pub project_id: ProjectId,
    pub references: Vec<HitRef>,
}

#[derive(Clone, Debug)]
pub struct CandidateRequest {
    pub project_id: ProjectId,
    pub question: String,
    pub answer: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub confidence: f32,
    pub metadata: Value,
    pub source: Option<String>,
    pub author: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ValidateRequest {
    pub project_id: ProjectId,
    pub qa_id: String,
    pub result: Option<String>,
    pub signal_strength: Option<f32>,
    pub success: Option<bool>,
    pub strong_signal: Option<bool>,
    pub source: Option<String>,
    pub context: Option<String>,
    pub client: Option<String>,
    pub message_id: Option<String>, // from run_id by default
    pub payload: Option<Value>,     // optional evidence payload
}

#[async_trait]
pub trait MemoryClient: Send + Sync {
    async fn search(&self, req: SearchRequest) -> anyhow::Result<SearchResponse>;
    async fn hit(&self, req: HitRequest) -> anyhow::Result<()>;
    async fn candidate(&self, req: CandidateRequest) -> anyhow::Result<()>;
    async fn validate(&self, req: ValidateRequest) -> anyhow::Result<()>;

    // optional maintenance endpoint
    async fn expire(&self, project_id: ProjectId, batch_size: u32) -> anyhow::Result<()>;
}

