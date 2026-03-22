//! Session Mapper implementation
//!
//! 管理 CLI run_id 与 backend session_id 的映射关系

use std::path::PathBuf;

use tokio::fs;

use super::types::{BackendSession, ExecutionStatus, SessionState};
use crate::error::RunnerError;

/// Resume 检查结果
#[derive(Debug, Clone)]
pub enum ResumeCheck {
    /// 会话不存在
    NotFound,

    /// 多任务会话，不支持 resume
    MultiTaskNotSupported,

    /// 状态不是 interrupted
    NotInterrupted { status: ExecutionStatus },

    /// 可以 resume
    CanResume {
        /// 后端 session_id (用于实际恢复)
        session_id: Option<String>,
        /// 工作目录
        workdir: String,
        /// 后端类型
        backend_kind: String,
        /// 原始 prompt
        original_prompt: Option<String>,
    },
}

/// Session 映射管理器
///
/// 存储路径: ~/.memex/sessions/<run_id>/state.json
#[derive(Debug)]
pub struct SessionMapper {
    base_path: PathBuf,
}

impl SessionMapper {
    /// 创建新的 SessionMapper
    ///
    /// 基础路径默认为 ~/.memex/sessions
    pub fn new() -> Self {
        let base_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".memex")
            .join("sessions");

