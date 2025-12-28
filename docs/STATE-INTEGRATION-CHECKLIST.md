# 状态管理集成 - 实施清单

快速参考指南，用于跟踪改造进度。

## 📋 改造前检查清单

- [ ] 已阅读 [集成方案](STATE-INTEGRATION-PLAN.md)
- [ ] 已阅读 [代码示例](STATE-INTEGRATION-CODE-EXAMPLES.md)
- [ ] 已在本地测试现有功能（建立基线）
- [ ] 创建改造分支 `feature/state-integration`
- [ ] 通知团队成员改造计划

## 🔧 代码改造清单

### 1. 核心代码修改

#### cli/src/app.rs

- [ ] 导入状态管理相关类型
  ```rust
  use std::sync::Arc;
  use memex_core::state::{StateManager, types::RuntimePhase};
  ```

- [ ] 修改 `run_app_with_config` 函数签名
  - [ ] 添加 `state_manager: Option<Arc<StateManager>>` 参数

- [ ] 在函数开始创建/获取状态管理器
  ```rust
  let manager = state_manager.unwrap_or_else(|| Arc::new(StateManager::new()));
  let handle = manager.handle();
  ```

- [ ] 创建会话
  ```rust
  let session_id = handle.create_session(Some(run_id.clone())).await?;
  ```

- [ ] 插入状态转换点（8 个）
  - [ ] Initializing - 函数开始后
  - [ ] MemorySearch - 记忆检索前
  - [ ] RunnerStarting - runner.start_session() 前
  - [ ] RunnerRunning - run_session() 前
  - [ ] ProcessingToolEvents - （在 run_session 内部）
  - [ ] GatekeeperEvaluating - gatekeeper.evaluate() 前
  - [ ] MemoryPersisting - post_run_memory_reporting() 前
  - [ ] Completed/Failed - 函数结束时

- [ ] 插入状态更新点
  - [ ] 记忆命中数 - build_merged_prompt() 后
  - [ ] Runner PID - runner.start_session() 后
  - [ ] 工具事件计数 - （在 run_session 内部）
  - [ ] Gatekeeper 决策 - gatekeeper.evaluate() 后

- [ ] 修改错误处理
  - [ ] 捕获错误时调用 `handle.fail()`

#### core/src/runner/run.rs

- [ ] 导入状态管理类型
  ```rust
  use std::sync::Arc;
  use memex_core::state::StateManager;
  ```

- [ ] 修改 `run_session` 函数签名
  - [ ] 添加 `state_manager: Option<Arc<StateManager>>` 参数
  - [ ] 添加 `session_id: &str` 参数

- [ ] 在工具事件处理中更新状态
  ```rust
  if let Some(mgr) = &state_manager {
      tokio::spawn(async move {
          let _ = mgr.update_session(session_id, |s| {
              s.increment_tool_events(1);
          }).await;
      });
  }
  ```

- [ ] 在返回前更新性能指标
  ```rust
  if let Some(mgr) = &state_manager {
      let _ = mgr.update_session(session_id, |session| {
          session.update_metrics(|m| {
              m.runner_duration_ms = Some(duration_ms);
          });
      }).await;
  }
  ```

#### cli/src/main.rs（可选）

- [ ] 创建全局 StateManager
  ```rust
  let state_manager = if std::env::var("MEMEX_ENABLE_STATE_MGMT").ok() == Some("true".to_string()) {
      Some(Arc::new(StateManager::new()))
  } else {
      None
  };
  ```

- [ ] 启动事件监听器（可选）
  ```rust
  if let Some(ref mgr) = state_manager {
      // 启动事件监听任务
  }
  ```

- [ ] 传递 state_manager 到 run_app_with_config

#### core/src/runner/traits.rs（可选）

- [ ] 在 RunnerSession trait 中添加 `pid()` 方法
  ```rust
  fn pid(&self) -> Option<u32> {
      None
  }
  ```

#### core/src/runner/spawn.rs 或实现文件（可选）

- [ ] 在具体实现中实现 `pid()` 方法
  ```rust
  fn pid(&self) -> Option<u32> {
      self.child.id()
  }
  ```

### 2. 更新函数调用

- [ ] 更新所有 `run_app_with_config` 的调用点
  - [ ] `cli/src/main.rs`
  - [ ] 测试文件中的调用

- [ ] 更新所有 `run_session` 的调用点
  - [ ] `cli/src/app.rs`

### 3. 辅助代码

- [ ] 添加 `get_session_pid()` 辅助函数（如果需要）

## 🧪 测试清单

### 单元测试

- [ ] 创建 `cli/tests/state_integration_test.rs`
- [ ] 测试用例：状态生命周期
  ```rust
  #[tokio::test]
  async fn test_run_app_state_lifecycle()
  ```
- [ ] 测试用例：状态转换
  ```rust
  #[tokio::test]
  async fn test_state_transitions()
  ```
- [ ] 测试用例：记忆命中追踪
  ```rust
  #[tokio::test]
  async fn test_memory_hits_tracking()
  ```
- [ ] 测试用例：工具事件追踪
  ```rust
  #[tokio::test]
  async fn test_tool_events_tracking()
  ```
- [ ] 测试用例：Gatekeeper 决策追踪
  ```rust
  #[tokio::test]
  async fn test_gatekeeper_decision_tracking()
  ```

### 集成测试

- [ ] 创建 `cli/tests/full_integration_test.rs`
- [ ] 测试用例：完整流程（启用状态管理）
  ```rust
  #[tokio::test]
  async fn test_full_flow_with_state_management()
  ```
- [ ] 测试用例：完整流程（禁用状态管理）
  ```rust
  #[tokio::test]
  async fn test_full_flow_without_state_management()
  ```
