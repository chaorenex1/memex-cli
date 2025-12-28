# 状态管理 - 快速参考

## 🚀 5 分钟快速上手

### 1. 创建管理器

```rust
use memex_core::state::StateManager;

let manager = StateManager::new();
let handle = manager.handle();
```

### 2. 创建会话

```rust
let session_id = handle.create_session(Some("run-123".into())).await?;
```

### 3. 状态转换

```rust
use memex_core::state::types::RuntimePhase;

handle.transition_phase(&session_id, RuntimePhase::Initializing).await?;
handle.transition_phase(&session_id, RuntimePhase::RunnerRunning).await?;
```

### 4. 更新数据

```rust
manager.update_session(&session_id, |session| {
    session.increment_tool_events(5);
    session.increment_memory_hits(3);
}).await?;
```

### 5. 完成会话

```rust
handle.complete(&session_id, 0).await?;
```

## 📋 常用 API 速查

| API | 用途 |
|-----|------|
| `StateManager::new()` | 创建管理器 |
| `manager.handle()` | 获取操作句柄 |
| `handle.create_session(run_id)` | 创建会话 |
| `handle.transition_phase(id, phase)` | 状态转换 |
| `handle.complete(id, exit_code)` | 完成会话 |
| `handle.fail(id, error)` | 失败会话 |
| `manager.get_session(id)` | 获取会话状态 |
| `manager.update_session(id, fn)` | 更新会话 |
| `manager.get_active_sessions()` | 获取活跃会话 |
| `manager.subscribe()` | 订阅事件 |

## 🔄 状态转换路径

```
Idle → Initializing → MemorySearch → RunnerStarting 
  → RunnerRunning → ProcessingToolEvents 
  → GatekeeperEvaluating → MemoryPersisting 
  → Completed

任意状态 → Failed
```

## 📡 事件类型

| 事件 | 触发时机 |
|------|----------|
| `SessionCreated` | 创建会话 |
| `SessionStateChanged` | 阶段转换 |
| `ToolEventReceived` | 接收工具事件 |
| `MemoryHit` | 记忆命中 |
| `GatekeeperDecision` | Gatekeeper 决策 |
| `SessionCompleted` | 会话完成 |
| `SessionFailed` | 会话失败 |

## 🎯 使用模式

### 模式 1: 基本会话

```rust
let manager = StateManager::new();
let session_id = manager.handle()
    .create_session(None).await?;

// ... 执行操作

manager.handle()
    .complete(&session_id, 0).await?;
```

### 模式 2: 事件监听

```rust
let mut rx = manager.subscribe();

tokio::spawn(async move {
    while let Ok(event) = rx.recv().await {
        println!("{:?}", event);
    }
});
```

### 模式 3: 状态查询

```rust
let session = manager.get_session(&session_id).await?;
println!("Phase: {:?}", session.runtime.phase);
println!("Events: {}", session.runtime.tool_events_count);
```

### 模式 4: 批量更新

```rust
manager.update_session(&session_id, |session| {
    session.increment_tool_events(5);
    session.set_runner_pid(12345);
    session.update_metrics(|m| {
        m.startup_duration_ms = Some(100);
    });
}).await?;
```

## 🔧 运行命令

```bash
# 构建
cargo build --package memex-core

# 测试
cargo test --package memex-core --lib state

# 运行示例
cargo run --package memex-core --example state_management

# 生成文档
cargo doc --package memex-core --open
```

## 📖 完整文档

- [完整设计文档](STATE-MANAGEMENT.md)
- [架构图](state-architecture-diagrams.md)
- [使用指南](../core/src/state/README.md)
- [示例程序](../core/examples/state_management.rs)

## ⚡ 性能提示

- ✅ 使用 `RwLock` 支持并发读取
- ✅ 事件广播不阻塞主流程
- ✅ 定期清理已完成会话：`cleanup_completed_sessions(keep_recent)`
- ✅ 限制快照数量防止磁盘占用

## 🐛 常见问题

### Q: 如何处理状态转换失败？

```rust
match handle.transition_phase(&session_id, phase).await {
    Ok(_) => { /* 成功 */ }
    Err(e) => {
        // 记录错误并可选择转换到 Failed
        handle.fail(&session_id, e.to_string()).await?;
    }
}
```

### Q: 如何获取所有会话统计？

```rust
let stats = manager.get_session_stats().await;
println!("Running: {}", stats.running);
println!("Completed: {}", stats.completed);
println!("Failed: {}", stats.failed);
```

### Q: 如何保存和恢复状态？

```rust
use memex_core::state::snapshot::{StateSnapshot, SnapshotManager};

// 保存
let snapshot_mgr = SnapshotManager::new("./snapshots", 10)?;
let snapshot = StateSnapshot::new(app_state, sessions);
snapshot_mgr.save_snapshot(&snapshot)?;

// 恢复
if let Some(snapshot) = snapshot_mgr.load_latest_snapshot()? {
    // 使用 snapshot.app_state 和 snapshot.sessions
}
```

## 📞 获取帮助

- 查看 [设计文档](STATE-MANAGEMENT.md) 了解详细设计
- 运行 [示例程序](../core/examples/state_management.rs) 学习用法
- 查看源码注释获取 API 详情
