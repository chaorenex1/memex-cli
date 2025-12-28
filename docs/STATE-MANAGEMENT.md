# 状态管理设计文档

## 一、概述

状态管理模块负责管理 memex-cli 运行过程中的全局状态、会话状态和事件状态，提供统一的状态访问和变更接口。

## 二、设计原则

1. **线程安全**：使用 `Arc<RwLock<T>>` 实现多线程共享访问
2. **状态分层**：应用状态、会话状态、运行时状态分离
3. **事件驱动**：状态变更触发事件通知，支持订阅机制
4. **可观测**：所有状态变更都可追踪和审计
5. **故障恢复**：支持状态快照和恢复机制

## 三、架构设计

```
┌─────────────────────────────────────────┐
│          StateManager (状态管理器)         │
├─────────────────────────────────────────┤
│                                         │
│  ┌─────────────┐    ┌──────────────┐   │
│  │  AppState   │    │  Sessions    │   │
│  │  应用状态     │    │  会话集合     │   │
│  └─────────────┘    └──────────────┘   │
│                                         │
│  ┌──────────────────────────────────┐  │
│  │   Event Broadcaster (事件广播)    │  │
│  └──────────────────────────────────┘  │
│                                         │
└─────────────────────────────────────────┘
         │                    │
         ▼                    ▼
┌─────────────────┐   ┌──────────────────┐
│  SessionState   │   │  StateEvent      │
│  会话状态        │   │  状态事件         │
└─────────────────┘   └──────────────────┘
         │
         ▼
┌─────────────────┐
│  RuntimeState   │
│  运行时状态      │
└─────────────────┘
```

## 四、核心类型

### 4.1 AppState（应用状态）

```rust
pub struct AppState {
    pub started_at: DateTime<Utc>,       // 应用启动时间
    pub active_sessions: usize,          // 活跃会话数
    pub completed_sessions: usize,       // 已完成会话数
    pub config_version: String,          // 配置版本
    pub maintenance_mode: bool,          // 维护模式
}
```

### 4.2 SessionState（会话状态）

```rust
pub struct SessionState {
    pub session_id: String,              // 会话唯一 ID
    pub run_id: Option<String>,          // 运行 ID
    pub status: SessionStatus,           // 会话状态
    pub runtime: RuntimeState,           // 运行时状态
    pub created_at: DateTime<Utc>,       // 创建时间
    pub updated_at: DateTime<Utc>,       // 更新时间
    pub completed_at: Option<DateTime<Utc>>, // 完成时间
    pub metadata: HashMap<String, String>,   // 元数据
}
```

### 4.3 RuntimeState（运行时状态）

```rust
pub struct RuntimeState {
    pub run_id: Option<String>,          // 当前运行 ID
    pub runner_pid: Option<u32>,         // Runner 进程 PID
    pub phase: RuntimePhase,             // 当前阶段
    pub tool_events_count: usize,        // 工具事件数量
    pub memory_hits: usize,              // 记忆命中数
    pub gatekeeper_decision: Option<GatekeeperDecisionSnapshot>,
    pub metrics: RuntimeMetrics,         // 性能指标
}
```

### 4.4 RuntimePhase（运行时阶段）

```rust
pub enum RuntimePhase {
    Idle,                    // 空闲
    Initializing,            // 初始化
    MemorySearch,            // 记忆检索中
    RunnerStarting,          // Runner 启动中
    RunnerRunning,           // Runner 运行中
    ProcessingToolEvents,    // 工具事件处理中
    GatekeeperEvaluating,    // Gatekeeper 评估中
    MemoryPersisting,        // 记忆沉淀中
    Completed,               // 完成
    Failed,                  // 失败
}
```

## 五、状态转换

### 5.1 合法转换路径

```
Idle → Initializing → MemorySearch → RunnerStarting 
  → RunnerRunning → ProcessingToolEvents 
  → GatekeeperEvaluating → MemoryPersisting → Completed

任意状态 → Failed（异常情况）
```

### 5.2 转换验证

使用 `StateTransition::validate()` 验证状态转换的合法性：

```rust
StateTransition::validate(from_phase, to_phase)?;
```

## 六、事件系统

### 6.1 事件类型

```rust
pub enum StateEvent {
    AppStarted { timestamp },
    SessionCreated { session_id, timestamp },
    SessionStateChanged { session_id, old_phase, new_phase, timestamp },
    ToolEventReceived { session_id, event_count, timestamp },
    MemoryHit { session_id, hit_count, timestamp },
    GatekeeperDecision { session_id, should_write, timestamp },
    SessionCompleted { session_id, exit_code, duration_ms, timestamp },
    SessionFailed { session_id, error, timestamp },
    AppShutdown { timestamp },
}
```

### 6.2 订阅事件

```rust
let manager = StateManager::new();
let mut event_rx = manager.subscribe();

while let Ok(event) = event_rx.recv().await {
    match event {
        StateEvent::SessionCreated { session_id, .. } => {
            println!("Session {} created", session_id);
        }
        _ => {}
    }
}
```

## 七、快照与恢复

### 7.1 创建快照

