# 状态管理集成 - 代码改造示例

本文档提供详细的代码改造前后对比示例，帮助理解具体实施方案。

## 📋 目录

1. [run_app_with_config 改造](#run_app_with_config-改造)
2. [run_session 改造](#run_session-改造)
3. [main.rs 改造](#mainrs-改造)
4. [辅助函数改造](#辅助函数改造)
5. [测试代码示例](#测试代码示例)

---

## run_app_with_config 改造

### 改造前

```rust
pub async fn run_app_with_config(
    args: Args,
    run_args: Option<RunArgs>,
    recover_run_id: Option<String>,
    mut cfg: memex_core::config::AppConfig,
) -> Result<i32, RunnerError> {
    // ... 初始化代码
    
    let run_id = recover_run_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    
    // ... 记忆检索
    let (merged_query, shown_qa_ids, matches, memory_search_event) = 
        build_merged_prompt(...).await;
    
    // ... 启动 session
    let session = runner.start_session(&session_args).await?;
    
    // ... 运行
    let run_result = run_session(...).await?;
    
    // ... Gatekeeper 评估
    let decision = gatekeeper.evaluate(...);
    
    // ... 记忆沉淀
    post_run_memory_reporting(...).await;
    
    Ok(run_outcome.exit_code)
}
```

### 改造后

```rust
use std::sync::Arc;
use memex_core::state::{StateManager, types::RuntimePhase};

pub async fn run_app_with_config(
    args: Args,
    run_args: Option<RunArgs>,
    recover_run_id: Option<String>,
    mut cfg: memex_core::config::AppConfig,
    state_manager: Option<Arc<StateManager>>, // 👈 新增参数
) -> Result<i32, RunnerError> {
    // ========== 状态管理初始化 ==========
    let manager = state_manager.unwrap_or_else(|| Arc::new(StateManager::new()));
    let handle = manager.handle();
    
    // 创建会话
    let run_id = recover_run_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    
    let session_id = handle
        .create_session(Some(run_id.clone()))
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    // [Initializing] 阶段
    handle
        .transition_phase(&session_id, RuntimePhase::Initializing)
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    tracing::debug!(session_id = %session_id, "Session created and initializing");
    
    // ========== 原有初始化逻辑 ==========
    // ... prompt_text 解析等
    
    // ========== 记忆检索阶段 ==========
    // [MemorySearch] 阶段
    handle
        .transition_phase(&session_id, RuntimePhase::MemorySearch)
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    let (merged_query, shown_qa_ids, matches, memory_search_event) = 
        build_merged_prompt(
            memory.as_deref(),
            &cfg.project_id,
            &user_query,
            memory_search_limit,
            memory_min_score,
            &gk_logic_cfg,
            &inject_cfg,
        )
        .await;
    
    // 👈 更新记忆命中数
    manager
        .update_session(&session_id, |session| {
            session.increment_memory_hits(matches.len());
        })
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    tracing::debug!(
        session_id = %session_id, 
        memory_hits = matches.len(), 
        "Memory search completed"
    );
    
    // ========== Runner 启动阶段 ==========
    // [RunnerStarting] 阶段
    handle
        .transition_phase(&session_id, RuntimePhase::RunnerStarting)
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    let session = runner
        .start_session(&session_args)
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    // 👈 记录 Runner PID（如果可用）
    if let Some(pid) = get_session_pid(&session) {
        manager
            .update_session(&session_id, |s| {
                s.set_runner_pid(pid);
            })
            .await
            .map_err(|e| RunnerError::Spawn(e.to_string()))?;
        
        tracing::debug!(session_id = %session_id, pid = pid, "Runner PID recorded");
    }
    
    // ========== Runner 运行阶段 ==========
    // [RunnerRunning] 阶段
    handle
        .transition_phase(&session_id, RuntimePhase::RunnerRunning)
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    // 👈 传入 StateManager 和 session_id
    let run_result = match run_session(
        session,
        &cfg.control,
        policy,
        args.capture_bytes,
        events_out_tx.clone(),
        &run_id,
        stream_plan.silent,
        Some(manager.clone()), // 传入 StateManager
        &session_id,            // 传入 session_id
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            // 👈 失败时更新状态
            let _ = handle.fail(&session_id, e.to_string()).await;
            
            // 仍然发送 wrapper events
            for mut ev in pending_wrapper_events {
                ev.run_id = Some(run_id.clone());
                write_wrapper_event(events_out_tx.as_ref(), &ev).await;
            }
            return Err(e);
        }
    };
    
    // ========== 工具事件处理阶段 ==========
    // [ProcessingToolEvents] 阶段（在 run_session 内部已更新）
    
    let effective_run_id = run_result.run_id.clone();
    let run_outcome: RunOutcome = build_run_outcome(&run_result, shown_qa_ids);
    
    // ========== Gatekeeper 评估阶段 ==========
    // [GatekeeperEvaluating] 阶段
    handle
        .transition_phase(&session_id, RuntimePhase::GatekeeperEvaluating)
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    let decision = gatekeeper.evaluate(
        Utc::now(), 
        &matches, 
        &run_outcome, 
        &run_result.tool_events
    );
    
    // 👈 记录 Gatekeeper 决策
    manager
        .update_session(&session_id, |session| {
            use memex_core::state::types::GatekeeperDecisionSnapshot;
            session.set_gatekeeper_decision(GatekeeperDecisionSnapshot {
                should_write_candidate: decision.should_write_candidate,
                reasons: decision.reasons.clone(),
                signals: decision.signals.clone(),
            });
        })
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    tracing::debug!(
        session_id = %session_id,
        should_write = decision.should_write_candidate,
        "Gatekeeper decision recorded"
    );
    
    // ... decision_event wrapper event
    
    // ========== 记忆沉淀阶段 ==========
    if let Some(mem) = &memory {
        // [MemoryPersisting] 阶段
        handle
            .transition_phase(&session_id, RuntimePhase::MemoryPersisting)
            .await
            .map_err(|e| RunnerError::Spawn(e.to_string()))?;
        
        let tool_events_lite: Vec<ToolEventLite> =
            run_result.tool_events.iter().map(|e| e.into()).collect();

        let candidate_drafts = if decision.should_write_candidate {
            extract_candidates(
                &cand_cfg,
                &user_query,
                &run_outcome.stdout_tail,
                &run_outcome.stderr_tail,
                &tool_events_lite,
            )
        } else {
            vec![]
        };

        post_run_memory_reporting(
            mem.as_ref(), 
            &cfg.project_id, 
            &decision, 
            candidate_drafts
        ).await;
    }
    
    // ========== 完成阶段 ==========
    // [Completed] 阶段
    handle
        .complete(&session_id, run_outcome.exit_code)
        .await
        .map_err(|e| RunnerError::Spawn(e.to_string()))?;
    
    tracing::info!(
        session_id = %session_id,
        exit_code = run_outcome.exit_code,
        "Session completed successfully"
    );
    
    // ... exit_event wrapper event
    
    Ok(run_outcome.exit_code)
}

// 👈 辅助函数：获取 session PID
fn get_session_pid(session: &Box<dyn RunnerSession>) -> Option<u32> {
    // 这需要在 RunnerSession trait 中添加 pid() 方法
    // 或者通过其他方式获取
    None // 临时返回 None
}
```

---

## run_session 改造

### 改造前

```rust
pub async fn run_session(
    mut session: Box<dyn RunnerSession>,
    control: &ControlConfig,
    policy: Option<Box<dyn PolicyPlugin>>,
    capture_bytes: usize,
    events_out: Option<EventsOutTx>,
    run_id: &str,
    silent: bool,
) -> Result<RunnerResult, RunnerError> {
    // ... 设置 stdout/stderr tee
    
    let mut tool_runtime = ToolEventRuntime::new(...);
    
    loop {
        tokio::select! {
            tap = line_rx.recv() => {
                if let Some(tap) = tap {
                    if let Some(ev) = tool_runtime.observe_line(&tap.line).await {
                        // 工具事件处理
                    }
                }
            }
            // ... 其他分支
        }
    }
    
    // ... 返回结果
}
```

### 改造后

```rust
use std::sync::Arc;
use memex_core::state::StateManager;

pub async fn run_session(
    mut session: Box<dyn RunnerSession>,
    control: &ControlConfig,
    policy: Option<Box<dyn PolicyPlugin>>,
    capture_bytes: usize,
    events_out: Option<EventsOutTx>,
    run_id: &str,
    silent: bool,
    state_manager: Option<Arc<StateManager>>, // 👈 新增参数
    session_id: &str,                         // 👈 新增参数
) -> Result<RunnerResult, RunnerError> {
    let _span = tracing::info_span!(
        "core.run_session",
        run_id = %run_id,
        session_id = %session_id, // 👈 添加到 span
        capture_bytes = capture_bytes,
        silent = silent,
        fail_mode = %control.fail_mode,
    );
    let _enter = _span.enter();
    
    // ... 设置 stdout/stderr tee
    
    let mut tool_runtime = ToolEventRuntime::new(...);
    
    // 👈 工具事件计数器
    let mut tool_events_count = 0;
    
    loop {
        tokio::select! {
            tap = line_rx.recv() => {
                if let Some(tap) = tap {
                    if let Some(ev) = tool_runtime.observe_line(&tap.line).await {
                        // 👈 更新工具事件计数
                        tool_events_count += 1;
                        
                        // 👈 更新状态（异步，不阻塞主流程）
                        if let Some(mgr) = &state_manager {
                            let mgr = mgr.clone();
                            let sid = session_id.to_string();
                            tokio::spawn(async move {
                                let _ = mgr.update_session(&sid, |s| {
                                    s.increment_tool_events(1);
                                }).await;
                            });
                        }
                        
                        // 原有 policy 检查逻辑
                        if ev.event_type == "tool.request" {
                            if let Some(p) = &policy {
                                match p.check(&ev).await {
                                    PolicyAction::Deny { reason: r } => {
                                        tracing::error!(
                                            error.kind="policy.deny", 
                                            tool=%ev.tool.as_deref().unwrap_or("?"), 
                                            reason=%r
                                        );
                                        reason = Some(format!("policy denial: {}", r));
                                        break;
                                    }
                                    PolicyAction::Ask { prompt } => {
                                        tracing::warn!(
                                            "Policy requested approval, denying by default"
                                        );
                                        reason = Some(format!("policy requires approval: {}", prompt));
                                        break;
                                    }
                                    PolicyAction::Allow => {}
                                }
                            }
                        }
                    }
                }
            }
            // ... 其他分支
        }
    }
    
    // 👈 在返回前记录最终指标
    if let Some(mgr) = &state_manager {
        let _ = mgr.update_session(session_id, |session| {
            session.update_metrics(|m| {
                m.runner_duration_ms = Some(duration_ms);
            });
        }).await;
    }
    
    // ... 返回结果
}
```

---

## main.rs 改造

### 改造前

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::Args::parse();
    let cfg = load_config()?;
    
    let exit_code = run_app_with_config(
        args,
        run_args,
        None,
        cfg,
    ).await?;
    
    std::process::exit(exit_code);
}
```

### 改造后

```rust
use std::sync::Arc;
use memex_core::state::StateManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::Args::parse();
    let cfg = load_config()?;
    
    // 👈 创建全局状态管理器（可选）
    let state_manager = if std::env::var("MEMEX_ENABLE_STATE_MGMT")
        .unwrap_or_default() == "true" {
        Some(Arc::new(StateManager::new()))
    } else {
        None
    };
    
    // 👈 如果启用，启动事件监听器
    if let Some(ref mgr) = state_manager {
        let mut event_rx = mgr.subscribe();
        tokio::spawn(async move {
            use memex_core::state::StateEvent;
            while let Ok(event) = event_rx.recv().await {
                match event {
                    StateEvent::SessionCreated { session_id, .. } => {
                        tracing::debug!("📦 Session created: {}", session_id);
                    }
                    StateEvent::SessionStateChanged { session_id, new_phase, .. } => {
                        tracing::debug!("🔄 Session {} → {:?}", session_id, new_phase);
                    }
                    StateEvent::SessionCompleted { session_id, exit_code, duration_ms, .. } => {
                        tracing::info!(
                            "✅ Session {} completed (exit={}, {}ms)", 
                            session_id, exit_code, duration_ms
                        );
                    }
                    StateEvent::SessionFailed { session_id, error, .. } => {
                        tracing::error!("❌ Session {} failed: {}", session_id, error);
                    }
                    _ => {}
                }
            }
        });
    }
    
    // 👈 传入状态管理器
    let exit_code = run_app_with_config(
        args,
        run_args,
        None,
        cfg,
        state_manager, // 传入状态管理器
    ).await?;
    
    std::process::exit(exit_code);
}
```

---

## 辅助函数改造

### 新增：RunnerSession trait 扩展（可选）

```rust
// core/src/runner/traits.rs

pub trait RunnerSession: Send {
    fn stdout(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>>;
    fn stderr(&mut self) -> Option<Box<dyn AsyncRead + Unpin + Send>>;
    fn stdin(&mut self) -> Option<Box<dyn AsyncWrite + Unpin + Send>>;
    fn wait(&mut self) -> Pin<Box<dyn Future<Output = Result<i32, RunnerError>> + Send + '_>>;
    
    // 👈 新增方法
    fn pid(&self) -> Option<u32> {
        None // 默认实现返回 None
    }
}
```

### 修改：在 TokioProcessSession 中实现

```rust
// core/src/runner/spawn.rs 或相关实现文件

impl RunnerSession for TokioProcessSession {
    // ... 现有方法实现
    
    // 👈 实现 pid() 方法
    fn pid(&self) -> Option<u32> {
        self.child.id()
    }
}
```

---

## 测试代码示例

### 单元测试

```rust
// cli/tests/state_integration_test.rs

use std::sync::Arc;
use memex_core::state::{StateManager, SessionStatus};
use memex_core::state::types::RuntimePhase;

#[tokio::test]
async fn test_run_app_state_lifecycle() {
    // 初始化测试环境
    let _ = tracing_subscriber::fmt::try_init();
    
    // 创建状态管理器
    let manager = Arc::new(StateManager::new());
    
    // 创建测试配置
    let cfg = create_test_config();
    let args = create_test_args();
    
    // 执行 run_app_with_config
    let result = run_app_with_config(
        args,
        Some(create_test_run_args()),
        None,
        cfg,
        Some(manager.clone()),
    ).await;
    
    // 验证执行成功
    assert!(result.is_ok());
    
    // 验证状态
    let stats = manager.get_session_stats().await;
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.failed, 0);
    
    // 获取最近的会话
    let sessions = manager.get_active_sessions().await;
    // 活跃会话应该为 0（已完成）
    assert_eq!(sessions.len(), 0);
}

#[tokio::test]
async fn test_state_transitions() {
    let manager = StateManager::new();
    let session_id = manager.handle()
        .create_session(Some("test-run".into()))
        .await
        .unwrap();
    
    // 验证初始状态
    let session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(session.status, SessionStatus::Created);
    assert_eq!(session.runtime.phase, RuntimePhase::Idle);
    
    // 执行状态转换
    let phases = vec![
        RuntimePhase::Initializing,
        RuntimePhase::MemorySearch,
        RuntimePhase::RunnerStarting,
        RuntimePhase::RunnerRunning,
        RuntimePhase::ProcessingToolEvents,
        RuntimePhase::GatekeeperEvaluating,
        RuntimePhase::MemoryPersisting,
    ];
    
    for phase in phases {
        manager.handle()
            .transition_phase(&session_id, phase)
            .await
            .unwrap();
        
        let session = manager.get_session(&session_id).await.unwrap();
        assert_eq!(session.runtime.phase, phase);
    }
    
    // 完成会话
    manager.handle()
        .complete(&session_id, 0)
        .await
        .unwrap();
    
    let final_session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(final_session.status, SessionStatus::Completed);
    assert_eq!(final_session.runtime.phase, RuntimePhase::Completed);
}

#[tokio::test]
async fn test_memory_hits_tracking() {
    let manager = StateManager::new();
    let session_id = manager.handle()
        .create_session(None)
        .await
        .unwrap();
    
    // 模拟记忆命中
    manager.update_session(&session_id, |session| {
        session.increment_memory_hits(5);
    }).await.unwrap();
    
    let session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(session.runtime.memory_hits, 5);
    
    // 再次增加
    manager.update_session(&session_id, |session| {
        session.increment_memory_hits(3);
    }).await.unwrap();
    
    let session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(session.runtime.memory_hits, 8);
}

#[tokio::test]
async fn test_tool_events_tracking() {
    let manager = StateManager::new();
    let session_id = manager.handle()
        .create_session(None)
        .await
        .unwrap();
    
    // 模拟工具事件
    for i in 1..=10 {
        manager.update_session(&session_id, |session| {
            session.increment_tool_events(1);
        }).await.unwrap();
    }
    
    let session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(session.runtime.tool_events_count, 10);
}

#[tokio::test]
async fn test_gatekeeper_decision_tracking() {
    use memex_core::state::types::GatekeeperDecisionSnapshot;
    use std::collections::HashMap;
    
    let manager = StateManager::new();
    let session_id = manager.handle()
        .create_session(None)
        .await
        .unwrap();
    
    // 设置 Gatekeeper 决策
    manager.update_session(&session_id, |session| {
        session.set_gatekeeper_decision(GatekeeperDecisionSnapshot {
            should_write_candidate: true,
            reasons: vec!["High quality".into()],
            signals: HashMap::new(),
        });
    }).await.unwrap();
    
    let session = manager.get_session(&session_id).await.unwrap();
    let decision = session.runtime.gatekeeper_decision.as_ref().unwrap();
    assert!(decision.should_write_candidate);
    assert_eq!(decision.reasons.len(), 1);
}
```

### 集成测试

```rust
// cli/tests/full_integration_test.rs

#[tokio::test]
async fn test_full_flow_with_state_management() {
    let _ = tracing_subscriber::fmt::try_init();
    
    // 创建状态管理器和事件订阅器
    let manager = Arc::new(StateManager::new());
    let mut event_rx = manager.subscribe();
    
    // 记录所有事件
    let events = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let events_clone = events.clone();
    
    tokio::spawn(async move {
        use memex_core::state::StateEvent;
        while let Ok(event) = event_rx.recv().await {
            events_clone.lock().await.push(event);
        }
    });
    
    // 执行完整流程
    let cfg = load_test_config();
    let result = run_app_with_config(
        create_test_args(),
        Some(create_test_run_args()),
        None,
        cfg,
        Some(manager.clone()),
    ).await;
    
    assert!(result.is_ok());
    
    // 等待事件处理
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // 验证事件序列
    let recorded_events = events.lock().await;
    assert!(recorded_events.len() > 0);
    
    // 验证包含关键事件
    use memex_core::state::StateEvent;
    let has_created = recorded_events.iter().any(|e| {
        matches!(e, StateEvent::SessionCreated { .. })
    });
    let has_completed = recorded_events.iter().any(|e| {
        matches!(e, StateEvent::SessionCompleted { .. })
    });
    
    assert!(has_created, "Should have SessionCreated event");
    assert!(has_completed, "Should have SessionCompleted event");
    
    // 验证最终状态
    let stats = manager.get_session_stats().await;
    assert_eq!(stats.completed, 1);
}
```

### 性能测试

```rust
// cli/tests/performance_test.rs

use std::time::Instant;

#[tokio::test]
async fn test_state_management_overhead() {
    let manager = Arc::new(StateManager::new());
    
    // 测试 1000 次会话创建和完成的耗时
    let start = Instant::now();
    
    for _ in 0..1000 {
        let session_id = manager.handle()
            .create_session(None)
            .await
            .unwrap();
        
        manager.handle()
            .transition_phase(&session_id, RuntimePhase::Initializing)
            .await
            .unwrap();
        
        manager.handle()
            .complete(&session_id, 0)
            .await
            .unwrap();
    }
    
    let duration = start.elapsed();
    
    println!("1000 sessions in {:?}", duration);
    
    // 确保性能在可接受范围内（< 5 秒）
    assert!(
        duration < Duration::from_secs(5),
        "Performance test failed: took {:?}",
        duration
    );
}

#[tokio::test]
async fn test_concurrent_state_updates() {
    let manager = Arc::new(StateManager::new());
    let session_id = manager.handle()
        .create_session(None)
        .await
        .unwrap();
    
    // 并发更新状态
    let mut handles = vec![];
    
    for _ in 0..100 {
        let mgr = manager.clone();
        let sid = session_id.clone();
        
        let handle = tokio::spawn(async move {
            mgr.update_session(&sid, |session| {
                session.increment_tool_events(1);
            }).await
        });
        
        handles.push(handle);
    }
    
    // 等待所有更新完成
    for h in handles {
        h.await.unwrap().unwrap();
    }
    
    // 验证计数正确
    let session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(session.runtime.tool_events_count, 100);
}
```

---

## 总结

以上代码示例展示了：

1. ✅ **最小侵入式改造**：主要添加参数和状态更新调用
2. ✅ **向后兼容**：`state_manager` 参数可选
3. ✅ **清晰的状态边界**：每个阶段都有明确的转换点
4. ✅ **完整的测试覆盖**：单元测试、集成测试、性能测试
5. ✅ **易于理解和维护**：代码注释清晰，逻辑简单

### 实施建议

1. 先实现 `run_app_with_config` 的改造
2. 再实现 `run_session` 的改造
3. 编写测试验证功能正确性
4. 性能测试确保无明显开销
5. 逐步启用状态管理功能

### 风险控制

- 通过环境变量 `MEMEX_ENABLE_STATE_MGMT` 控制启用
- 状态更新使用 `tokio::spawn` 异步执行，不阻塞主流程
- 所有状态操作都有错误处理，不影响核心功能
