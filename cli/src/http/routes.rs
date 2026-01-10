//! HTTP路由handlers

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use chrono::Local;
use memex_core::api::{
    QACandidatePayload, QAHitsPayload, QAReferencePayload, QASearchPayload, QAValidationPayload,
};

use crate::http::{
    models::*,
    state::AppState,
    validation::{validate_candidate, validate_project_id},
};

/// 创建所有路由
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/search", post(search_handler))
        .route("/api/v1/record-candidate", post(record_candidate_handler))
        .route("/api/v1/record-hit", post(record_hit_handler))
        .route("/api/v1/validate", post(validate_handler))
        .route("/health", get(health_handler))
        .route("/api/v1/shutdown", post(shutdown_handler))
        .with_state(state)
}

/// POST /api/v1/search - 搜索记忆
async fn search_handler(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, HttpServerError> {
    // 更新统计
    {
        let mut stats = state.stats.write().unwrap();
        stats.increment_request("/api/v1/search");
    }

    // 验证 project_id
    validate_project_id(&req.project_id)?;

    // 检查 memory 服务
    let memory =
        state.services.memory.as_ref().ok_or_else(|| {
            HttpServerError::MemoryService("Memory service not configured".into())
        })?;

    // 构建 payload
    let payload = QASearchPayload {
        project_id: req.project_id,
        query: req.query,
        limit: req.limit,
        min_score: req.min_score,
    };

    // 调用 memory 服务
    match memory.search(payload).await {
        Ok(results) => {
            let data = serde_json::to_value(results).unwrap_or_default();
            Ok(Json(SearchResponse {
                success: true,
                data: Some(data),
                error: None,
                error_code: None,
            }))
        }
        Err(e) => {
            let mut stats = state.stats.write().unwrap();
            stats.increment_error();
            Err(HttpServerError::MemoryService(e.to_string()))
        }
    }
}

/// POST /api/v1/record-candidate - 记录候选QA
async fn record_candidate_handler(
    State(state): State<AppState>,
    Json(req): Json<RecordCandidateRequest>,
) -> Result<Json<RecordCandidateResponse>, HttpServerError> {
    // 更新统计
    {
        let mut stats = state.stats.write().unwrap();
        stats.increment_request("/api/v1/record-candidate");
    }

    // 验证
    validate_project_id(&req.project_id)?;
    validate_candidate(&req.question, &req.answer)?;

    // 检查 memory 服务
    let memory =
        state.services.memory.as_ref().ok_or_else(|| {
            HttpServerError::MemoryService("Memory service not configured".into())
        })?;

    // 构建 payload
    let payload = QACandidatePayload {
        project_id: req.project_id,
        question: req.question,
        answer: req.answer,
        tags: vec![],
        confidence: 0.0,
        metadata: serde_json::Value::Null,
        summary: None,
        source: None,
        author: None,
    };

    // 调用 memory 服务
    match memory.record_candidate(payload).await {
        Ok(_) => Ok(Json(RecordCandidateResponse {
            success: true,
            message: Some("Candidate recorded successfully".into()),
            error: None,
            error_code: None,
        })),
        Err(e) => {
            let mut stats = state.stats.write().unwrap();
            stats.increment_error();
            Err(HttpServerError::MemoryService(e.to_string()))
        }
    }
}

/// POST /api/v1/record-hit - 记录命中
async fn record_hit_handler(
    State(state): State<AppState>,
    Json(req): Json<RecordHitRequest>,
) -> Result<Json<RecordHitResponse>, HttpServerError> {
    // 更新统计
    {
        let mut stats = state.stats.write().unwrap();
        stats.increment_request("/api/v1/record-hit");
    }

    // 验证
    validate_project_id(&req.project_id)?;

    // 检查 memory 服务
    let memory =
        state.services.memory.as_ref().ok_or_else(|| {
            HttpServerError::MemoryService("Memory service not configured".into())
        })?;

    // 构建 references（合并 qa_ids 和 shown_ids）
    let mut references = Vec::new();

    // qa_ids 标记为 used=true
    for qa_id in req.qa_ids {
        references.push(QAReferencePayload {
            qa_id,
            shown: None,
            used: Some(true),
            message_id: None,
            context: None,
        });
    }

    // shown_ids 标记为 shown=true
    if let Some(shown_ids) = req.shown_ids {
        for qa_id in shown_ids {
            // 检查是否已经在 references 中
            if !references.iter().any(|r| r.qa_id == qa_id) {
                references.push(QAReferencePayload {
                    qa_id,
                    shown: Some(true),
                    used: None,
                    message_id: None,
                    context: None,
                });
            }
        }
    }

    // 构建 payload
    let payload = QAHitsPayload {
        project_id: req.project_id,
        references,
    };

    // 调用 memory 服务
    match memory.record_hit(payload).await {
        Ok(_) => Ok(Json(RecordHitResponse {
            success: true,
            data: Some(serde_json::json!({"message": "Hit recorded successfully"})),
            error: None,
            error_code: None,
        })),
        Err(e) => {
            let mut stats = state.stats.write().unwrap();
            stats.increment_error();
            Err(HttpServerError::MemoryService(e.to_string()))
        }
    }
}

/// POST /api/v1/validate - 记录验证
async fn validate_handler(
    State(state): State<AppState>,
    Json(req): Json<ValidateRequest>,
) -> Result<Json<ValidateResponse>, HttpServerError> {
    // 更新统计
    {
        let mut stats = state.stats.write().unwrap();
        stats.increment_request("/api/v1/validate");
    }

    // 验证
    validate_project_id(&req.project_id)?;

    // 验证 result 字段
    if req.result != "success" && req.result != "fail" {
        return Err(HttpServerError::InvalidRequest(
            "result must be 'success' or 'fail'".into(),
        ));
    }

    // 检查 memory 服务
    let memory =
        state.services.memory.as_ref().ok_or_else(|| {
            HttpServerError::MemoryService("Memory service not configured".into())
        })?;

    // 转换 result 和 signal_strength
    let success = req.result == "success";
    let strong_signal = req.signal_strength.as_ref().map(|s| s.as_str() == "strong");

    // 构建 payload
    let payload = QAValidationPayload {
        project_id: req.project_id,
        qa_id: req.qa_id,
        result: Some(req.result),
        signal_strength: req.signal_strength,
        success: Some(success),
        strong_signal,
        source: None,
        context: req.context,
        client: None,
        ts: None,
        payload: req.payload,
    };

    // 调用 memory 服务
    match memory.record_validation(payload).await {
        Ok(_) => Ok(Json(ValidateResponse {
            success: true,
            error: None,
            error_code: None,
        })),
        Err(e) => {
            let mut stats = state.stats.write().unwrap();
            stats.increment_error();
            Err(HttpServerError::MemoryService(e.to_string()))
        }
    }
}

/// GET /health - 健康检查
async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let stats = state.stats.read().unwrap();

    Json(HealthResponse {
        status: "healthy".into(),
        session_id: state.session_id.clone(),
        uptime_seconds: stats.uptime_seconds(),
        requests_handled: stats.requests_total,
        timestamp: Local::now().to_rfc3339(),
    })
}