```rust
let snapshot_manager = SnapshotManager::new("./snapshots", 10)?;
let snapshot = StateSnapshot::new(app_state, sessions);
snapshot_manager.save_snapshot(&snapshot)?;
```

### 7.2 恢复快照

```rust
if let Some(snapshot) = snapshot_manager.load_latest_snapshot()? {
    // 恢复应用状态
    manager.update_app_state(|state| {
        *state = snapshot.app_state;
    }).await?;
}
```

## 八、使用示例

### 8.1 完整的会话生命周期

```rust
use memex_core::state::{StateManager, RuntimePhase};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 创建状态管理器
    let manager = StateManager::new();
    let handle = manager.handle();

    // 2. 创建会话
    let session_id = handle.create_session(Some("run-123".to_string())).await?;

    // 3. 执行状态转换
    handle.transition_phase(&session_id, RuntimePhase::Initializing).await?;
    handle.transition_phase(&session_id, RuntimePhase::MemorySearch).await?;
    
    // 4. 更新会话状态
    manager.update_session(&session_id, |session| {
        session.increment_tool_events(5);
        session.increment_memory_hits(3);
    }).await?;

    // 5. 继续转换
    handle.transition_phase(&session_id, RuntimePhase::RunnerStarting).await?;
    handle.transition_phase(&session_id, RuntimePhase::RunnerRunning).await?;
    
    // 6. 设置 Runner PID
    manager.update_session(&session_id, |session| {
        session.set_runner_pid(12345);
    }).await?;

    // 7. 处理工具事件
    handle.transition_phase(&session_id, RuntimePhase::ProcessingToolEvents).await?;
    
    // 8. 完成会话
    handle.complete(&session_id, 0).await?;

    Ok(())
}
```

### 8.2 事件监听

```rust
use memex_core::state::{StateManager, StateEvent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let manager = StateManager::new();
    let mut event_rx = manager.subscribe();

    // 在后台任务中监听事件
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match event {
                StateEvent::SessionCreated { session_id, .. } => {
                    println!("✓ Session created: {}", session_id);
                }
                StateEvent::SessionStateChanged { session_id, new_phase, .. } => {
                    println!("→ Session {} transitioned to {:?}", session_id, new_phase);
                }
                StateEvent::SessionCompleted { session_id, exit_code, duration_ms, .. } => {
                    println!("✓ Session {} completed (exit={}, duration={}ms)", 
                             session_id, exit_code, duration_ms);
                }
                _ => {}
            }
        }
    });

    // ... 执行其他操作

    Ok(())
}
```

### 8.3 状态查询

```rust
// 获取应用状态
let app_state = manager.get_app_state().await;
println!("Active sessions: {}", app_state.active_sessions);

// 获取会话状态
let session = manager.get_session(&session_id).await?;
println!("Phase: {:?}", session.runtime.phase);
println!("Tool events: {}", session.runtime.tool_events_count);

// 获取活跃会话列表
let active_sessions = manager.get_active_sessions().await;
for session in active_sessions {
    println!("Active: {} (phase: {:?})", session.session_id, session.runtime.phase);
}

// 获取统计信息
let stats = manager.get_session_stats().await;
println!("Running: {}, Completed: {}, Failed: {}", 
         stats.running, stats.completed, stats.failed);
```

## 九、与现有模块集成

### 9.1 在 Runner 中使用

```rust
// cli/src/commands/run.rs
pub async fn run_cmd(args: RunArgs) -> Result<i32> {
    let manager = StateManager::new();
    let handle = manager.handle();
    
    let session_id = handle.create_session(args.resume_id.clone()).await?;
    
    handle.transition_phase(&session_id, RuntimePhase::Initializing).await?;
    
    // ... 加载配置
    
    handle.transition_phase(&session_id, RuntimePhase::MemorySearch).await?;
    let search_result = memory_client.search(&query).await?;
    
    manager.update_session(&session_id, |session| {
        session.increment_memory_hits(search_result.matches.len());
    }).await?;
    
    handle.transition_phase(&session_id, RuntimePhase::RunnerStarting).await?;
    let child = spawn_child(&args, &cfg)?;
    
    manager.update_session(&session_id, |session| {
        session.set_runner_pid(child.id().unwrap());
    }).await?;
    
    handle.transition_phase(&session_id, RuntimePhase::RunnerRunning).await?;
    let outcome = tee_child_io(child, ...).await?;
    
    handle.complete(&session_id, outcome.exit_code).await?;
    
    Ok(outcome.exit_code)
}
```

## 十、测试

所有模块都包含单元测试和集成测试，可以通过以下命令运行：

```bash
cargo test --package memex_core --lib state
```

## 十一、性能考虑

1. **读写锁**：使用 `RwLock` 允许多个并发读取，减少锁竞争
2. **事件通道**：使用 `broadcast` 通道实现高效的事件分发
3. **快照管理**：自动清理旧快照，限制存储空间使用
4. **会话清理**：提供 API 清理已完成的会话，防止内存泄漏

## 十二、未来扩展

1. **持久化**：支持将状态持久化到数据库
2. **分布式**：支持多实例状态同步
3. **可视化**：提供 Web UI 查看实时状态
4. **告警**：基于状态事件的告警机制
