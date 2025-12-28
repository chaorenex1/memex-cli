# memex_cli 架构重构方案

> **文档日期**: 2025年12月29日  
> **当前分支**: develop  
> **目标版本**: v0.2.0  
> **作者**: Architecture Review

---

## 目录

1. [当前架构问题总结](#1-当前架构问题总结)
2. [重构目标与原则](#2-重构目标与原则)
3. [详细重构方案](#3-详细重构方案)
4. [实施步骤与里程碑](#4-实施步骤与里程碑)
5. [风险评估与缓解](#5-风险评估与缓解)
6. [预期收益](#6-预期收益)

---

## 1. 当前架构问题总结

### 1.1 严重性分级

#### 🔴 P0 - 严重问题（影响安全性和稳定性）

**问题1: TUI流程滥用unsafe和裸指针**

- **位置**: `cli/src/flow/flow_tui.rs:141-174`
- **表现**:
  ```rust
  let tui_ptr = &mut tui as *mut TuiRuntime;
  run_with_query(
      ...,
      Some(tui_ptr),  // 传递裸指针
      |input| async move {
          run_tui_session_continuing(
              unsafe { &mut *tui_ptr },  // unsafe解引用
              ...
          )
      }
  )
  ```
- **危害**:
  - 完全绕过Rust借用检查器
  - 异步上下文中生命周期无保证
  - 可能导致数据竞争和悬垂指针
  - 如果TuiRuntime被提前释放会导致未定义行为
- **影响范围**: 整个TUI流程的稳定性

**问题2: 模块边界混乱**

- **位置**: `cli/src/**/*.rs`
- **表现**: cli层直接依赖core内部模块20+处
  ```rust
  use memex_core::tool_event::ToolEvent;
  use memex_core::state::types::RuntimePhase;
  use memex_core::runner::RunnerResult;
  // ... 大量内部依赖
  ```
- **危害**:
  - 违背分层架构原则
  - core内部重构会破坏cli
  - 模块耦合度极高
  - 测试和维护困难

#### 🟠 P1 - 高优先级问题（影响可维护性）

**问题3: 插件生命周期管理混乱**

- **位置**: `cli/src/app.rs:67` vs `cli/src/flow/flow_tui.rs:147`
- **表现**:
  ```rust
  // app.rs - 外部创建
  let memory = factory::build_memory(&cfg)?;
  let gatekeeper = factory::build_gatekeeper(&cfg);
  
  // flow_tui.rs - 参数被忽略，重复创建
  _gatekeeper: Box<dyn GatekeeperPlugin>,  // 未使用
  let query_gatekeeper = factory::build_gatekeeper(&cfg);
  ```
- **问题**:
  - 插件重复创建，浪费资源
  - 标准流程和TUI流程行为不一致
  - 外部创建的插件被丢弃

**问题4: 过度复杂的参数传递**

- **位置**: `cli/src/flow/flow_qa.rs:41-60`
- **表现**: `run_with_query`接受13个参数
  ```rust
  pub async fn run_with_query<F, Fut>(
      user_query: String,
      args: &Args,
      run_args: Option<&RunArgs>,
      cfg: &mut AppConfig,
      state_manager: Option<Arc<StateManager>>,
      events_out_tx: Option<EventsOutTx>,
      run_id: String,
      recover_run_id: Option<String>,
      should_use_tui: bool,
      stream_enabled: bool,
      stream_format: &str,
      stream_silent: bool,
      policy: Option<Box<dyn PolicyPlugin>>,
      memory: Option<Box<dyn MemoryPlugin>>,
      gatekeeper: Box<dyn GatekeeperPlugin>,
      tui_runtime: Option<*mut tui::TuiRuntime>,  // 裸指针
      run_session_fn: F,
  )
  ```
- **问题**:
  - 接口臃肿，难以维护和测试
  - 包含TUI专用裸指针，破坏通用性
  - 参数职责不清

**问题5: TUI事件循环嵌套混乱**

- **位置**: `cli/src/flow/flow_tui.rs`
- **表现**: 三层嵌套事件循环
  1. `'main_loop` - 输入提示循环 (L74)
  2. `run_tui_session_continuing` - 执行期间事件循环 (L280)
  3. Review阶段内层循环 (L200)
- **问题**:
  - 状态同步困难
  - 控制流复杂
  - 代码高度重复

**问题6: 资源泄漏隐患**

- **位置**: `cli/src/flow/flow_tui.rs:74-76`
- **表现**:
  ```rust
  'main_loop: loop {
      let (input_reader, mut input_rx) = InputReader::start();
      // 多次创建，但未显式停止
      // 只在review阶段停止review_reader
  }
  ```
- **问题**: InputReader后台线程可能泄漏

#### 🟡 P2 - 中优先级问题（设计缺陷）

**问题7: 错误处理分层不合理**

```rust
pub enum CliError {
    Runner(RunnerError),
    Command(String),
    Config(String),  // 重复
}

pub enum RunnerError {
    Config(String),  // 与CliError::Config重复
    Spawn(String),
}
```

- 错误类型职责重叠
- 缺少文档提到的DependencyError层

**问题8: Backend抽象不足**

```rust
pub fn build_backend(backend: &str) -> Box<dyn BackendStrategy> {
    if backend.starts_with("http://") {
        Box::new(AiServiceBackendStrategy)
    } else {
        Box::new(CodeCliBackendStrategy)
    }
}
```

- 通过字符串前缀判断类型
- 添加新backend需修改工厂函数
- 缺少配置化注册机制

**问题9: 状态管理过度设计**

- 引入完整状态系统但实际使用率低
- 核心逻辑未真正依赖状态管理
- `env_state_enabled`开关说明其可选性
- 增加不必要复杂度

**问题10: 插件trait碎片化**

- `MemoryPlugin`, `PolicyPlugin`, `RunnerPlugin`, `GatekeeperPlugin`各自独立
- 缺少统一生命周期管理
- 插件间协作困难
- 无依赖管理机制

---

## 2. 重构目标与原则

### 2.1 核心目标

1. **安全性第一**: 消除所有unsafe代码和裸指针
2. **分层清晰**: 建立明确的模块边界和依赖关系
3. **简化设计**: 减少不必要的抽象和复杂度
4. **可测试性**: 提高单元测试和集成测试覆盖率
5. **可维护性**: 降低代码重复，提高可读性

### 2.2 设计原则

- **依赖倒置**: cli依赖core的抽象接口，不依赖实现细节
- **单一职责**: 每个模块只负责一个明确的功能
- **开闭原则**: 对扩展开放，对修改封闭
- **最小惊讶**: API设计符合直觉，行为可预测
- **渐进式重构**: 保持向后兼容，分步实施

---

## 3. 详细重构方案

### 3.1 Phase 1: 紧急修复 (P0问题)

#### 3.1.1 移除TUI裸指针 (问题1)

**目标**: 用安全的Rust模式替换unsafe裸指针

**方案**: 使用Arc<Mutex<TuiRuntime>>或通道传递事件

**实现A: Arc+Mutex方案**

```rust
// cli/src/flow/flow_tui.rs

pub struct TuiRuntime {
    pub terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    pub app: TuiApp,
}

impl TuiRuntime {
    pub fn shared(cfg: &TuiConfig, run_id: String) -> Result<Arc<Mutex<Self>>, RunnerError> {
        let terminal = setup_terminal().map_err(RunnerError::Spawn)?;
        let app = TuiApp::new(cfg.clone(), run_id);
        Ok(Arc::new(Mutex::new(Self { terminal, app })))
    }
}

pub async fn run_tui_flow(...) -> Result<i32, RunnerError> {
    let tui = TuiRuntime::shared(&cfg.tui, run_id.clone())?;
    
    'main_loop: loop {
        // ...
        
        let tui_clone = Arc::clone(&tui);
        let result = run_with_query(
            user_input,
            // ... 其他参数
            None,  // 移除tui_runtime参数
            |input| async move {
                run_tui_session_with_shared(
                    tui_clone,
                    input,
                    &mut input_rx,
                    &mut tick,
                )
                .await
            },
        )
        .await;
    }
}

async fn run_tui_session_with_shared(
    tui: Arc<Mutex<TuiRuntime>>,
    input: RunSessionInput,
    input_rx: &mut mpsc::UnboundedReceiver<InputEvent>,
    tick: &mut tokio::time::Interval,
) -> Result<RunnerResult, RunnerError> {
    // 安全地访问TUI
    {
        let mut tui_guard = tui.lock().unwrap();
        tui_guard.app.pending_qa = false;
    }
    
    // ... 执行逻辑
}
```

**实现B: 事件通道方案（推荐）**

```rust
// core/src/tui.rs - 移到core中作为公共接口

pub enum TuiEvent {
    PhaseChanged(RuntimePhase),
    ToolEventReceived(ToolEvent),
    MemoryHit { count: usize },
    StatusUpdate(String),
    RunCompleted { exit_code: i32 },
    RunFailed(String),
}

// cli/src/flow/flow_tui.rs

pub async fn run_tui_flow(...) -> Result<i32, RunnerError> {
    let mut tui = TuiRuntime::new(&cfg.tui, run_id.clone())?;
    let (tui_tx, mut tui_rx) = mpsc::unbounded_channel::<TuiEvent>();
    
    'main_loop: loop {
        // ...
        
        let tui_tx_clone = tui_tx.clone();
        let result = run_with_query_v2(  // 新版本API
            user_input,
            // ... 其他参数
            Some(tui_tx_clone),  // 传递发送端
            |input| async move {
                run_session_standard(input).await
            },
        )
        .await;
        
        // 在主线程中处理TUI事件
        while let Ok(event) = tui_rx.try_recv() {
            match event {
                TuiEvent::PhaseChanged(phase) => {
                    tui.app.update_phase(phase);
                }
                TuiEvent::ToolEventReceived(ev) => {
                    tui.app.handle_tool_event(ev);
                }
                // ... 其他事件处理
            }
            
            if let Err(e) = tui.terminal.draw(|f| ui::draw(f, &mut tui.app)) {
                tracing::warn!("TUI render error: {}", e);
            }
        }
    }
}
```

**优势对比**:

| 方案 | 优点 | 缺点 |
|-----|------|------|
| Arc+Mutex | 实现简单，修改量小 | 可能产生锁竞争 |
| 事件通道 | 完全无锁，职责清晰 | 需要重构事件传递 |

**推荐**: **事件通道方案** - 更符合Rust异步编程模式，完全消除共享可变状态

#### 3.1.2 引入Facade层 (问题2)

**目标**: 减少cli对core内部的直接依赖

**方案**: 创建core/facade模块，暴露稳定的高层接口

```rust
// core/src/facade/mod.rs

pub mod types;
pub mod session;
pub mod plugins;

pub use types::*;
pub use session::*;
pub use plugins::*;

// 重新导出稳定的公共类型
pub use crate::error::{CliError, RunnerError};
pub use crate::config::AppConfig;
```

```rust
// core/src/facade/session.rs

use crate::runner::{RunnerSession, RunnerResult, PolicyPlugin};
use crate::events_out::EventsOutTx;
use crate::state::StateManager;
use std::sync::Arc;

/// 运行会话的配置
pub struct SessionConfig {
    pub run_id: String,
    pub capture_bytes: usize,
    pub silent: bool,
    pub control: crate::config::ControlConfig,
}

/// 运行会话的上下文
pub struct SessionContext {
    pub state_manager: Option<Arc<StateManager>>,
    pub events_out: Option<EventsOutTx>,
    pub policy: Option<Box<dyn PolicyPlugin>>,
}

/// 高层会话运行接口
pub async fn run_session(
    session: Box<dyn RunnerSession>,
    config: SessionConfig,
    context: SessionContext,
) -> Result<RunnerResult, crate::error::RunnerError> {
    crate::runner::run_session(
        session,
        &config.control,
        context.policy,
        config.capture_bytes,
        context.events_out,
        None,
        &config.run_id,
        config.silent,
        context.state_manager,
        None,
    ).await
}
```

```rust
// core/src/facade/types.rs

// 重新导出稳定的类型，隐藏内部细节
pub use crate::tool_event::ToolEvent;
pub use crate::memory::{MemoryPlugin, CandidateDraft};
pub use crate::runner::{PolicyPlugin, RunnerPlugin};
pub use crate::gatekeeper::GatekeeperPlugin;

// 为cli层定义专用事件类型
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    Started { run_id: String },
    PhaseChanged { phase: String },
    ToolEventReceived { event_type: String, count: usize },
    Completed { exit_code: i32, duration_ms: u64 },
    Failed { error: String },
}
```

**cli层使用**:

```rust
// cli/src/flow/flow_standard.rs

use memex_core::facade::{
    self,
    SessionConfig,
    SessionContext,
    ExecutionEvent,
};

pub async fn run_standard_flow(...) -> Result<i32, RunnerError> {
    let config = SessionConfig {
        run_id: run_id.clone(),
        capture_bytes: cfg.capture_bytes,
        silent: stream_silent,
        control: cfg.control.clone(),
    };
    
    let context = SessionContext {
        state_manager,
        events_out: events_out_tx,
        policy,
    };
    
    let result = facade::run_session(session, config, context).await?;
    Ok(result.exit_code)
}
```

**收益**:
- cli层只依赖`facade`模块，与core内部解耦
- core内部重构不影响cli
- 接口语义更清晰

### 3.2 Phase 2: 架构优化 (P1问题)

#### 3.2.1 统一插件生命周期管理 (问题3)

**目标**: 确保插件在整个执行流程中一致使用

**方案**: 引入PluginRegistry管理插件生命周期

```rust
// core/src/plugins/registry.rs

use std::sync::Arc;
use crate::memory::MemoryPlugin;
use crate::runner::PolicyPlugin;
use crate::gatekeeper::GatekeeperPlugin;

pub struct PluginRegistry {
    memory: Option<Arc<dyn MemoryPlugin>>,
    policy: Option<Arc<dyn PolicyPlugin>>,
    gatekeeper: Arc<dyn GatekeeperPlugin>,
}

impl PluginRegistry {
    pub fn new(
        memory: Option<Box<dyn MemoryPlugin>>,
        policy: Option<Box<dyn PolicyPlugin>>,
        gatekeeper: Box<dyn GatekeeperPlugin>,
    ) -> Self {
        Self {
            memory: memory.map(|m| Arc::from(m) as Arc<dyn MemoryPlugin>),
            policy: policy.map(|p| Arc::from(p) as Arc<dyn PolicyPlugin>),
            gatekeeper: Arc::from(gatekeeper),
        }
    }
    
    pub fn memory(&self) -> Option<Arc<dyn MemoryPlugin>> {
        self.memory.clone()
    }
    
    pub fn policy(&self) -> Option<Arc<dyn PolicyPlugin>> {
        self.policy.clone()
    }
    
    pub fn gatekeeper(&self) -> Arc<dyn GatekeeperPlugin> {
        Arc::clone(&self.gatekeeper)
    }
}
```

```rust
// core/src/context.rs - 扩展AppContext

pub struct AppContext {
    cfg: AppConfig,
    plugins: PluginRegistry,
    state_manager: Option<Arc<StateManager>>,
    events_out: Option<EventsOutTx>,
}

impl AppContext {
    pub async fn new(
        cfg: AppConfig,
        plugins: PluginRegistry,
        state_manager: Option<Arc<StateManager>>,
    ) -> Result<Self, RunnerError> {
        let events_out = start_events_out(&cfg.events_out)
            .await
            .map_err(RunnerError::Spawn)?;
        Ok(Self {
            cfg,
            plugins,
            state_manager,
            events_out,
        })
    }
    
    pub fn plugins(&self) -> &PluginRegistry {
        &self.plugins
    }
}
```

**使用方式**:

```rust
// cli/src/main.rs

let memory = factory::build_memory(&cfg)?;
let policy = factory::build_policy(&cfg);
let gatekeeper = factory::build_gatekeeper(&cfg);

let plugins = PluginRegistry::new(memory, policy, gatekeeper);
let ctx = AppContext::new(cfg, plugins, state_manager).await?;

// TUI和标准流程都从ctx获取插件
app::run_app_with_config(args, None, None, &ctx).await?;
```

**收益**:
- 插件创建一次，全局共享
- TUI和标准流程行为一致
- 避免重复初始化开销

#### 3.2.2 简化参数传递 (问题4)

**目标**: 减少`run_with_query`的参数数量

**方案**: 使用配置对象封装参数

```rust
// cli/src/flow/types.rs

use memex_core::facade::ExecutionEvent;
use tokio::sync::mpsc;

pub struct ExecutionConfig {
    pub run_id: String,
    pub recover_run_id: Option<String>,
    pub user_query: String,
    pub stream_enabled: bool,
    pub stream_format: String,
    pub stream_silent: bool,
}

pub struct ExecutionContext {
    pub ctx: Arc<AppContext>,
    pub event_tx: Option<mpsc::UnboundedSender<ExecutionEvent>>,
}

// flow/flow_qa.rs

pub async fn execute_query(
    config: ExecutionConfig,
    context: ExecutionContext,
) -> Result<i32, RunnerError> {
    // 简化的实现
}
```

**对比**:

```rust
// 旧版本 - 13个参数
run_with_query(
    user_query,
    args,
    run_args,
    cfg,
    state_manager,
    events_out_tx,
    run_id,
    recover_run_id,
    should_use_tui,
    stream_enabled,
    stream_format,
    stream_silent,
    policy,
    memory,
    gatekeeper,
    tui_runtime,
    run_session_fn,
).await

// 新版本 - 2个参数
execute_query(config, context).await
```

#### 3.2.3 重构TUI事件循环 (问题5)

**目标**: 统一事件处理，消除嵌套循环

**方案**: 单一事件循环 + 状态机

```rust
// cli/src/tui/state_machine.rs

#[derive(Debug, Clone, PartialEq)]
pub enum TuiState {
    Prompting,      // 等待用户输入
    Executing,      // 执行查询中
    Reviewing,      // 显示结果，等待确认
    Exiting,        // 退出中
}

pub struct TuiStateMachine {
    current: TuiState,
    input_buffer: String,
    last_result: Option<Result<i32, String>>,
}

impl TuiStateMachine {
    pub fn handle_event(&mut self, event: TuiInputEvent) -> TuiAction {
        match (&self.current, event) {
            (TuiState::Prompting, TuiInputEvent::Submit(query)) => {
                self.input_buffer = query;
                self.current = TuiState::Executing;
                TuiAction::ExecuteQuery
            }
            (TuiState::Executing, TuiInputEvent::ExecutionComplete(result)) => {
                self.last_result = Some(result);
                self.current = TuiState::Reviewing;
                TuiAction::ShowResult
            }
            (TuiState::Reviewing, TuiInputEvent::Continue) => {
                self.current = TuiState::Prompting;
                TuiAction::PromptAgain
            }
            (_, TuiInputEvent::Exit) => {
                self.current = TuiState::Exiting;
                TuiAction::Exit
            }
            _ => TuiAction::None,
        }
    }
}
```

```rust
// cli/src/flow/flow_tui.rs - 重构后的主循环

pub async fn run_tui_flow(...) -> Result<i32, RunnerError> {
    let mut tui = TuiRuntime::new(&cfg.tui, run_id.clone())?;
    let (input_reader, mut input_rx) = InputReader::start();
    let mut tick = tokio::time::interval(Duration::from_millis(16));
    let mut state_machine = TuiStateMachine::new();
    
    // 单一事件循环
    loop {
        tokio::select! {
            Some(input_event) = input_rx.recv() => {
                let action = state_machine.handle_event(input_event);
                match action {
                    TuiAction::ExecuteQuery => {
                        let query = state_machine.input_buffer.clone();
                        // 启动异步执行，不阻塞事件循环
                        spawn_query_execution(query, &ctx);
                    }
                    TuiAction::Exit => {
                        break;
                    }
                    _ => {}
                }
            }
            _ = tick.tick() => {
                // 定期更新UI
                tui.terminal.draw(|f| ui::draw(f, &mut tui.app))?;
            }
        }
    }
    
    input_reader.stop();
    tui.restore();
    Ok(state_machine.last_exit_code())
}
```

**收益**:
- 单一事件循环，逻辑清晰
- 状态转换显式，易于测试
- 消除代码重复

#### 3.2.4 修复资源泄漏 (问题6)

**目标**: 确保InputReader正确清理

**方案**: 使用RAII模式自动清理

```rust
// cli/src/tui/events.rs

pub struct InputReaderGuard {
    reader: InputReader,
}

impl InputReaderGuard {
    pub fn start() -> (Self, mpsc::UnboundedReceiver<InputEvent>) {
        let (reader, rx) = InputReader::start();
        (Self { reader }, rx)
    }
    
    pub fn receiver(&self) -> &mpsc::UnboundedReceiver<InputEvent> {
        &self.rx
    }
}

impl Drop for InputReaderGuard {
    fn drop(&mut self) {
        self.reader.stop();
        tracing::debug!("InputReader auto-stopped");
    }
}
```

**使用**:

```rust
// cli/src/flow/flow_tui.rs

pub async fn run_tui_flow(...) -> Result<i32, RunnerError> {
    let (_reader_guard, mut input_rx) = InputReaderGuard::start();
    
    // 函数退出时自动调用drop清理
    loop {
        // ...
    }
    
    // 无需显式调用stop
}
```

### 3.3 Phase 3: 设计改进 (P2问题)

#### 3.3.1 重构错误处理 (问题7)

**目标**: 建立清晰的错误分层

```rust
// core/src/error/mod.rs

use thiserror::Error;

// 顶层CLI错误
#[derive(Error, Debug)]
pub enum CliError {
    #[error("runner error: {0}")]
    Runner(#[from] RunnerError),
    
    #[error("command error: {0}")]
    Command(String),
    
    #[error("configuration error: {0}")]
    Config(#[from] ConfigError),
    
    #[error("dependency error: {0}")]
    Dependency(#[from] DependencyError),
    
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

// 配置错误
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("invalid config: {0}")]
    Invalid(String),
    
    #[error("config file not found: {0}")]
    NotFound(String),
    
    #[error("parse error: {0}")]
    Parse(#[from] toml::de::Error),
}

// 运行器错误
#[derive(Error, Debug)]
pub enum RunnerError {
    #[error("spawn failed: {0}")]
    Spawn(String),
    
    #[error("stream io error: {stream}")]
    StreamIo {
        stream: &'static str,
        #[source]
        source: std::io::Error,
    },
    
    #[error("process error: {0}")]
    Process(String),
}

// 依赖错误
#[derive(Error, Debug)]
pub enum DependencyError {
    #[error("memory service error: {0}")]
    Memory(#[from] MemoryError),
    
    #[error("policy error: {0}")]
    Policy(String),
    
    #[error("gatekeeper error: {0}")]
    Gatekeeper(String),
}

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("authentication failed")]
    Auth,
    
    #[error("service unavailable")]
    Unavailable,
}
```

**退出码映射**:

```rust
// core/src/error/exit_code.rs

pub fn exit_code_for_error(e: &CliError) -> i32 {
    match e {
        CliError::Command(_) => 10,
        CliError::Config(ConfigError::Invalid(_)) => 11,
        CliError::Config(ConfigError::NotFound(_)) => 11,
        CliError::Config(ConfigError::Parse(_)) => 11,
        CliError::Runner(RunnerError::Spawn(_)) => 20,
        CliError::Runner(RunnerError::StreamIo { .. }) => 20,
        CliError::Runner(RunnerError::Process(_)) => 20,
        CliError::Dependency(DependencyError::Memory(MemoryError::Network(_))) => 30,
        CliError::Dependency(DependencyError::Memory(MemoryError::Auth)) => 31,
        CliError::Dependency(DependencyError::Policy(_)) => 40,
        CliError::Io(_) => 50,
        _ => 50,
    }
}
```

#### 3.3.2 Backend配置化注册 (问题8)

**目标**: 支持动态注册backend

```rust
// core/src/backend/registry.rs

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub type BackendFactory = Box<dyn Fn() -> Box<dyn BackendStrategy> + Send + Sync>;

pub struct BackendRegistry {
    factories: RwLock<HashMap<String, BackendFactory>>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self {
            factories: RwLock::new(HashMap::new()),
        }
    }
    
    pub fn register(&self, name: &str, factory: BackendFactory) {
        self.factories.write().unwrap().insert(name.to_string(), factory);
    }
    
    pub fn create(&self, name: &str) -> Option<Box<dyn BackendStrategy>> {
        let factories = self.factories.read().unwrap();
        factories.get(name).map(|f| f())
    }
}

lazy_static! {
    static ref BACKEND_REGISTRY: BackendRegistry = {
        let registry = BackendRegistry::new();
        
        // 注册内置backend
        registry.register("codecli", Box::new(|| {
            Box::new(CodeCliBackendStrategy)
        }));
        
        registry.register("aiservice", Box::new(|| {
            Box::new(AiServiceBackendStrategy)
        }));
        
        registry
    };
}

pub fn get_backend(name: &str) -> Result<Box<dyn BackendStrategy>, BackendError> {
    BACKEND_REGISTRY
        .create(name)
        .ok_or_else(|| BackendError::Unknown(name.to_string()))
}
```

**配置文件**:

```toml
# config.toml

[backend]
default = "codecli"

[[backend.providers]]
name = "codecli"
type = "codecli"
enabled = true

[[backend.providers]]
name = "openai"
type = "aiservice"
base_url = "https://api.openai.com"
enabled = true
```

#### 3.3.3 简化状态管理 (问题9)

**方案**: 将StateManager改为可选的observability工具

```rust
// core/src/observability/mod.rs

pub trait ExecutionObserver: Send + Sync {
    fn on_phase_changed(&self, phase: RuntimePhase);
    fn on_tool_event(&self, event: &ToolEvent);
    fn on_completed(&self, exit_code: i32, duration: Duration);
    fn on_failed(&self, error: &str);
}

// 默认实现：日志观察者
pub struct LoggingObserver;

impl ExecutionObserver for LoggingObserver {
    fn on_phase_changed(&self, phase: RuntimePhase) {
        tracing::info!("Phase changed: {:?}", phase);
    }
    // ...
}

// 状态管理观察者（可选）
pub struct StateManagerObserver {
    manager: Arc<StateManager>,
    session_id: String,
}

impl ExecutionObserver for StateManagerObserver {
    fn on_phase_changed(&self, phase: RuntimePhase) {
        let _ = self.manager.handle().transition_phase(&self.session_id, phase);
    }
    // ...
}
```

**使用**:

```rust
let observer: Box<dyn ExecutionObserver> = if state_enabled {
    Box::new(StateManagerObserver::new(state_manager, session_id))
} else {
    Box::new(LoggingObserver)
};

// 在执行过程中调用
observer.on_phase_changed(RuntimePhase::Running);
```

#### 3.3.4 统一插件接口 (问题10)

**目标**: 为所有插件提供统一的生命周期管理

```rust
// core/src/plugin/mod.rs

#[async_trait]
pub trait Plugin: Send + Sync {
    /// 插件名称
    fn name(&self) -> &str;
    
    /// 插件初始化
    async fn initialize(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
    
    /// 插件清理
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
    
    /// 健康检查
    async fn health_check(&self) -> Result<(), PluginError> {
        Ok(())
    }
}

// 特化的插件trait继承基础Plugin
#[async_trait]
pub trait MemoryPlugin: Plugin {
    async fn search(&self, query: &QASearchPayload) -> Result<serde_json::Value, MemoryError>;
    async fn hit(&self, payload: &QAHitsPayload) -> Result<(), MemoryError>;
    async fn candidate(&self, payloads: &[QACandidatePayload]) -> Result<(), MemoryError>;
}

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub async fn initialize_all(&mut self) -> Result<(), PluginError> {
        for plugin in &mut self.plugins {
            plugin.initialize().await?;
        }
        Ok(())
    }
    
    pub async fn shutdown_all(&mut self) -> Result<(), PluginError> {
        for plugin in &mut self.plugins {
            plugin.shutdown().await?;
        }
        Ok(())
    }
}
```

---

## 4. 实施步骤与里程碑

### 4.1 里程碑规划

| 里程碑 | 目标 | 预计工作量 | 完成标准 |
|-------|------|-----------|---------|
| M1 | 移除TUI unsafe代码 | 3天 | ✅ 所有unsafe代码移除<br>✅ TUI测试通过 |
| M2 | 引入Facade层 | 2天 | ✅ cli依赖减少到<5个core模块 |
| M3 | 统一插件管理 | 3天 | ✅ 插件不再重复创建<br>✅ TUI和标准流程一致 |
| M4 | 简化参数传递 | 2天 | ✅ 核心接口参数<5个 |
| M5 | 重构TUI事件循环 | 4天 | ✅ 单一事件循环<br>✅ 状态机测试覆盖 |
| M6 | 修复资源泄漏 | 1天 | ✅ Valgrind/Miri检查通过 |
| M7 | 完善错误处理 | 2天 | ✅ 错误分层清晰<br>✅ 退出码映射正确 |
| M8 | Backend配置化 | 2天 | ✅ 支持动态注册 |
| M9 | 简化状态管理 | 2天 | ✅ 改为可选observability |
| M10 | 统一插件接口 | 3天 | ✅ 统一生命周期管理 |

**总计**: 约24个工作日 (约5周)

### 4.2 详细实施步骤

#### Week 1: 紧急修复 (M1-M2)

**Day 1-3**: 移除TUI unsafe代码
- [ ] 实现事件通道方案
- [ ] 重构`run_tui_flow`使用通道
- [ ] 移除`tui_runtime`裸指针参数
- [ ] 单元测试和集成测试
- [ ] 代码审查

**Day 4-5**: 引入Facade层
- [ ] 创建`core/src/facade`模块
- [ ] 定义稳定的公共接口
- [ ] 重构cli层使用facade
- [ ] 更新文档

#### Week 2: 架构优化 (M3-M4)

**Day 6-8**: 统一插件管理
- [ ] 实现PluginRegistry
- [ ] 扩展AppContext集成插件
- [ ] 重构app.rs和flow模块
- [ ] 验证插件共享正确

**Day 9-10**: 简化参数传递
- [ ] 定义配置对象
- [ ] 重构`execute_query`接口
- [ ] 更新所有调用点

#### Week 3: TUI重构 (M5-M6)

**Day 11-14**: 重构TUI事件循环
- [ ] 实现TuiStateMachine
- [ ] 重构主事件循环
- [ ] 移除嵌套循环
- [ ] 集成测试

**Day 15**: 修复资源泄漏
- [ ] 实现InputReaderGuard
- [ ] 验证资源正确释放
- [ ] Miri检查

#### Week 4-5: 设计改进 (M7-M10)

**Day 16-17**: 完善错误处理
- [ ] 定义新的错误类型层次
- [ ] 实现退出码映射
- [ ] 迁移现有代码

**Day 18-19**: Backend配置化
- [ ] 实现BackendRegistry
- [ ] 配置文件支持
- [ ] 文档更新

**Day 20-21**: 简化状态管理
- [ ] 定义ExecutionObserver接口
- [ ] 实现默认和可选观察者
- [ ] 重构状态管理为可选

**Day 22-24**: 统一插件接口
- [ ] 定义Plugin基础trait
- [ ] 实现PluginManager
- [ ] 迁移现有插件

### 4.3 验收标准

每个里程碑需满足:

1. **代码质量**:
   - 所有新代码通过`cargo clippy`
   - 无unsafe代码（除非有充分理由）
   - 测试覆盖率>80%

2. **功能验证**:
   - 所有现有测试通过
   - 新增测试覆盖关键路径
   - 手动测试TUI和标准流程

3. **文档更新**:
   - API文档完整
   - ARCHITECTURE.md更新
   - CHANGELOG.md记录

4. **性能基准**:
   - 不应有明显性能退化
   - TUI响应延迟<100ms

---

## 5. 风险评估与缓解

### 5.1 技术风险

| 风险 | 影响 | 概率 | 缓解措施 |
|-----|------|------|---------|
| TUI重构破坏现有功能 | 高 | 中 | • 充分的集成测试<br>• 渐进式迁移<br>• 保留旧代码作为fallback |
| 插件共享导致竞态 | 高 | 低 | • 使用Arc保证线程安全<br>• 插件内部处理并发 |
| Facade抽象不足 | 中 | 中 | • 迭代式设计<br>• 预留扩展点 |
| 性能退化 | 中 | 低 | • 性能基准测试<br>• 避免不必要的克隆 |
| 测试覆盖不足 | 高 | 中 | • TDD开发模式<br>• 集成测试自动化 |

### 5.2 项目风险

| 风险 | 影响 | 概率 | 缓解措施 |
|-----|------|------|---------|
| 工期延误 | 中 | 中 | • 按优先级分phase<br>• P0问题优先完成 |
| 向后兼容性破坏 | 高 | 低 | • 保持配置文件兼容<br>• 提供迁移指南 |
| 团队熟悉度不足 | 低 | 中 | • 代码审查<br>• 技术分享会 |

### 5.3 回滚策略

每个里程碑完成后:
1. 创建git tag (如`v0.2.0-m1`)
2. 保留feature branch
3. 如果发现重大问题，可回滚到上一个稳定版本

---

## 6. 预期收益

### 6.1 安全性提升

- ✅ **消除所有unsafe代码**: 零未定义行为风险
- ✅ **类型安全**: 编译期捕获更多错误
- ✅ **并发安全**: Arc+事件通道避免数据竞争

### 6.2 可维护性提升

- ✅ **代码行数减少**: 预计减少15-20%重复代码
- ✅ **模块耦合度降低**: cli/core依赖明确
- ✅ **测试覆盖率提升**: 从当前~60%提升到>80%

### 6.3 可扩展性提升

- ✅ **插件机制完善**: 统一生命周期管理
- ✅ **Backend可配置**: 支持动态注册
- ✅ **观察者模式**: 灵活的observability

### 6.4 性能优化

- ✅ **插件复用**: 避免重复初始化开销
- ✅ **资源管理**: 无泄漏，内存占用稳定
- ✅ **异步优化**: 事件通道无锁设计

### 6.5 用户体验

- ✅ **TUI稳定性**: 无crash，响应流畅
- ✅ **错误信息清晰**: 分层错误提示
- ✅ **一致性**: TUI/CLI行为一致

---

## 附录

### A. 参考文档

- [ARCHITECTURE.md](ARCHITECTURE.md) - 当前架构文档
- [tui-refactor-plan.md](tui-refactor-plan.md) - TUI专项重构
- [STATE-MANAGEMENT.md](STATE-MANAGEMENT.md) - 状态管理设计

### B. 相关Issue

- #1 - TUI unsafe代码安全问题
- #2 - 插件重复创建
- #3 - 资源泄漏排查

### C. 技术选型理由

**为何选择事件通道而非Arc<Mutex>?**

- 事件通道完全无锁，避免死锁风险
- 符合Rust异步编程最佳实践
- 职责分离清晰（TUI渲染 vs 业务逻辑）
- 易于测试和mock

**为何引入Facade层而非直接依赖?**

- 降低模块耦合，提高内聚性
- core重构不影响cli稳定性
- 提供稳定的API契约
- 便于版本演进

**为何简化状态管理?**

- 当前使用率低，复杂度高
- 改为可选的observability更灵活
- 符合YAGNI原则（You Aren't Gonna Need It）
- 降低学习和维护成本

---

**文档版本**: v1.0  
**最后更新**: 2025年12月29日  
**审核状态**: 待审核