        Self { base_path }
    }

    /// 使用自定义基础路径 (用于测试)
    #[cfg(test)]
    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    /// 获取状态文件路径
    fn state_path(&self, run_id: &str) -> PathBuf {
        // 清理 run_id 中的路径分隔符，防止路径遍历
        let safe_run_id = run_id.replace(['/', '\\', ':'], "_");
        self.base_path.join(safe_run_id).join("state.json")
    }

    /// 创建新会话状态
    ///
    /// 在执行开始时调用
    pub async fn create(
        &self,
        run_id: &str,
        workdir: &str,
        backend_kind: &str,
        original_prompt: Option<String>,
        is_multi_task: bool,
    ) -> Result<(), RunnerError> {
        let state = SessionState::new(
            run_id.to_string(),
            workdir.to_string(),
            backend_kind.to_string(),
            original_prompt,
            is_multi_task,
        );

        self.save(&state).await?;

        tracing::info!(
            target: "memex.session",
            run_id = %run_id,
            is_multi_task = is_multi_task,
            "session state created"
        );

        Ok(())
    }

    /// 注册后端 session (一对多)
    ///
    /// 当后端返回 session_id 时调用
    /// 同一个 run_id 可以注册多个 session (多任务场景)
    pub async fn register_session(
        &self,
        run_id: &str,
        session_id: &str,
        task_id: &str,
        backend_kind: &str,
    ) -> Result<(), RunnerError> {
        let mut state = self
            .load(run_id)
            .await?
            .ok_or_else(|| RunnerError::SessionNotFound(run_id.to_string()))?;

        // 检查是否已存在该 session_id
        if state.sessions.iter().any(|s| s.session_id == session_id) {
            tracing::debug!(
                target: "memex.session",
                run_id = %run_id,
                session_id = %session_id,
                "session already registered, skipping"
            );
            return Ok(());
        }

        // 添加新的 session
        state.sessions.push(BackendSession::new(
            session_id.to_string(),
            task_id.to_string(),
            backend_kind.to_string(),
        ));

        state.touch();
        self.save(&state).await?;

        tracing::info!(
            target: "memex.session",
            run_id = %run_id,
            session_id = %session_id,
            task_id = %task_id,
            total_sessions = state.sessions.len(),
            "backend session registered"
        );

        Ok(())
    }

    /// 更新执行状态
    ///
    /// 在执行结束或中断时调用
    pub async fn update_status(
        &self,
        run_id: &str,
        status: ExecutionStatus,
        exit_code: Option<i32>,
    ) -> Result<(), RunnerError> {
        let mut state = self
            .load(run_id)
            .await?
            .ok_or_else(|| RunnerError::SessionNotFound(run_id.to_string()))?;

        state.status = status;
        state.exit_code = exit_code;
        state.touch();

        self.save(&state).await?;

        tracing::info!(
            target: "memex.session",
            run_id = %run_id,
            status = %status,
            exit_code = ?exit_code,
            "session status updated"
        );

        Ok(())
    }

    /// 检查是否可以 resume
    ///
    /// 返回 ResumeCheck 枚举，指示 resume 的可行性
    pub async fn check_resume(&self, run_id: &str) -> Result<ResumeCheck, RunnerError> {
        let state = self.load(run_id).await?;

        match state {
            None => Ok(ResumeCheck::NotFound),

            Some(s) if s.is_multi_task => {
                tracing::warn!(
                    target: "memex.session",
                    run_id = %run_id,
                    task_count = s.sessions.len(),
                    "resume not supported for multi-task session"
                );
                Ok(ResumeCheck::MultiTaskNotSupported)
            }

            Some(s) if s.status != ExecutionStatus::Interrupted => {
                tracing::debug!(
                    target: "memex.session",
                    run_id = %run_id,
                    status = %s.status,
                    "session not in interrupted state"
                );
                Ok(ResumeCheck::NotInterrupted { status: s.status })
            }

            Some(s) => {
                tracing::info!(
                    target: "memex.session",
                    run_id = %run_id,
                    session_id = ?s.primary_session_id(),
                    "session can be resumed"
                );
                Ok(ResumeCheck::CanResume {
                    session_id: s.primary_session_id().map(String::from),
                    workdir: s.workdir,
                    backend_kind: s.backend_kind,
                    original_prompt: s.original_prompt,
                })
            }
        }
    }

    /// 加载会话状态
    pub async fn load(&self, run_id: &str) -> Result<Option<SessionState>, RunnerError> {
        let path = self.state_path(run_id);

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .await
            .map_err(|e| RunnerError::Io(format!("Failed to read session state: {}", e)))?;

        let state: SessionState = serde_json::from_str(&content)
            .map_err(|e| RunnerError::Serialization(format!("Invalid session state: {}", e)))?;

        Ok(Some(state))
    }

    /// 保存会话状态
    async fn save(&self, state: &SessionState) -> Result<(), RunnerError> {
        let path = self.state_path(&state.run_id);

        // 确保目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                RunnerError::Io(format!("Failed to create session directory: {}", e))
            })?;
        }

        // 序列化并写入
        let content = serde_json::to_string_pretty(state).map_err(|e| {
            RunnerError::Serialization(format!("Failed to serialize session state: {}", e))
        })?;

        fs::write(&path, content)
            .await
            .map_err(|e| RunnerError::Io(format!("Failed to write session state: {}", e)))?;

        Ok(())
    }

    /// 删除会话状态
    #[allow(dead_code)]
    pub async fn delete(&self, run_id: &str) -> Result<(), RunnerError> {
        let dir = self.base_path.join(run_id.replace(['/', '\\', ':'], "_"));

        if dir.exists() {
            fs::remove_dir_all(&dir)
                .await
                .map_err(|e| RunnerError::Io(format!("Failed to delete session: {}", e)))?;
        }

        Ok(())
    }

    /// 列出所有会话
    #[allow(dead_code)]
    pub async fn list_all(&self) -> Result<Vec<SessionState>, RunnerError> {
        let mut sessions = Vec::new();

        if !self.base_path.exists() {
            return Ok(sessions);
        }

        let mut entries = fs::read_dir(&self.base_path)
            .await
            .map_err(|e| RunnerError::Io(format!("Failed to list sessions: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| RunnerError::Io(format!("Failed to read session entry: {}", e)))?
        {
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(run_id) = entry.file_name().to_str() {
                    if let Ok(Some(state)) = self.load(run_id).await {
                        sessions.push(state);
                    }
                }
            }
        }

        // 按更新时间排序 (最新在前)
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(sessions)
    }
}

impl Default for SessionMapper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_and_load_session() {
        let dir = tempdir().unwrap();
        let mapper = SessionMapper::with_base_path(dir.path().to_path_buf());

        // Create session
        mapper
            .create(
                "run-test-1",
                "/tmp/work",
                "gemini",
                Some("test prompt".into()),
                false,
            )
            .await
            .unwrap();

        // Load and verify
        let state = mapper.load("run-test-1").await.unwrap().unwrap();
        assert_eq!(state.run_id, "run-test-1");
        assert!(!state.is_multi_task);
        assert_eq!(state.status, ExecutionStatus::Running);
        assert!(state.sessions.is_empty());
    }

    #[tokio::test]
    async fn test_register_session() {
        let dir = tempdir().unwrap();
        let mapper = SessionMapper::with_base_path(dir.path().to_path_buf());

        // Create and register
        mapper
            .create("run-test-2", "/tmp/work", "gemini", None, false)
            .await
            .unwrap();

        mapper
            .register_session("run-test-2", "sess-123", "task-1", "gemini")
            .await
            .unwrap();

        // Verify
        let state = mapper.load("run-test-2").await.unwrap().unwrap();
        assert_eq!(state.sessions.len(), 1);
        assert_eq!(state.sessions[0].session_id, "sess-123");
        assert_eq!(state.primary_session_id(), Some("sess-123"));
    }

    #[tokio::test]
    async fn test_multi_session_registration() {
        let dir = tempdir().unwrap();
        let mapper = SessionMapper::with_base_path(dir.path().to_path_buf());

        // Create multi-task session
        mapper
            .create("run-multi", "/tmp/work", "gemini", None, true)
            .await
            .unwrap();

        // Register multiple sessions
        mapper
            .register_session("run-multi", "sess-1", "task-1", "gemini")
            .await
            .unwrap();
        mapper
            .register_session("run-multi", "sess-2", "task-2", "gemini")
            .await
            .unwrap();
        mapper
            .register_session("run-multi", "sess-3", "task-3", "claude")
            .await
            .unwrap();

        // Verify
        let state = mapper.load("run-multi").await.unwrap().unwrap();
        assert_eq!(state.sessions.len(), 3);
        assert!(state.is_multi_task);

        let session_ids = state.all_session_ids();
        assert_eq!(session_ids, vec!["sess-1", "sess-2", "sess-3"]);
    }

    #[tokio::test]
    async fn test_update_status() {
        let dir = tempdir().unwrap();
        let mapper = SessionMapper::with_base_path(dir.path().to_path_buf());

        mapper
            .create("run-test-3", "/tmp/work", "gemini", None, false)
            .await
            .unwrap();

        mapper
            .update_status("run-test-3", ExecutionStatus::Interrupted, Some(130))
            .await
            .unwrap();

        let state = mapper.load("run-test-3").await.unwrap().unwrap();
        assert_eq!(state.status, ExecutionStatus::Interrupted);
        assert_eq!(state.exit_code, Some(130));
    }

    #[tokio::test]
    async fn test_check_resume_not_found() {
        let dir = tempdir().unwrap();
        let mapper = SessionMapper::with_base_path(dir.path().to_path_buf());

        let check = mapper.check_resume("nonexistent").await.unwrap();
        assert!(matches!(check, ResumeCheck::NotFound));
    }

    #[tokio::test]
    async fn test_check_resume_multi_task() {
        let dir = tempdir().unwrap();
        let mapper = SessionMapper::with_base_path(dir.path().to_path_buf());

        mapper
            .create("run-multi", "/tmp/work", "gemini", None, true)
            .await
            .unwrap();

        mapper
            .update_status("run-multi", ExecutionStatus::Interrupted, None)
            .await
            .unwrap();

        let check = mapper.check_resume("run-multi").await.unwrap();
        assert!(matches!(check, ResumeCheck::MultiTaskNotSupported));
    }

    #[tokio::test]
    async fn test_check_resume_not_interrupted() {
        let dir = tempdir().unwrap();
        let mapper = SessionMapper::with_base_path(dir.path().to_path_buf());

        mapper
            .create("run-test", "/tmp/work", "gemini", None, false)
            .await
            .unwrap();

        // Status is Running, not Interrupted
        let check = mapper.check_resume("run-test").await.unwrap();
        assert!(matches!(check, ResumeCheck::NotInterrupted { .. }));
    }

    #[tokio::test]
    async fn test_check_resume_can_resume() {
        let dir = tempdir().unwrap();
        let mapper = SessionMapper::with_base_path(dir.path().to_path_buf());

        mapper
            .create(
                "run-resume",
                "/tmp/work",
                "gemini",
                Some("original prompt".into()),
                false,
            )
            .await
            .unwrap();

        mapper
            .register_session("run-resume", "sess-resume-123", "run-resume", "gemini")
            .await
            .unwrap();

        mapper
            .update_status("run-resume", ExecutionStatus::Interrupted, None)
            .await
            .unwrap();

        let check = mapper.check_resume("run-resume").await.unwrap();
        match check {
            ResumeCheck::CanResume {
                session_id,
                workdir,
                backend_kind,
                original_prompt,
            } => {
                assert_eq!(session_id, Some("sess-resume-123".to_string()));
                assert_eq!(workdir, "/tmp/work");
                assert_eq!(backend_kind, "gemini");
                assert_eq!(original_prompt, Some("original prompt".to_string()));
            }
            _ => panic!("Expected CanResume"),
        }
    }

    #[tokio::test]
    async fn test_duplicate_session_registration() {
        let dir = tempdir().unwrap();
        let mapper = SessionMapper::with_base_path(dir.path().to_path_buf());

        mapper
            .create("run-dup", "/tmp/work", "gemini", None, false)
            .await
            .unwrap();

        // Register same session twice
        mapper
            .register_session("run-dup", "sess-1", "task-1", "gemini")
            .await
            .unwrap();
        mapper
            .register_session("run-dup", "sess-1", "task-1", "gemini")
            .await
            .unwrap();

        let state = mapper.load("run-dup").await.unwrap().unwrap();
        assert_eq!(state.sessions.len(), 1); // Should not duplicate
    }
}
