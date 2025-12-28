# 状态管理模块

## 概述

状态管理模块为 memex-cli 提供统一的状态跟踪、管理和事件通知机制。支持会话生命周期管理、状态转换验证、事件订阅和状态快照恢复。

## 特性

✅ **线程安全**：基于 `Arc<RwLock<T>>` 实现多线程共享访问  
✅ **状态分层**：应用状态、会话状态、运行时状态分离  
✅ **事件驱动**：状态变更自动触发事件通知  
✅ **状态转换验证**：严格的状态机模型，防止非法转换  
✅ **可观测性**：所有状态变更可追踪和审计  
✅ **快照与恢复**：支持状态持久化和故障恢复  
✅ **完整测试**：13 个单元测试，覆盖所有核心功能  

## 快速开始

### 1. 创建状态管理器

```rust
use memex_core::state::StateManager;

let manager = StateManager::new();
let handle = manager.handle();
```

### 2. 创建会话

```rust
let session_id = handle.create_session(Some("run-123".to_string())).await?;
```

### 3. 状态转换

```rust
use memex_core::state::types::RuntimePhase;

handle.transition_phase(&session_id, RuntimePhase::Initializing).await?;
handle.transition_phase(&session_id, RuntimePhase::MemorySearch).await?;
handle.transition_phase(&session_id, RuntimePhase::RunnerRunning).await?;
```

### 4. 更新会话数据

```rust
manager.update_session(&session_id, |session| {
    session.increment_tool_events(5);
    session.increment_memory_hits(3);
    session.set_runner_pid(12345);
}).await?;
```

### 5. 订阅事件

```rust
use memex_core::state::StateEvent;

let mut event_rx = manager.subscribe();

tokio::spawn(async move {
    while let Ok(event) = event_rx.recv().await {
        match event {
            StateEvent::SessionCreated { session_id, .. } => {
                println!("Session {} created", session_id);
            }
            StateEvent::SessionCompleted { session_id, exit_code, .. } => {
                println!("Session {} completed with exit code {}", session_id, exit_code);
            }
            _ => {}
        }
    }
});
```

### 6. 完成会话

```rust
handle.complete(&session_id, 0).await?;
```

## 状态生命周期

```
Idle → Initializing → MemorySearch → RunnerStarting 
  → RunnerRunning → ProcessingToolEvents 
  → GatekeeperEvaluating → MemoryPersisting 
  → Completed
```

任意阶段都可以转换到 `Failed` 状态。

## 运行示例

```bash
cargo run --package memex-core --example state_management
```

示例输出：

```
📡 Event listener started

🚀 Starting memex-cli session

[Phase 1] Initializing...
✓ Session created: d9d24e2c-8818-4f29-b771-a0606b3a7213
→ Session d9d24e2c-8818-4f29-b771-a0606b3a7213 → Initializing
[Phase 2] Memory search...
→ Session d9d24e2c-8818-4f29-b771-a0606b3a7213 → MemorySearch
...
✓ Session completed (exit=0, duration=2288ms)

📊 Final Statistics:
   Active sessions: 0
   Completed sessions: 1
```

## 运行测试

```bash
# 运行所有状态管理测试
cargo test --package memex-core --lib state

# 运行特定测试
cargo test --package memex-core --lib state::manager::tests
cargo test --package memex-core --lib state::session::tests
cargo test --package memex-core --lib state::transitions::tests
cargo test --package memex-core --lib state::snapshot::tests
```

测试结果：

```
running 13 tests
test state::session::tests::test_session_creation ... ok
test state::session::tests::test_session_transition ... ok
test state::session::tests::test_tool_events_increment ... ok
test state::manager::tests::test_state_manager_creation ... ok
test state::manager::tests::test_session_lifecycle ... ok
test state::manager::tests::test_event_subscription ... ok
test state::transitions::tests::test_valid_transitions ... ok
test state::transitions::tests::test_invalid_transitions ... ok
test state::transitions::tests::test_terminal_states ... ok
test state::transitions::tests::test_next_phase ... ok
test state::snapshot::tests::test_snapshot_serialization ... ok
test state::snapshot::tests::test_snapshot_manager ... ok
test state::snapshot::tests::test_snapshot_cleanup ... ok

test result: ok. 13 passed; 0 failed; 0 ignored
```