- [ ] 测试用例：错误处理
  ```rust
  #[tokio::test]
  async fn test_error_handling_updates_state()
  ```

### 性能测试

- [ ] 创建 `cli/tests/performance_test.rs`
- [ ] 测试用例：状态管理开销
  ```rust
  #[tokio::test]
  async fn test_state_management_overhead()
  ```
- [ ] 测试用例：并发状态更新
  ```rust
  #[tokio::test]
  async fn test_concurrent_state_updates()
  ```

### 手动测试

- [ ] 运行简单命令测试
  ```bash
  MEMEX_ENABLE_STATE_MGMT=true cargo run -- run --backend codecli --prompt "hello"
  ```
- [ ] 运行复杂场景测试（记忆检索、工具调用等）
- [ ] 测试错误场景（网络错误、超时等）
- [ ] 测试恢复场景（使用 recover_run_id）

### 测试执行

- [ ] 运行所有单元测试
  ```bash
  cargo test --package memex-cli --lib
  cargo test --package memex-core --lib state
  ```
- [ ] 运行所有集成测试
  ```bash
  cargo test --package memex-cli --test state_integration_test
  cargo test --package memex-cli --test full_integration_test
  ```
- [ ] 运行性能测试
  ```bash
  cargo test --package memex-cli --test performance_test -- --nocapture
  ```
- [ ] 检查测试覆盖率（如果有工具）

## 📝 文档更新清单

- [ ] 更新 `docs/ARCHITECTURE.md`
  - [ ] 添加状态管理模块说明
  - [ ] 更新数据流图

- [ ] 更新 `docs/data-flow.md`
  - [ ] 添加状态转换流程
  - [ ] 更新函数调用链

- [ ] 更新 `README.md`
  - [ ] 添加状态管理功能说明
  - [ ] 添加环境变量说明

- [ ] 创建 `docs/STATE-USAGE-GUIDE.md`（可选）
  - [ ] 使用指南
  - [ ] 配置说明
  - [ ] 故障排查

## 🔍 代码审查清单

### 代码质量

- [ ] 所有函数都有适当的错误处理
- [ ] 状态更新不阻塞主流程（使用 tokio::spawn）
- [ ] 避免死锁（不嵌套锁）
- [ ] 内存使用合理（定期清理会话）
- [ ] 代码注释清晰
- [ ] 遵循项目代码风格

### 功能正确性

- [ ] 所有状态转换点都已覆盖
- [ ] 错误场景下状态正确更新
- [ ] 会话 ID 正确传递和使用
- [ ] 事件订阅不影响主流程
- [ ] 向后兼容（state_manager=None 时仍能工作）

### 性能

- [ ] 状态操作不增加明显延迟
- [ ] 内存使用在可接受范围
- [ ] 并发访问安全且高效
- [ ] 事件通道不会溢出

## 🚀 部署清单

### 部署前

- [ ] 所有测试通过
- [ ] Code Review 完成
- [ ] 文档更新完成
- [ ] 性能指标符合预期
- [ ] 创建详细的变更日志

### 部署步骤

- [ ] 合并到 develop 分支
  ```bash
  git checkout develop
  git merge feature/state-integration
  git push origin develop
  ```

- [ ] 创建 PR 到 master（如果需要）

- [ ] 标记版本
  ```bash
  git tag v0.2.0-state-mgmt
  git push --tags
  ```

### 部署后

- [ ] 监控应用运行状态
- [ ] 检查日志输出
- [ ] 验证状态数据正确性
- [ ] 收集性能指标
- [ ] 记录任何问题

## ⚠️ 回滚准备

- [ ] 记录当前版本 commit hash
- [ ] 准备回滚脚本
  ```bash
  git revert <commit-hash>
  ```
- [ ] 测试回滚流程
- [ ] 通知相关人员回滚计划

## 📊 验收标准

### 功能验收

- [ ] ✅ 所有单元测试通过
- [ ] ✅ 所有集成测试通过
- [ ] ✅ 手动测试通过
- [ ] ✅ 状态转换正确
- [ ] ✅ 数据追踪准确
- [ ] ✅ 错误处理完善

### 性能验收

- [ ] ✅ 延迟增加 < 5%
- [ ] ✅ 内存增加 < 10MB per session
- [ ] ✅ CPU 开销 < 1%
- [ ] ✅ 1000 次操作 < 5 秒

### 文档验收

- [ ] ✅ API 文档完整
- [ ] ✅ 架构文档更新
- [ ] ✅ 使用指南清晰
- [ ] ✅ 代码注释充分

## 📅 里程碑

### Phase 1: 准备工作（Day 1, 0.5 天）
- [ ] 完成所有准备工作检查
- [ ] 创建改造分支
- [ ] 通知团队

### Phase 2: 核心集成（Day 1-3, 2 天）
- [ ] 完成代码改造
- [ ] 完成单元测试
- [ ] 初步功能验证

### Phase 3: 测试验证（Day 3-4, 1 天）
- [ ] 完成集成测试
- [ ] 完成性能测试
- [ ] 手动测试验证

### Phase 4: 文档和部署（Day 4-5, 1 天）
- [ ] 文档更新完成
- [ ] Code Review 通过
- [ ] 部署到 develop

### Phase 5: 监控和优化（Day 5+）
- [ ] 监控运行状态
- [ ] 收集反馈
- [ ] 必要的优化

## 🎯 完成标志

当以下所有条件满足时，改造完成：

- ✅ 所有代码改造清单项完成
- ✅ 所有测试清单项通过
- ✅ 所有文档更新完成
- ✅ Code Review 通过
- ✅ 部署成功并稳定运行
- ✅ 性能指标达标
- ✅ 无严重 bug

---

**预计总工时**: 4.5 天  
**实际工时**: _____  
**完成日期**: _____  
**负责人**: _____
