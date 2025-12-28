# 状态管理系统集成方案

## 📋 目录

1. [改造目标](#改造目标)
2. [现状分析](#现状分析)
3. [架构设计](#架构设计)
4. [集成方案](#集成方案)
5. [改造步骤](#改造步骤)
6. [风险评估](#风险评估)
7. [测试策略](#测试策略)
8. [回滚方案](#回滚方案)

---

## 改造目标

### 核心目标

1. **统一状态管理**：将分散的状态逻辑集中到状态管理器
2. **提升可观测性**：通过事件系统实现全链路追踪
3. **增强可维护性**：清晰的状态生命周期和转换规则
4. **支持故障恢复**：基于快照的状态恢复能力
5. **保持向后兼容**：不破坏现有 API 和功能

### 非目标（后续迭代）

- ❌ 分布式状态同步
- ❌ Web UI 实时监控
- ❌ 状态持久化到数据库
- ❌ 修改现有 WrapperEvent 格式

---

## 现状分析

### 当前代码结构

```
cli/src/app.rs (run_app_with_config)
    ├─ 内存检索 (memory.search)
    ├─ backend plan (factory::build_backend)
    ├─ 启动 session (runner.start_session)
    ├─ 执行 session (run_session)
    ├─ Gatekeeper 评估 (gatekeeper.evaluate)
    └─ 记忆沉淀 (memory reporting)

core/src/runner/run.rs (run_session)
    ├─ 启动 stdout/stderr tee
    ├─ 工具事件解析 (ToolEventRuntime)
    ├─ Policy 检查
    └─ 等待进程退出
```

### 现有状态分散在

| 位置 | 状态内容 | 问题 |
|------|----------|------|
| `run_app_with_config` | run_id, user_query, matches | 分散在局部变量 |
| `run_session` | pending, tool_runtime | 嵌套在函数内 |
| `RunnerResult` | exit_code, stdout_tail, tool_events | 只在结果中体现 |
| `WrapperEvent` | 分散的事件记录 | 无统一状态视图 |

### 痛点识别

1. ❌ **状态不可见**：运行中无法查询当前阶段
2. ❌ **调试困难**：缺少统一的状态追踪
3. ❌ **恢复困难**：失败后无状态快照
4. ❌ **监控缺失**：无法实时监控会话进度
5. ❌ **测试困难**：状态逻辑与业务逻辑耦合

---

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                   StateManager                           │
│  ┌───────────────────────────────────────────────────┐  │
│  │  AppState + Sessions + Event Broadcasting         │  │
│  └───────────────────────────────────────────────────┘  │
└──────────────┬──────────────────────────────────────────┘
               │
               │ integrate
               ▼
┌─────────────────────────────────────────────────────────┐
│              run_app_with_config()                       │
│  ┌─────────────────────────────────────────────────┐   │
│  │  1. 创建 StateManager                            │   │
│  │  2. 创建 Session                                 │   │
│  │  3. 状态转换 (各阶段)                            │   │
│  │  4. 更新状态 (指标、事件)                        │   │
│  │  5. 完成/失败 Session                            │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
               │
               │ calls
               ▼
┌─────────────────────────────────────────────────────────┐
│              run_session()                               │
│  ┌─────────────────────────────────────────────────┐   │
│  │  接收 StateManagerHandle                         │   │
│  │  在关键点更新状态                                 │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### 状态映射

| 原有阶段 | RuntimePhase | 触发点 |
|---------|--------------|--------|
| 初始化 | Initializing | run_app_with_config 开始 |
| 记忆检索 | MemorySearch | memory.search() 调用前 |
| Runner 准备 | RunnerStarting | runner.start_session() 调用前 |
| Runner 运行 | RunnerRunning | run_session() 开始 |
| 工具事件处理 | ProcessingToolEvents | tool_runtime.observe_line() |
| Gatekeeper 评估 | GatekeeperEvaluating | gatekeeper.evaluate() 调用前 |
| 记忆沉淀 | MemoryPersisting | post_run_memory_reporting() 调用前 |
| 完成 | Completed | 正常返回 exit_code |
| 失败 | Failed | 异常捕获 |

---

## 集成方案

### Phase 1: 核心集成（本次实施）

#### 1.1 修改 `run_app_with_config` 函数签名

```rust
pub async fn run_app_with_config(
    args: Args,
    run_args: Option<RunArgs>,
    recover_run_id: Option<String>,
    cfg: memex_core::config::AppConfig,
    state_manager: Option<Arc<StateManager>>, // 新增参数
) -> Result<i32, RunnerError>
```

#### 1.2 在函数开始创建会话

```rust
// 创建或使用传入的 StateManager
let manager = state_manager.unwrap_or_else(|| Arc::new(StateManager::new()));
let handle = manager.handle();

// 创建会话
let session_id = handle.create_session(recover_run_id.clone()).await
    .map_err(|e| RunnerError::Spawn(e.to_string()))?;

// Initializing 阶段
handle.transition_phase(&session_id, RuntimePhase::Initializing).await
    .map_err(|e| RunnerError::Spawn(e.to_string()))?;
```

#### 1.3 状态转换点插入

**记忆检索前**
```rust
handle.transition_phase(&session_id, RuntimePhase::MemorySearch).await?;

// 执行记忆检索
let (merged_query, shown_qa_ids, matches, memory_search_event) = 
    build_merged_prompt(...).await;

// 更新命中数
manager.update_session(&session_id, |session| {
    session.increment_memory_hits(matches.len());
}).await?;
```

**Runner 启动前**
```rust
handle.transition_phase(&session_id, RuntimePhase::RunnerStarting).await?;

let session = runner.start_session(&session_args).await?;

// 记录 PID（如果有）
if let Some(pid) = session.pid() {
    manager.update_session(&session_id, |s| {
        s.set_runner_pid(pid);
    }).await?;
}
```

**Runner 运行中**
```rust
handle.transition_phase(&session_id, RuntimePhase::RunnerRunning).await?;

let run_result = run_session(
    session,
    &cfg.control,
    policy,
    args.capture_bytes,
    events_out_tx.clone(),
    &run_id,
    stream_plan.silent,
    Some(manager.clone()), // 传入 StateManager
    &session_id,            // 传入 session_id
).await?;
```

**工具事件处理**
```rust
// 在 run_session 内部
if let Some(ev) = tool_runtime.observe_line(&tap.line).await {
    // 更新工具事件计数
    if let Some(mgr) = &state_manager {
        let _ = mgr.update_session(session_id, |s| {
            s.increment_tool_events(1);
        }).await;
    }
}
```

**Gatekeeper 评估前**
```rust
handle.transition_phase(&session_id, RuntimePhase::GatekeeperEvaluating).await?;

let decision = gatekeeper.evaluate(...);

// 记录决策
manager.update_session(&session_id, |session| {
    session.set_gatekeeper_decision(GatekeeperDecisionSnapshot {
        should_write_candidate: decision.should_write_candidate,
        reasons: decision.reasons.clone(),
        signals: decision.signals.clone(),
    });
}).await?;
```

**记忆沉淀**
```rust
if let Some(mem) = &memory {
    handle.transition_phase(&session_id, RuntimePhase::MemoryPersisting).await?;
    
    post_run_memory_reporting(...).await;
}
```

**完成会话**
```rust
// 成功
handle.complete(&session_id, run_outcome.exit_code).await?;

// 或失败
handle.fail(&session_id, error.to_string()).await?;
```

#### 1.4 修改 `run_session` 函数签名

```rust
pub async fn run_session(
    mut session: Box<dyn RunnerSession>,
    control: &ControlConfig,
    policy: Option<Box<dyn PolicyPlugin>>,
    capture_bytes: usize,
    events_out: Option<EventsOutTx>,
    run_id: &str,
    silent: bool,
    state_manager: Option<Arc<StateManager>>, // 新增
    session_id: &str,                         // 新增
) -> Result<RunnerResult, RunnerError>
```

#### 1.5 事件订阅（可选，用于日志增强）

```rust
// 在 run_app_with_config 开始时
let mut event_rx = manager.subscribe();
tokio::spawn(async move {
    while let Ok(event) = event_rx.recv().await {
        match event {
            StateEvent::SessionStateChanged { new_phase, .. } => {
                tracing::debug!("State transition: {:?}", new_phase);
            }
            StateEvent::SessionCompleted { exit_code, duration_ms, .. } => {
                tracing::info!("Session completed: exit={}, duration={}ms", 
                              exit_code, duration_ms);
            }
            _ => {}
        }
    }
});
```

### Phase 2: 增强功能（后续迭代）

#### 2.1 快照支持

```rust
// 定期保存快照
let snapshot_manager = SnapshotManager::new("./snapshots", 10)?;
let app_state = manager.get_app_state().await;
let sessions = /* 导出所有会话 */;
let snapshot = StateSnapshot::new(app_state, sessions);
snapshot_manager.save_snapshot(&snapshot)?;
```

#### 2.2 故障恢复

```rust
// 从快照恢复
if let Some(snapshot) = snapshot_manager.load_latest_snapshot()? {
    // 恢复状态
}
```

#### 2.3 性能指标追踪

```rust
// 在各阶段记录耗时
let start = Instant::now();
// ... 执行操作
manager.update_session(&session_id, |session| {
    session.update_metrics(|m| {
        m.memory_search_duration_ms = Some(start.elapsed().as_millis() as u64);
    });
}).await?;
```

#### 2.4 状态查询 API

```rust
// 提供 HTTP API 查询状态
async fn get_session_status(session_id: &str) -> Result<SessionState> {
    manager.get_session(session_id).await
}

async fn list_active_sessions() -> Vec<SessionState> {
    manager.get_active_sessions().await
}
```

---

## 改造步骤

### Step 1: 准备工作 (0.5 天)

- [x] 状态管理模块已实现
- [x] 文档已完成
- [ ] Code Review 通过
- [ ] 创建改造分支 `feature/state-integration`

### Step 2: 核心集成 (2 天)

#### 2.1 修改函数签名 (0.5 天)

- [ ] `run_app_with_config` 添加 `state_manager` 参数
- [ ] `run_session` 添加 `state_manager` 和 `session_id` 参数
- [ ] 更新所有调用点

#### 2.2 会话生命周期集成 (1 天)

- [ ] 在 `run_app_with_config` 开始创建会话
- [ ] 插入所有状态转换点（8 个阶段）
- [ ] 更新会话数据（内存命中、工具事件、决策等）
- [ ] 完成/失败会话

#### 2.3 在 `run_session` 中更新状态 (0.5 天)

- [ ] 工具事件计数更新
- [ ] Runner PID 记录
- [ ] 错误状态更新

### Step 3: 测试验证 (1 天)

#### 3.1 单元测试 (0.5 天)

- [ ] 测试状态转换正确性
- [ ] 测试数据更新正确性
- [ ] 测试错误处理

#### 3.2 集成测试 (0.5 天)

- [ ] 端到端测试完整流程
- [ ] 测试异常场景
- [ ] 性能测试（确保无明显性能下降）

### Step 4: 文档更新 (0.5 天)

- [ ] 更新 API 文档
- [ ] 更新使用示例
- [ ] 更新架构文档

### Step 5: 部署和监控 (0.5 天)

- [ ] 合并到 develop 分支
- [ ] 监控运行状态
- [ ] 收集反馈

**总计：约 4.5 天**

---

## 风险评估

### 高风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| **性能下降** | 高 | 中 | 使用 RwLock 优化并发，异步操作不阻塞主流程 |
| **现有功能破坏** | 高 | 低 | 保持向后兼容，state_manager 参数可选 |
| **死锁风险** | 高 | 低 | 避免嵌套锁，使用超时机制 |

### 中风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| **内存泄漏** | 中 | 中 | 定期清理已完成会话 |
| **状态不一致** | 中 | 低 | 严格的状态转换验证 |
| **测试覆盖不足** | 中 | 中 | 编写完整的单元和集成测试 |

### 低风险

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|----------|
| **日志过多** | 低 | 中 | 可配置的事件日志级别 |
| **调试复杂度** | 低 | 低 | 提供状态查询工具 |

---

## 测试策略

### 单元测试

```rust
#[tokio::test]
async fn test_run_app_state_lifecycle() {
    let manager = StateManager::new();
    
    // 模拟 run_app_with_config
    let session_id = manager.handle().create_session(None).await.unwrap();
    
    // 验证初始状态
    let session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(session.status, SessionStatus::Created);
    
    // 模拟各阶段转换
    manager.handle().transition_phase(&session_id, RuntimePhase::Initializing).await.unwrap();
    manager.handle().transition_phase(&session_id, RuntimePhase::MemorySearch).await.unwrap();
    // ... 其他阶段
    
    // 验证完成
    manager.handle().complete(&session_id, 0).await.unwrap();
    let final_session = manager.get_session(&session_id).await.unwrap();
    assert_eq!(final_session.status, SessionStatus::Completed);
}
```

### 集成测试

```rust
#[tokio::test]
async fn test_full_run_with_state_management() {
    // 创建测试配置
    let cfg = load_test_config();
    let manager = Arc::new(StateManager::new());
    
    // 执行完整流程
    let result = run_app_with_config(
        test_args(),
        Some(test_run_args()),
        None,
        cfg,
        Some(manager.clone()),
    ).await;
    
    assert!(result.is_ok());
    
    // 验证状态
    let stats = manager.get_session_stats().await;
    assert_eq!(stats.completed, 1);
}
```

### 性能测试

```rust
#[tokio::test]
async fn test_state_management_performance() {
    let manager = StateManager::new();
    
    let start = Instant::now();
    for _ in 0..1000 {
        let session_id = manager.handle().create_session(None).await.unwrap();
        manager.handle().complete(&session_id, 0).await.unwrap();
    }
    let duration = start.elapsed();
    
    // 确保 1000 次操作在合理时间内完成
    assert!(duration < Duration::from_secs(5));
}
```

---

## 回滚方案

### 场景 1: 性能问题

**回滚步骤：**
1. 将 `state_manager` 参数设为 `None`
2. 状态管理代码自动禁用
3. 系统恢复到原始行为

**代码示例：**
```rust
// 禁用状态管理
let result = run_app_with_config(
    args,
    run_args,
    recover_run_id,
    cfg,
    None, // 禁用状态管理
).await?;
```

### 场景 2: 功能异常

**回滚步骤：**
1. 回滚到改造前的 commit
2. 使用 `git revert` 撤销改造提交
3. 重新部署

**命令：**
```bash
git revert <integration-commit-hash>
git push origin develop
```

### 场景 3: 内存泄漏

**临时措施：**
1. 减少保留的会话数量
2. 增加清理频率

**代码调整：**
```rust
// 更激进的清理策略
manager.cleanup_completed_sessions(5).await?; // 只保留最近 5 个
```

---

## 兼容性保证

### 向后兼容

```rust
// state_manager 参数可选，默认为 None
pub async fn run_app_with_config(
    // ... 其他参数
    state_manager: Option<Arc<StateManager>>, // 可选参数
) -> Result<i32, RunnerError> {
    // 如果未提供，创建临时的或不使用状态管理
    let manager = state_manager.unwrap_or_else(|| {
        Arc::new(StateManager::new())
    });
    
    // ... 其余逻辑
}
```

### 现有 API 不变

- ✅ `run_app_with_config` 的返回值不变
- ✅ `run_session` 的返回值不变
- ✅ `WrapperEvent` 格式不变
- ✅ 配置文件格式不变

---

## 监控指标

### 关键指标

| 指标 | 目标 | 监控方法 |
|------|------|----------|
| **状态转换耗时** | < 1ms | 状态事件时间戳 |
| **内存占用** | < 10MB per session | 进程监控 |
| **CPU 开销** | < 1% | 进程监控 |
| **事件延迟** | < 10ms | 事件时间戳差 |

### 日志增强

```rust
// 在状态转换时自动记录
tracing::debug!(
    session_id = %session_id,
    old_phase = ?old_phase,
    new_phase = ?new_phase,
    "State transition"
);

// 在完成时记录统计
tracing::info!(
    session_id = %session_id,
    duration_ms = duration_ms,
    tool_events = tool_events_count,
    memory_hits = memory_hits,
    "Session completed"
);
```

---

## 附录

### A. 状态转换流程图

```
run_app_with_config()
    │
    ├─> [Idle]
    │      │
    │      ▼
    ├─> [Initializing]
    │      │ 加载配置、解析参数
    │      ▼
    ├─> [MemorySearch]
    │      │ 记忆检索、上下文注入
    │      ▼
    ├─> [RunnerStarting]
    │      │ 构建 backend plan、启动 session
    │      ▼
    ├─> [RunnerRunning]
    │      │ run_session() 执行
    │      │   ├─ stdout/stderr tee
    │      │   ├─ 工具事件解析
    │      │   └─ Policy 检查
    │      ▼
    ├─> [ProcessingToolEvents]
    │      │ 关联、统计工具事件
    │      ▼
    ├─> [GatekeeperEvaluating]
    │      │ 评估是否写入记忆
    │      ▼
    ├─> [MemoryPersisting]
    │      │ hit/validate/candidate 上报
    │      ▼
    └─> [Completed] or [Failed]
```

### B. 改造清单

#### 需要修改的文件

- [ ] `cli/src/app.rs` - 主要集成点
- [ ] `core/src/runner/run.rs` - 运行时状态更新
- [ ] `cli/src/main.rs` - 传递 StateManager（可选）
- [ ] `core/src/runner/types.rs` - 可能需要添加字段

#### 需要新增的测试

- [ ] `cli/tests/state_integration_test.rs` - 集成测试
- [ ] `core/tests/state_runner_test.rs` - Runner 状态测试

#### 需要更新的文档

- [ ] `docs/ARCHITECTURE.md` - 更新架构说明
- [ ] `docs/data-flow.md` - 更新数据流
- [ ] `README.md` - 更新使用说明

---

## 总结

本方案提供了一个**渐进式、低风险**的状态管理集成策略：

### 优势

✅ **向后兼容**：state_manager 参数可选，现有代码无需修改  
✅ **渐进集成**：分阶段实施，每阶段可独立验证  
✅ **低风险**：出问题可快速回滚  
✅ **高可观测**：自动的状态追踪和事件通知  
✅ **易于测试**：清晰的状态边界便于单元测试  

### 实施建议

1. **先在测试环境**验证完整流程
2. **逐步启用功能**：先日志，后监控，最后快照
3. **密切监控性能**：确保无明显性能下降
4. **收集反馈**：及时调整方案

### 下一步行动

1. ✅ Review 本方案
2. 🔜 创建改造分支
3. 🔜 开始 Step 2 核心集成
4. 🔜 编写测试
5. 🔜 Code Review 和合并

---

**文档版本**: v1.0  
**创建日期**: 2025-12-28  
**作者**: GitHub Copilot  
**状态**: 待 Review
