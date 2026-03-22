//! Session mapping types

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// 后端会话记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendSession {
    /// 后端返回的 session_id
    pub session_id: String,

    /// 任务 ID
    pub task_id: String,

    /// 后端类型: "gemini", "claude", "codex", "codecli"
    pub backend_kind: String,

    /// 记录时间 (RFC3339 格式)
    pub ts: String,
}

impl BackendSession {
    pub fn new(session_id: String, task_id: String, backend_kind: String) -> Self {
        Self {
            session_id,
            task_id,
            backend_kind,
            ts: Utc::now().to_rfc3339(),
        }
    }
}

/// 执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// 执行中
    Running,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已中断 (可 resume)
    Interrupted,
    /// 已恢复
    Resumed,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Interrupted => write!(f, "interrupted"),
            Self::Resumed => write!(f, "resumed"),
        }
    }
}

/// 会话状态文件 (存储在 ~/.memex/sessions/<run_id>/state.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// CLI run_id
    pub run_id: String,

    /// 是否为多任务执行
    pub is_multi_task: bool,

    /// 后端会话列表 (一对多: 一个 run_id 对应多个 backend session)
    pub sessions: Vec<BackendSession>,

    /// 执行状态
    pub status: ExecutionStatus,

    /// 创建时间 (RFC3339 格式)
    pub created_at: String,

    /// 最后更新时间 (RFC3339 格式)
    pub updated_at: String,

    /// 工作目录
    pub workdir: String,

    /// 后端类型
    pub backend_kind: String,

    /// 原始 prompt (用于 resume)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_prompt: Option<String>,

    /// 退出码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

impl SessionState {
    /// 创建新的会话状态
    pub fn new(
        run_id: String,
        workdir: String,
        backend_kind: String,
        original_prompt: Option<String>,
        is_multi_task: bool,
    ) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            run_id,
            is_multi_task,
            sessions: Vec::new(),
            status: ExecutionStatus::Running,
            created_at: now.clone(),
            updated_at: now,
            workdir,
            backend_kind,
            original_prompt,
            exit_code: None,
        }
    }

    /// 获取第一个后端 session_id (用于单任务 resume)
    pub fn primary_session_id(&self) -> Option<&str> {
        self.sessions.first().map(|s| s.session_id.as_str())
    }

    /// 获取所有 session_id
    pub fn all_session_ids(&self) -> Vec<&str> {
        self.sessions
            .iter()
            .map(|s| s.session_id.as_str())
            .collect()
    }

    /// 获取所有 task_id
    pub fn all_task_ids(&self) -> Vec<&str> {
        self.sessions.iter().map(|s| s.task_id.as_str()).collect()
    }

    /// 更新时间戳
    pub fn touch(&mut self) {
        self.updated_at = Utc::now().to_rfc3339();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_state_serialization() {
        let state = SessionState::new(
            "run-test-123".to_string(),
            "/tmp/test".to_string(),
            "gemini".to_string(),
            Some("test prompt".to_string()),
            false,
        );

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("run-test-123"));

        let decoded: SessionState = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.run_id, "run-test-123");
        assert_eq!(decoded.sessions.len(), 0);
        assert!(!decoded.is_multi_task);
    }

    #[test]
    fn test_backend_session() {
        let session = BackendSession::new(
            "sess-abc".to_string(),
            "task-1".to_string(),
            "gemini".to_string(),
        );

        assert_eq!(session.session_id, "sess-abc");
        assert!(!session.ts.is_empty());
    }
}