## 模块结构

```
state/
├── mod.rs           # 模块导出
├── types.rs         # 状态类型定义（AppState, RuntimeState, RuntimePhase 等）
├── session.rs       # 会话状态管理（SessionState, SessionStatus）
├── manager.rs       # 状态管理器（StateManager, 事件系统）
├── transitions.rs   # 状态转换验证
└── snapshot.rs      # 状态快照和恢复
```

## 主要类型

### StateManager
核心状态管理器，提供：
- 会话创建和管理
- 状态转换
- 事件广播
- 统计查询

### SessionState
单个会话的完整状态，包含：
- 会话 ID 和运行 ID
- 会话状态（Created, Running, Completed, Failed）
- 运行时状态（阶段、指标等）
- 时间戳和元数据

### RuntimePhase
会话执行的各个阶段：
- `Idle` - 空闲
- `Initializing` - 初始化
- `MemorySearch` - 记忆检索
- `RunnerStarting` - 启动 Runner
- `RunnerRunning` - Runner 运行中
- `ProcessingToolEvents` - 处理工具事件
- `GatekeeperEvaluating` - Gatekeeper 评估
- `MemoryPersisting` - 记忆沉淀
- `Completed` / `Failed` - 终态

### StateEvent
状态变更事件：
- `SessionCreated` - 会话创建
- `SessionStateChanged` - 阶段转换
- `ToolEventReceived` - 接收工具事件
- `MemoryHit` - 记忆命中
- `GatekeeperDecision` - Gatekeeper 决策
- `SessionCompleted` / `SessionFailed` - 会话完成/失败

## 快照功能

### 保存快照

```rust
use memex_core::state::snapshot::SnapshotManager;

let snapshot_manager = SnapshotManager::new("./snapshots", 10)?;

// 创建并保存快照
let app_state = manager.get_app_state().await;
let sessions = /* 获取所有会话 */;
let snapshot = StateSnapshot::new(app_state, sessions);

snapshot_manager.save_snapshot(&snapshot)?;
```

### 恢复快照

```rust
if let Some(snapshot) = snapshot_manager.load_latest_snapshot()? {
    // 恢复状态
    manager.update_app_state(|state| {
        *state = snapshot.app_state;
    }).await?;
}
```

## 性能考虑

- **读写锁**：使用 `RwLock` 允许多个并发读取
- **事件通道**：使用 `broadcast` 通道实现高效事件分发（容量 1000）
- **快照管理**：自动清理旧快照，限制存储空间
- **会话清理**：提供 `cleanup_completed_sessions()` API

## 集成到现有代码

### 在 run_cmd 中使用

```rust
pub async fn run_cmd(args: RunArgs) -> Result<i32> {
    let manager = StateManager::new();
    let handle = manager.handle();
    
    // 创建会话
    let session_id = handle.create_session(args.resume_id.clone()).await?;
    
    // 初始化
    handle.transition_phase(&session_id, RuntimePhase::Initializing).await?;
    
    // 记忆检索
    handle.transition_phase(&session_id, RuntimePhase::MemorySearch).await?;
    let search_result = memory_client.search(&query).await?;
    
    manager.update_session(&session_id, |session| {
        session.increment_memory_hits(search_result.matches.len());
    }).await?;
    
    // 启动 Runner
    handle.transition_phase(&session_id, RuntimePhase::RunnerStarting).await?;
    let child = spawn_child(&args, &cfg)?;
    
    manager.update_session(&session_id, |session| {
        session.set_runner_pid(child.id().unwrap());
    }).await?;
    
    // 运行
    handle.transition_phase(&session_id, RuntimePhase::RunnerRunning).await?;
    let outcome = tee_child_io(child, ...).await?;
    
    // 完成
    handle.complete(&session_id, outcome.exit_code).await?;
    
    Ok(outcome.exit_code)
}
```

## 文档

详细设计文档请参见：[docs/STATE-MANAGEMENT.md](../../docs/STATE-MANAGEMENT.md)

## 许可证

Apache-2.0