/// POST /api/v1/shutdown - 触发优雅关闭
async fn shutdown_handler(State(state): State<AppState>) -> Json<serde_json::Value> {
    // 发送关闭信号
    let _ = state.shutdown_tx.send(());

    Json(serde_json::json!({
        "success": true,
        "message": "Shutdown signal sent"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::state::ServerStats;
    use async_trait::async_trait;
    use memex_core::api::{MemoryPlugin, SearchMatch, Services, TaskGradeResult};
    use std::sync::{Arc, RwLock};
    use tokio::sync::broadcast;

    // Mock MemoryPlugin
    struct MockMemoryPlugin {
        should_fail: bool,
    }

    #[async_trait]
    impl MemoryPlugin for MockMemoryPlugin {
        fn name(&self) -> &str {
            "mock"
        }

        async fn search(&self, _payload: QASearchPayload) -> anyhow::Result<Vec<SearchMatch>> {
            if self.should_fail {
                anyhow::bail!("Mock search error")
            } else {
                Ok(vec![SearchMatch {
                    qa_id: "test-qa-id".into(),
                    project_id: Some("test-project".into()),
                    question: "Test question".into(),
                    answer: "Test answer".into(),
                    tags: vec![],
                    score: 0.95,
                    relevance: 0.95,
                    validation_level: 1,
                    level: Some("L1".into()),
                    trust: 0.9,
                    freshness: 1.0,
                    confidence: 0.95,
                    status: "active".into(),
                    summary: None,
                    source: None,
                    expiry_at: None,
                    metadata: serde_json::Value::Null,
                }])
            }
        }

        async fn record_hit(&self, _payload: QAHitsPayload) -> anyhow::Result<()> {
            if self.should_fail {
                anyhow::bail!("Mock record_hit error")
            } else {
                Ok(())
            }
        }

        async fn record_candidate(&self, _payload: QACandidatePayload) -> anyhow::Result<()> {
            if self.should_fail {
                anyhow::bail!("Mock record_candidate error")
            } else {
                Ok(())
            }
        }

        async fn record_validation(&self, _payload: QAValidationPayload) -> anyhow::Result<()> {
            if self.should_fail {
                anyhow::bail!("Mock record_validation error")
            } else {
                Ok(())
            }
        }

        async fn task_grade(&self, _prompt: String) -> anyhow::Result<TaskGradeResult> {
            Ok(TaskGradeResult {
                task_level: "L1".into(),
                reason: "Mock grade".into(),
                recommended_model: "gpt-4".into(),
                recommended_model_provider: Some("openai".into()),
                confidence: 0.9,
            })
        }
    }

    fn create_test_state(with_memory: bool, should_fail: bool) -> AppState {
        let (shutdown_tx, _) = broadcast::channel(1);
        let memory: Option<Arc<dyn MemoryPlugin>> = if with_memory {
            Some(Arc::new(MockMemoryPlugin { should_fail }))
        } else {
            None
        };

        let gatekeeper_config = memex_core::api::GatekeeperConfig::default();
        let services = Services {
            policy: None,
            memory,
            gatekeeper: Arc::new(memex_plugins::gatekeeper::StandardGatekeeperPlugin::new(
                gatekeeper_config,
            )),
        };

        AppState {
            session_id: "test-session".into(),
            services: Arc::new(services),
            stats: Arc::new(RwLock::new(ServerStats::new())),
            shutdown_tx,
        }
    }

    #[tokio::test]
    async fn test_search_handler_success() {
        let state = create_test_state(true, false);
        let req = SearchRequest {
            query: "test query".into(),
            project_id: "test-project".into(),
            limit: 5,
            min_score: 0.6,
        };

        let result = search_handler(State(state.clone()), Json(req)).await;
        assert!(result.is_ok());

        let response = result.unwrap().0;
        assert!(response.success);
        assert!(response.data.is_some());
        assert!(response.error.is_none());

        // 检查统计
        let stats = state.stats.read().unwrap();
        assert_eq!(stats.requests_total, 1);
    }

    #[tokio::test]
    async fn test_search_handler_no_memory() {
        let state = create_test_state(false, false);
        let req = SearchRequest {
            query: "test query".into(),
            project_id: "test-project".into(),
            limit: 5,
            min_score: 0.6,
        };

        let result = search_handler(State(state), Json(req)).await;
        assert!(result.is_err());

        match result {
            Err(HttpServerError::MemoryService(msg)) => {
                assert!(msg.contains("not configured"));
            }
            _ => panic!("Expected MemoryService error"),
        }
    }

    #[tokio::test]
    async fn test_search_handler_invalid_project_id() {
        let state = create_test_state(true, false);
        let req = SearchRequest {
            query: "test query".into(),
            project_id: "invalid@project".into(),
            limit: 5,
            min_score: 0.6,
        };

        let result = search_handler(State(state), Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_record_candidate_handler_success() {
        let state = create_test_state(true, false);
        let req = RecordCandidateRequest {
            project_id: "test-project".into(),
            question: "What is Rust?".into(),
            answer: "Rust is a systems programming language".into(),
        };

        let result = record_candidate_handler(State(state.clone()), Json(req)).await;
        assert!(result.is_ok());

        let response = result.unwrap().0;
        assert!(response.success);
        assert!(response.message.is_some());
    }

    #[tokio::test]
    async fn test_record_candidate_handler_validation_error() {
        let state = create_test_state(true, false);
        let req = RecordCandidateRequest {
            project_id: "test-project".into(),
            question: "Hi".into(), // Too short
            answer: "Rust is a systems programming language".into(),
        };

        let result = record_candidate_handler(State(state), Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_handler_success() {
        let state = create_test_state(true, false);
        let req = ValidateRequest {
            project_id: "test-project".into(),
            qa_id: "test-qa-id".into(),
            result: "success".into(),
            signal_strength: Some("strong".into()),
            context: None,
            payload: None,
        };

        let result = validate_handler(State(state), Json(req)).await;
        assert!(result.is_ok());

        let response = result.unwrap().0;
        assert!(response.success);
    }

    #[tokio::test]
    async fn test_validate_handler_invalid_result() {
        let state = create_test_state(true, false);
        let req = ValidateRequest {
            project_id: "test-project".into(),
            qa_id: "test-qa-id".into(),
            result: "invalid".into(),
            signal_strength: None,
            context: None,
            payload: None,
        };

        let result = validate_handler(State(state), Json(req)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_health_handler() {
        let state = create_test_state(true, false);
        let response = health_handler(State(state.clone())).await;

        assert_eq!(response.0.status, "healthy");
        assert_eq!(response.0.session_id, "test-session");
        assert!(response.0.uptime_seconds >= 0.0);
    }

    #[tokio::test]
    async fn test_shutdown_handler() {
        let state = create_test_state(true, false);
        let mut shutdown_rx = state.shutdown_tx.subscribe();

        let response = shutdown_handler(State(state)).await;
        assert_eq!(response.0["success"], true);

        // 验证关闭信号已发送
        let result = shutdown_rx.try_recv();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_record_hit_handler_success() {
        let state = create_test_state(true, false);
        let req = RecordHitRequest {
            project_id: "test-project".into(),
            qa_ids: vec!["qa1".into(), "qa2".into()],
            shown_ids: Some(vec!["qa3".into()]),
        };

        let result = record_hit_handler(State(state), Json(req)).await;
        assert!(result.is_ok());

        let response = result.unwrap().0;
        assert!(response.success);
    }
}
