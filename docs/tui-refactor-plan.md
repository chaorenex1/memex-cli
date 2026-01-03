# TUI 流程架构问题分析与重构方案

> **文档日期**: 2025年12月29日  
> **当前分支**: develop  
> **分析范围**: `cli/src/flow/flow_tui.rs` 及相关 TUI 模块

---

## 目录

- [一、当前架构问题分析](#一当前架构问题分析)
- [二、重构目标](#二重构目标)
- [三、新架构设计](#三新架构设计)
- [四、通信机制设计](#四通信机制设计)
- [五、重构实施步骤](#五重构实施步骤)
- [六、风险评估与缓解](#六风险评估与缓解)
- [七、预期收益](#七预期收益)
- [八、测试策略](#八测试策略)

---

## 一、当前架构问题分析

### 🔴 1. 严重的安全问题：滥用裸指针和 unsafe

**问题代码位置**: `cli/src/flow/flow_tui.rs:141-174`

```rust
let tui_ptr = &mut tui as *mut TuiRuntime;
// ...
run_with_query(
    // ...
    Some(tui_ptr),  // 传递裸指针
    |input| async move {
        run_tui_session_continuing(
            unsafe { &mut *tui_ptr },  // unsafe 解引用
            // ...
        )
    }
)
```

**危害**:
- ❌ 绕过 Rust 的借用检查器，引入数据竞争风险
- ❌ 裸指针在异步上下文中传递，生命周期无法保证
- ❌ 如果 `tui` 被提前释放，会导致**悬垂指针**和未定义行为
- ❌ 违反 Rust 的核心安全承诺

### 🟠 2. 资源泄漏：InputReader 未正确清理

**问题代码位置**: `cli/src/flow/flow_tui.rs:74-76`

```rust
'main_loop: loop {
    let (input_reader, mut input_rx) = InputReader::start();
    // ...循环多次创建 InputReader
    // 但只在 review 阶段调用 review_reader.stop()
    // 第一个 input_reader 从未被停止
}
```

**问题**:
- 每次循环创建新的 `InputReader`，但未显式停止
- 可能导致后台线程泄漏
- 多个输入读取器同时运行可能干扰

### 🟠 3. 状态管理混乱：重复的事件循环

整个 TUI 流程有**三层嵌套的事件循环**:

1. **`run_tui_flow` 的 `'main_loop`** (输入提示循环) - 第74行
2. **`run_tui_session_continuing`** (执行期间事件循环) - 第280行
3. **review 阶段的内层循环** (等待用户决策) - 第200行

**问题**:
- 事件循环职责不清，代码高度重复
- `tick.tick()` 在不同阶段的语义不同
- `input_rx` 被多处共享和修改，状态难以跟踪
- 循环嵌套导致控制流复杂，难以理解和维护

### 🟡 4. 不一致的插件生命周期管理

**问题代码位置**: `cli/src/flow/flow_tui.rs:147-151`

```rust
// Rebuild plugins for each query
let query_memory = factory::build_memory(&cfg)?;
let query_policy = factory::build_policy(&cfg);
let query_gatekeeper = factory::build_gatekeeper(&cfg);
```

**与 app.rs 的冲突**:
```rust
// app.rs:67-69 - 外部已经创建插件
let memory = factory::build_memory(&cfg)?;
let policy = factory::build_policy(&cfg);
let gatekeeper = factory::build_gatekeeper(&cfg);

// 但在 flow_tui.rs:59 - gatekeeper 参数被忽略
_gatekeeper: Box<dyn memex_core::api::GatekeeperPlugin>,
```

**问题**:
- 每次查询都重建插件，但外部已经创建
- `_gatekeeper` 参数前缀下划线表示未使用，造成资源浪费
- 插件初始化开销重复执行

### 🟡 5. 异常的参数传递链

**问题接口**: `cli/src/flow/flow_qa.rs`

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
    gatekeeper: Box<dyn memex_core::api::GatekeeperPlugin>,
    tui_runtime: Option<*mut tui::TuiRuntime>,  // ❌ 裸指针
    run_session_fn: F,  // ❌ 闭包捕获裸指针
)
```

**问题**:
- 13个参数，接口过于复杂
- `tui_runtime` 裸指针专为 TUI 硬编码
- 标准流程 `flow_standard` 传递 `None`，设计不统一
- 闭包捕获裸指针，违反 Rust 安全原则

### 🟡 6. 不必要的状态重置和复杂的模式切换

**问题代码位置**: `cli/src/flow/flow_tui.rs:80-82, 135-138`

```rust
// 第80-82行
tui.app.reset_for_new_query();
tui.app.set_prompt_mode();

// ...

// 第135-138行
tui.app.input_buffer.clear();
tui.app.input_cursor = 0;
tui.app.input_mode = crate::tui::InputMode::Normal;
```

**问题**: 
- 状态重置逻辑散落在多处
- `reset_for_new_query()` 后又手动设置模式
- 容易遗漏某些状态字段的重置

### 🟡 7. 事件处理中的按键过滤不一致

**问题代码位置**: `cli/src/flow/flow_tui.rs:344-357`

```rust
match key.code {
    // 硬编码的白名单
    KeyCode::Char('q') | KeyCode::Char('c') | KeyCode::Tab | 
    KeyCode::Char('1') | KeyCode::Char('2') | KeyCode::Char('3') |
    KeyCode::Char('k') | KeyCode::Char('j') | KeyCode::Char('u') | 
    KeyCode::Char('d') | KeyCode::Char('g') | KeyCode::Char('G') |
    KeyCode::Char('p') | KeyCode::Char(' ') |
    KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
        if tui.app.handle_key(key) { exit_requested = true; }
    }
    _ => {
        tracing::trace!("Ignoring key during execution: {:?}", key);
    }
}
```

**问题**:
- 硬编码的按键列表，难以维护
- 与 `TuiApp::handle_key` 的逻辑重复
- 注释说"忽略字符输入"，但实际是白名单过滤
- 新增快捷键需要修改多处

### 🟡 8. 错误处理不完整

**问题代码位置**: `cli/src/flow/flow_tui.rs:366-373`

```rust
res = &mut run_task => {
    let res = match res {
        Ok(inner) => inner,
        Err(e) => {
            let err_msg = format!("Task panic: {}", e);
            handle_execution_error(&mut tui.app, &err_msg);
            run_result = Some(Err(RunnerError::Spawn(err_msg)));
            continue; // ❌ 继续循环，但不 break
        }
    };
}
```

**问题**:
- 任务 panic 后设置 `run_result` 但继续循环
- 只有用户手动退出时才返回结果
- 错误状态下应该让用户选择：重试/退出，而非强制等待

### 🟡 9. run_id 生成逻辑混乱

**问题代码分布**:

```rust
// app.rs:70 - 外部生成 run_id
let run_id = recover_run_id.clone()
    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

// flow_tui.rs:144 - 循环内重新生成
let query_run_id = Uuid::new_v4().to_string();
tui.app.run_id = query_run_id.clone();
```

**问题**:
- 每次查询都生成新的 `run_id`
- 外部传入的 `run_id` 参数被忽略
- 多轮查询的 run_id 不连贯，影响日志追踪

### 🟡 10. 多余的数据结构和复杂度

**问题分析**:

1. **TuiRuntime** 只是简单封装:
   ```rust
   pub struct TuiRuntime {
       pub terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
       pub app: TuiApp,
   }
   ```
   - 没有提供额外的抽象价值
   - 可以直接使用 `terminal` 和 `app`

2. **RunSessionInput** 包含 13 个字段:
   - 可以用 builder 模式简化
   - 字段职责不清晰（既有配置又有运行时状态）

3. **事件监听代码重复**:
   - `run_tui_session_continuing` 中的状态监听逻辑
   - 与 `flow_qa.rs` 中的逻辑高度相似
   - 应该抽取为独立函数

---

## 二、重构目标

### 核心原则

1. ✅ **消除 unsafe 代码** - 使用 Rust 安全的并发原语
2. ✅ **单一职责** - 每个模块只做一件事
3. ✅ **清晰的所有权** - 避免复杂的生命周期和借用
4. ✅ **可测试性** - 各层独立，易于单元测试
5. ✅ **资源管理** - 确保所有资源正确创建和释放

### 量化指标

- **代码行数**: 减少 30-40% (`flow_tui.rs` 从 443行 → ~150行)
- **圈复杂度**: 降低 50%+
- **unsafe 块**: 0个
- **参数数量**: 核心函数参数 ≤ 8个
- **嵌套循环**: 最多 1层
- **资源泄漏**: 0个（通过 RAII 保证）

---

## 三、新架构设计

### 3.1 模块分层

```
┌─────────────────────────────────────┐
│   TuiOrchestrator (编排器)          │
│  - 管理整个 TUI 生命周期             │
│  - 协调各个子系统                    │
│  - 状态机: Prompting → Executing    │
│            → Reviewing → [Loop|Exit]│
└──────────┬──────────────────────────┘
           │
    ┌──────┴──────┬──────────┬─────────┐
    │             │          │         │
┌───▼───┐  ┌─────▼────┐ ┌───▼───┐ ┌──▼──┐
│TuiView│  │TuiInput  │ │TuiState│ │Exec │
│(渲染)  │  │(输入处理)│ │(状态)  │ │(执行)│
└───────┘  └──────────┘ └────────┘ └─────┘
```

### 3.2 核心组件设计

#### A. TuiOrchestrator (编排器)

**职责**: 协调 TUI 的整个生命周期

**状态机**:
```
┌──────────┐
│ Prompting│ ─────► 用户输入查询
└─────┬────┘
      │ Submit
      ▼
┌──────────┐
│Executing │ ─────► 执行查询，显示进度
└─────┬────┘
      │ Complete/Error
      ▼
┌──────────┐
│Reviewing │ ─────► 审查结果
└─────┬────┘
      │
      ├─► 'n' / Enter ─────► Prompting (循环)
      └─► 'q' / Ctrl+C ───► Exit
```

**字段**:
```rust
pub struct TuiOrchestrator {
    view_controller: TuiViewController,
    input_handler: TuiInputHandler,
    state: Arc<RwLock<TuiState>>,
    config: TuiConfig,
    phase: OrchestratorPhase,
}

enum OrchestratorPhase {
    Prompting,
    Executing,
    Reviewing,
}
```

**方法**:
```rust
impl TuiOrchestrator {
    pub fn new(config: TuiConfig, run_id: String) -> Result<Self>;
    pub async fn run(mut self, ctx: ExecutionContext) -> Result<i32>;
    
    async fn phase_prompting(&mut self) -> Result<String>;
    async fn phase_executing(&mut self, query: String, ctx: &ExecutionContext) 
        -> Result<ExecutionResult>;
    async fn phase_reviewing(&mut self) -> Result<UserAction>;
}

enum UserAction {
    NewQuery,
    Quit,
}
```

---

#### B. TuiViewController (视图控制器)

**职责**: 负责终端渲染和显示逻辑

**字段**:
```rust
pub struct TuiViewController {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    state: Arc<RwLock<TuiState>>,
    tick_interval: Duration,
    render_task: Option<JoinHandle<()>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}
```

**方法**:
```rust
impl TuiViewController {
    pub fn new(state: Arc<RwLock<TuiState>>, config: &TuiConfig) 
        -> Result<Self>;
    
    pub fn start_rendering(&mut self) -> Result<()>;
    pub fn stop_rendering(&mut self);
    
    fn draw_frame(&mut self) -> Result<()>;
}
```

**设计要点**:
- 渲染循环在独立的 tokio 任务中运行
- 通过 `shutdown_tx` 接收停止信号
- 只读访问状态（`state.read()`），不修改
- 使用 `try_read()` 避免阻塞主逻辑

---

#### C. TuiInputHandler (输入处理器)

**职责**: 处理用户输入事件

**字段**:
```rust
pub struct TuiInputHandler {
    reader: Option<InputReader>,
    current_mode: InputMode,
}

pub enum InputMode {
    Prompting,    // 输入提示，允许文本编辑
    Executing,    // 执行中，只允许导航/退出
    Reviewing,    // 审查结果，允许导航和决策
}
```

**方法**:
```rust
impl TuiInputHandler {
    pub fn new() -> Self;
    
    pub fn start_reading(&mut self, mode: InputMode) 
        -> UnboundedReceiver<InputEvent>;
    
    pub fn stop_reading(&mut self);
    
    pub fn handle_key(&self, key: KeyEvent, state: &mut TuiState) 
        -> KeyAction;
}

pub enum KeyAction {
    Submit(String),         // 提交输入
    Navigate(NavAction),    // 导航操作
    Exit,                   // 退出
    None,                   // 无操作
}

pub enum NavAction {
    ScrollUp(usize),
    ScrollDown(usize),
    SwitchPanel(PanelKind),
    ToggleExpand,
    // ...
}
```

**设计要点**:
- 每次调用 `start_reading` 会停止之前的 reader
- 按 `InputMode` 过滤按键，逻辑集中
- 使用 RAII 模式确保资源清理

---

#### D. TuiState (状态管理)

**职责**: 集中管理所有 TUI 状态

**字段**:
```rust
pub struct TuiState {
    // 元数据
    pub run_id: String,
    pub session_start: Instant,
    
    // 输入状态
    pub input_buffer: String,
    pub input_cursor: usize,
    pub selection: Option<(usize, usize)>,
    
    // 运行状态
    pub status: RunStatus,
    pub runtime_phase: Option<RuntimePhase>,
    pub memory_hits: usize,
    pub tool_events_count: usize,
    
    // 输出数据
    pub tool_events: VecDeque<ToolEventEntry>,
    pub assistant_lines: VecDeque<String>,
    pub raw_lines: VecDeque<RawLine>,
    
    // UI 状态
    pub active_panel: PanelKind,
    pub scroll_offsets: [usize; 3],
    pub expanded_events: HashSet<usize>,
    pub show_splash: bool,
}
```

**方法**:
```rust
impl TuiState {
    pub fn new(run_id: String, config: &TuiConfig) -> Self;
    
    pub fn reset_for_new_query(&mut self, new_run_id: String);
    
    pub fn apply_event(&mut self, event: TuiEvent);
    
    pub fn handle_input_char(&mut self, c: char);
    pub fn handle_backspace(&mut self);
    pub fn handle_cursor_move(&mut self, offset: isize);
    
    pub fn scroll_panel(&mut self, delta: isize);
    pub fn switch_panel(&mut self, panel: PanelKind);
}
```

**设计要点**:
- 所有状态集中管理，避免分散
- 方法只修改内部状态，不涉及 I/O
- 易于测试（纯数据结构 + 纯函数）

---

#### E. ExecutionCoordinator (执行协调器)

**职责**: 协调查询执行和事件处理

**字段**:
```rust
pub struct ExecutionCoordinator {
    state: Arc<RwLock<TuiState>>,
    event_tasks: Vec<JoinHandle<()>>,
}
```

**方法**:
```rust
impl ExecutionCoordinator {
    pub fn new(state: Arc<RwLock<TuiState>>) -> Self;
    
    pub async fn execute(
        &mut self,
        query: String,
        ctx: &ExecutionContext,
    ) -> Result<RunnerResult>;
    
    fn spawn_event_listener(
        &mut self, 
        event_rx: UnboundedReceiver<TuiEvent>
    );
    
    fn spawn_state_listener(
        &mut self,
        state_manager: Arc<StateManager>,
        session_id: String,
    );
    
    async fn cleanup(&mut self);
}
```

**执行流程**:
```
1. 创建事件通道 (tui_tx, tui_rx)
2. 启动事件监听任务 → 更新 TuiState
3. 启动状态管理器监听任务 → 更新 TuiState
4. 调用 run_with_query 执行查询
5. 等待完成
6. 清理所有任务
7. 返回结果
```

---

## 四、通信机制设计

### 4.1 Channel 架构

```
┌───────────────┐
│ run_session   │
│   (core)      │
└───────┬───────┘
        │ TuiEvent (tool_event, output, error...)
        ▼
┌───────────────────┐
│ Event Listener    │ ─────► state.write().apply_event(event)
│  (tokio task)     │
└───────────────────┘
                           ┌──────────────┐
┌───────────────┐          │  TuiState    │
│StateManager   │          │ (Arc<RwLock>)│
└───────┬───────┘          └──────┬───────┘
        │ StateEvent              │
        ▼                         │ state.read()
┌───────────────────┐             │
│State Listener     │ ────────────┤
│  (tokio task)     │             │
└───────────────────┘             ▼
                           ┌──────────────┐
                           │TuiView       │
                           │Controller    │
                           │ (rendering)  │
                           └──────────────┘
```

### 4.2 生命周期管理

#### InputReader 生命周期

```rust
// RAII 包装器
pub struct ScopedInputReader {
    reader: InputReader,
}

impl Drop for ScopedInputReader {
    fn drop(&mut self) {
        self.reader.stop();
        tracing::debug!("InputReader stopped and cleaned up");
    }
}

impl TuiInputHandler {
    pub fn start_reading(&mut self, mode: InputMode) 
        -> UnboundedReceiver<InputEvent> 
    {
        // 停止之前的 reader
        if let Some(old_reader) = self.reader.take() {
            old_reader.stop();
        }
        
        let (reader, rx) = InputReader::start();
        self.reader = Some(reader);
        self.current_mode = mode;
        rx
    }
}
```

#### 渲染任务生命周期

```rust
impl TuiViewController {
    pub fn start_rendering(&mut self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let state = self.state.clone();
        let tick_interval = self.tick_interval;
        
        let task = tokio::spawn(async move {
            let mut tick = tokio::time::interval(tick_interval);
            loop {
                tokio::select! {
                    _ = shutdown_rx => break,
                    _ = tick.tick() => {
                        // 渲染逻辑
                        if let Ok(state) = state.try_read() {
                            // draw(&state);
                        }
                    }
                }
            }
        });
        
        self.render_task = Some(task);
        self.shutdown_tx = Some(shutdown_tx);
        Ok(())
    }
    
    pub fn stop_rendering(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.render_task.take() {
            task.abort();
        }
    }
}

impl Drop for TuiViewController {
    fn drop(&mut self) {
        self.stop_rendering();
        tracing::debug!("TuiViewController cleaned up");
    }
}
```

#### 事件监听任务生命周期

```rust
impl ExecutionCoordinator {
    fn spawn_event_listener(
        &mut self,
        mut event_rx: UnboundedReceiver<TuiEvent>,
    ) {
        let state = self.state.clone();
        let task = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                if let Ok(mut state) = state.write() {
                    state.apply_event(event);
                }
            }
            tracing::debug!("Event listener task finished");
        });
        self.event_tasks.push(task);
    }
    
    async fn cleanup(&mut self) {
        for task in self.event_tasks.drain(..) {
            task.abort();
            let _ = task.await;
        }
    }
}
```

---

## 五、重构实施步骤

### 阶段 1: 基础设施 (1-2天)

#### 任务清单

- [ ] 创建 `cli/src/tui/state.rs`
  - [ ] 定义 `TuiState` 结构体
  - [ ] 实现 `new()` 和 `reset_for_new_query()`
  - [ ] 实现 `apply_event()`
  - [ ] 迁移所有状态字段从 `TuiApp`
  
- [ ] 创建 `cli/src/tui/view.rs`
  - [ ] 定义 `TuiViewController` 结构体
  - [ ] 实现终端初始化和清理
  - [ ] 实现独立的渲染循环
  - [ ] 使用 `Arc<RwLock<TuiState>>` 只读访问

- [ ] 添加单元测试
  - [ ] `TuiState::apply_event` 测试
  - [ ] `TuiState::reset_for_new_query` 测试
  - [ ] 状态转换逻辑测试

#### 验收标准

- ✅ `TuiState` 可以独立创建和更新
- ✅ `TuiViewController` 可以在测试中模拟渲染
- ✅ 所有测试通过
- ✅ 编译无警告

#### 估计工时: 8-12小时

---

### 阶段 2: 输入处理重构 (1天)

#### 任务清单

- [ ] 创建 `cli/src/tui/input.rs`
  - [ ] 定义 `TuiInputHandler` 结构体
  - [ ] 定义 `InputMode` 枚举
  - [ ] 实现 `start_reading()` 和 `stop_reading()`
  - [ ] 实现按模式的按键处理逻辑
  
- [ ] 创建 RAII 包装器
  - [ ] `ScopedInputReader` 实现 `Drop` trait
  - [ ] 确保资源自动清理

- [ ] 重构按键处理
  - [ ] 从 `TuiApp` 提取按键处理逻辑
  - [ ] 按 `InputMode` 分离不同的处理路径
  - [ ] 移除硬编码的按键白名单

- [ ] 添加测试
  - [ ] 按键映射测试
  - [ ] 模式切换测试
  - [ ] 资源清理测试（验证 Drop 被调用）

#### 验收标准

- ✅ 输入处理逻辑独立于渲染
- ✅ 无 InputReader 泄漏（通过日志验证）
- ✅ 按键处理在三种模式下行为正确
- ✅ 所有测试通过

#### 估计工时: 6-8小时

---

### 阶段 3: 执行协调器 (2天)

#### 任务清单

- [ ] 创建 `cli/src/tui/coordinator.rs`
  - [ ] 定义 `ExecutionCoordinator` 结构体
  - [ ] 实现 `execute()` 方法
  - [ ] 实现 `spawn_event_listener()`
  - [ ] 实现 `spawn_state_listener()`
  - [ ] 实现 `cleanup()` 方法

- [ ] 修改 `flow_qa.rs`
  - [ ] 移除 `tui_runtime: Option<*mut TuiRuntime>` 参数
  - [ ] 添加 `tui_events_tx: Option<UnboundedSender<TuiEvent>>` 参数
  - [ ] 更新所有调用点

- [ ] 迁移事件监听逻辑
  - [ ] 从 `flow_tui.rs` 提取状态监听代码
  - [ ] 统一事件处理逻辑
  - [ ] 确保任务正确清理

- [ ] 添加测试
  - [ ] 事件处理测试（模拟事件流）
  - [ ] 任务生命周期测试
  - [ ] 错误场景测试

#### 验收标准

- ✅ 无 unsafe 代码
- ✅ 所有异步任务正确清理（无泄漏）
- ✅ 执行阶段可以独立测试
- ✅ TUI 和 Standard 流程使用相同的接口

#### 估计工时: 12-16小时

---

### 阶段 4: 编排器实现 (2天)

#### 任务清单

- [ ] 创建 `cli/src/tui/orchestrator.rs`
  - [ ] 定义 `TuiOrchestrator` 结构体
  - [ ] 定义 `OrchestratorPhase` 枚举
  - [ ] 实现 `new()` 方法
  - [ ] 实现 `run()` 主循环
  - [ ] 实现 `phase_prompting()`
  - [ ] 实现 `phase_executing()`
  - [ ] 实现 `phase_reviewing()`

- [ ] 重构 `flow_tui.rs`
  - [ ] 简化 `run_tui_flow` 为入口函数
  - [ ] 委托给 `TuiOrchestrator::run()`
  - [ ] 移除所有嵌套循环
  - [ ] 移除 `TuiRuntime` 结构体

- [ ] 集成所有组件
  - [ ] `TuiOrchestrator` 持有所有子系统
  - [ ] 状态机清晰表达阶段转换
  - [ ] 统一错误处理

- [ ] 添加测试
  - [ ] 完整流程测试（mock 输入和执行）
  - [ ] 状态转换测试
  - [ ] 多轮查询测试

#### 验收标准

- ✅ `run_tui_flow` 代码行数减少 50%+
- ✅ 状态转换清晰可见
- ✅ 支持多轮交互
- ✅ 所有测试通过

#### 估计工时: 12-16小时

---

### 阶段 5: 接口适配和插件管理 (1天)

#### 任务清单

- [ ] 统一插件管理
  - [ ] 在 `app.rs` 中创建插件实例
  - [ ] 通过 `ExecutionContext` 传递给编排器
  - [ ] 移除循环内的插件重建逻辑

- [ ] 更新 `flow_standard.rs`
  - [ ] 适配新的 `run_with_query` 接口
  - [ ] 传递 `None` 给 `tui_events_tx`

- [ ] 修改 `ExecutionContext`
  - [ ] 添加插件引用字段
  - [ ] 简化参数传递

- [ ] 统一 run_id 管理
  - [ ] 在编排器层面管理 run_id
  - [ ] 每个会话使用基础 run_id + 查询序号

#### 验收标准

- ✅ 插件只创建一次
- ✅ TUI 和 Standard 流程接口一致
- ✅ run_id 追踪连贯
- ✅ 参数数量 ≤ 8个

#### 估计工时: 6-8小时

---

### 阶段 6: 清理和优化 (1天)

#### 任务清单

- [ ] 清理冗余代码
  - [ ] 移除 `TuiRuntime` 结构体
  - [ ] 移除 `run_tui_session_continuing` 函数
  - [ ] 清理未使用的导入和函数

- [ ] 统一错误处理
  - [ ] 创建 `TuiError` 类型
  - [ ] 统一错误消息格式
  - [ ] 改进用户可见的错误信息

- [ ] 文档和注释
  - [ ] 为每个模块添加文档注释
  - [ ] 更新 `tui-design.md`
  - [ ] 添加架构图

- [ ] 性能测试
  - [ ] 高频事件更新压测
  - [ ] 内存占用测试
  - [ ] 与旧版本性能对比

#### 验收标准

- ✅ 编译无警告
- ✅ `cargo clippy` 无问题
- ✅ 所有测试通过
- ✅ 性能无回归（±5%）
- ✅ 文档完整

#### 估计工时: 6-8小时

---

### 总估计工时: 50-68小时 (约 6-8 个工作日)

---

## 六、风险评估与缓解

### 风险 1: 并发性能问题

**描述**: `Arc<RwLock<TuiState>>` 可能引入锁竞争

**概率**: 中等  
**影响**: 中等（渲染延迟）

**缓解措施**:
1. 使用 `try_read()` 和 `try_write()` 避免阻塞
2. 渲染频率限制（16ms tick，最高60fps）
3. 事件批处理更新状态（减少锁持有次数）
4. 如果性能仍有问题，考虑使用 `parking_lot::RwLock`（更快）

**监控**:
- 添加锁持有时间的 tracing
- 性能测试中测量 P50/P95/P99 延迟

---

### 风险 2: 破坏现有功能

**描述**: 重构可能导致功能回退或引入新 bug

**概率**: 中等  
**影响**: 高（用户体验）

**缓解措施**:
1. **分阶段实施**，每阶段独立验证
2. **保留旧代码**在 `flow_tui_legacy.rs` 中作为参考
3. **Feature flag**：`tui-refactored`，逐步切换
4. **端到端测试**覆盖主要场景：
   - 单次查询
   - 多轮交互
   - 用户中断
   - 执行失败
   - 长时间运行
5. **回滚计划**：如果发现严重问题，可快速回退

**监控**:
- CI 中运行所有现有测试
- Beta 用户测试
- 收集用户反馈

---

### 风险 3: 开发周期延长

**描述**: 重构可能需要比预期更长时间

**概率**: 中等  
**影响**: 中等（延迟其他功能开发）

**缓解措施**:
1. **MVP 策略**：前 3 个阶段完成即可发布
   - 阶段1-3 完成后，核心功能可用
   - 阶段4-6 可以增量优化
2. **并行开发**：不阻塞其他模块的开发
3. **时间盒**：每个阶段设置最大工时
4. **技术债务记录**：如果时间紧张，先实现核心功能，记录优化点

**监控**:
- 每日工时记录
- 每阶段完成后 review 进度
- 及时调整计划

---

### 风险 4: 学习曲线

**描述**: 新架构对其他开发者可能不够直观

**概率**: 低  
**影响**: 中等（维护成本）

**缓解措施**:
1. **完善文档**：
   - 架构图
   - 数据流图
   - 各组件职责说明
2. **代码注释**：关键设计决策添加注释
3. **示例代码**：在文档中提供使用示例
4. **Code review**：团队成员参与 review

---

## 七、预期收益

### 7.1 代码质量

| 指标 | 当前 | 目标 | 改善 |
|-----|------|------|------|
| `flow_tui.rs` 行数 | 443 | ~150 | -66% |
| unsafe 块 | 1 | 0 | -100% |
| 函数参数数量 (max) | 16 | 8 | -50% |
| 嵌套循环深度 | 3 | 1 | -67% |
| 圈复杂度 (avg) | ~15 | ~5 | -67% |

### 7.2 可维护性

**改善前**:
- ❌ 事件循环逻辑分散在 3 处
- ❌ 状态散落在多个结构体
- ❌ 输入处理与业务逻辑耦合
- ❌ 难以定位问题根源

**改善后**:
- ✅ 单一职责，每个模块功能清晰
- ✅ 状态集中管理
- ✅ 组件独立可测试
- ✅ 新功能扩展简单（如：添加新 Panel、新快捷键）

### 7.3 性能

**预期改进**:
- ⚡ 减少不必要的状态拷贝（使用引用）
- ⚡ 事件批处理减少锁竞争
- ⚡ 插件复用减少初始化开销（~10-20ms per query）
- ⚡ 渲染频率限制防止 CPU 占用过高

**性能测试场景**:
1. 高频事件更新（100 events/s）
2. 大量输出（10K 行）
3. 长时间运行（1小时+）

### 7.4 安全性

| 安全指标 | 当前 | 目标 |
|---------|------|------|
| 内存安全问题 | 可能（unsafe） | 零（编译器保证） |
| 数据竞争 | 可能（裸指针） | 零（RwLock） |
| 资源泄漏 | 可能（InputReader） | 零（RAII） |
| 悬垂指针 | 可能（异步 + 裸指针） | 不可能 |

---

## 八、测试策略

### 8.1 单元测试

#### TuiState 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_creation() {
        let state = TuiState::new("test-run-id".to_string(), &TuiConfig::default());
        assert_eq!(state.run_id, "test-run-id");
        assert_eq!(state.status, RunStatus::Running);
    }

    #[test]
    fn test_apply_tool_event() {
        let mut state = TuiState::new("test".to_string(), &TuiConfig::default());
        let event = TuiEvent::ToolEvent(Box::new(/* ... */));
        state.apply_event(event);
        assert_eq!(state.tool_events.len(), 1);
    }

    #[test]
    fn test_reset_for_new_query() {
        let mut state = TuiState::new("test".to_string(), &TuiConfig::default());
        state.input_buffer = "old query".to_string();
        state.tool_events.push_back(/* ... */);
        
        state.reset_for_new_query("new-run-id".to_string());
        
        assert_eq!(state.run_id, "new-run-id");
        assert_eq!(state.input_buffer, "");
        assert_eq!(state.tool_events.len(), 0);
    }
}
```

#### TuiInputHandler 测试

```rust
#[test]
fn test_key_handling_in_prompting_mode() {
    let handler = TuiInputHandler::new();
    let mut state = TuiState::new("test".to_string(), &TuiConfig::default());
    
    let key = KeyEvent::from(KeyCode::Char('a'));
    let action = handler.handle_key(key, &mut state);
    
    assert_eq!(action, KeyAction::None);
    assert_eq!(state.input_buffer, "a");
}

#[test]
fn test_key_handling_in_executing_mode() {
    let mut handler = TuiInputHandler::new();
    handler.current_mode = InputMode::Executing;
    let mut state = TuiState::new("test".to_string(), &TuiConfig::default());
    
    let key = KeyEvent::from(KeyCode::Char('a'));
    let action = handler.handle_key(key, &mut state);
    
    // 执行模式下忽略字符输入
    assert_eq!(action, KeyAction::None);
    assert_eq!(state.input_buffer, "");
}
```

#### ExecutionCoordinator 测试

```rust
#[tokio::test]
async fn test_event_listener() {
    let state = Arc::new(RwLock::new(TuiState::new("test".to_string(), &TuiConfig::default())));
    let mut coordinator = ExecutionCoordinator::new(state.clone());
    
    let (tx, rx) = mpsc::unbounded_channel();
    coordinator.spawn_event_listener(rx);
    
    tx.send(TuiEvent::AssistantOutput("test line".to_string())).unwrap();
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let state = state.read().unwrap();
    assert_eq!(state.assistant_lines.len(), 1);
}
```

### 8.2 集成测试

#### 完整流程测试

```rust
#[tokio::test]
async fn test_full_tui_flow() {
    // Mock 输入：输入查询 → 等待执行 → 选择退出
    let mock_inputs = vec![
        "test query\n",
        "q",
    ];
    
    // Mock 执行上下文
    let ctx = build_mock_context();
    
    // 运行 TUI
    let result = run_tui_flow_with_mock(mock_inputs, ctx).await;
    
    assert!(result.is_ok());
    // 验证日志、状态等
}
```

#### 多轮交互测试

```rust
#[tokio::test]
async fn test_multiple_queries() {
    let mock_inputs = vec![
        "query 1\n",
        "n",  // 新查询
        "query 2\n",
        "q",  // 退出
    ];
    
    let result = run_tui_flow_with_mock(mock_inputs, ctx).await;
    
    // 验证两个查询都执行了
    assert_eq!(execution_count, 2);
}
```

#### 异常场景测试

```rust
#[tokio::test]
async fn test_execution_failure() {
    let mock_inputs = vec![
        "failing query\n",
        "q",
    ];
    
    let ctx = build_mock_context_with_failure();
    let result = run_tui_flow_with_mock(mock_inputs, ctx).await;
    
    // 验证错误被正确处理
    assert!(matches!(result, Ok(code) if code != 0));
}
```

### 8.3 性能测试

#### 高频事件测试

```rust
#[tokio::test]
async fn test_high_frequency_events() {
    let state = Arc::new(RwLock::new(TuiState::new("test".to_string(), &TuiConfig::default())));
    let (tx, rx) = mpsc::unbounded_channel();
    
    // 启动事件监听
    spawn_event_listener(rx, state.clone());
    
    // 发送 1000 个事件
    let start = Instant::now();
    for i in 0..1000 {
        tx.send(TuiEvent::AssistantOutput(format!("line {}", i))).unwrap();
    }
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    let elapsed = start.elapsed();
    
    // 验证性能
    assert!(elapsed < Duration::from_millis(500), "处理1000个事件超过500ms");
    
    let state = state.read().unwrap();
    assert_eq!(state.assistant_lines.len(), 1000);
}
```

#### 内存占用测试

```rust
#[tokio::test]
async fn test_memory_usage() {
    let initial_memory = get_current_memory_usage();
    
    // 运行多轮查询
    for _ in 0..10 {
        run_single_query().await;
    }
    
    let final_memory = get_current_memory_usage();
    let diff = final_memory - initial_memory;
    
    // 验证没有明显的内存泄漏（允许10MB增长）
    assert!(diff < 10 * 1024 * 1024, "内存增长超过10MB");
}
```

### 8.4 测试覆盖率目标

| 模块 | 行覆盖率 | 分支覆盖率 |
|-----|---------|-----------|
| `state.rs` | ≥90% | ≥85% |
| `input.rs` | ≥85% | ≥80% |
| `coordinator.rs` | ≥80% | ≥75% |
| `orchestrator.rs` | ≥75% | ≥70% |
| `view.rs` | ≥60% | ≥50% |
| **总体** | **≥80%** | **≥75%** |

---

## 九、实施时间表

### Week 1 (Day 1-3)
- **阶段 1**: 基础设施（TuiState + TuiViewController）
- **阶段 2**: 输入处理重构（TuiInputHandler）

### Week 2 (Day 4-5)
- **阶段 3**: 执行协调器（ExecutionCoordinator）

### Week 2-3 (Day 6-7)
- **阶段 4**: 编排器实现（TuiOrchestrator）

### Week 3 (Day 8)
- **阶段 5**: 接口适配和插件管理

### Week 3 (Day 9)
- **阶段 6**: 清理和优化

### Week 3 (Day 10)
- **Buffer**: 处理遗留问题、性能优化、文档完善

---

## 十、关键决策记录

### 决策 1: 使用 Arc<RwLock<T>> vs Channel

**选择**: `Arc<RwLock<TuiState>>`

**原因**:
- TuiState 需要被多处读取（渲染、输入处理、事件监听）
- 写操作不频繁（主要是事件应用）
- RwLock 允许多读一写，适合这个场景
- Channel 会引入额外的复杂度（需要一个 actor 管理状态）

**权衡**:
- ✅ 简单直观
- ✅ 性能良好（读多写少）
- ⚠️ 需要注意锁持有时间

---

### 决策 2: 渲染循环独立 vs 主循环内渲染

**选择**: 渲染循环在独立任务中

**原因**:
- 渲染和业务逻辑解耦
- 可以精确控制渲染频率（60fps）
- 不会阻塞输入处理和执行逻辑

**权衡**:
- ✅ 性能更好
- ✅ 逻辑更清晰
- ⚠️ 需要管理任务生命周期

---

### 决策 3: 三阶段状态机 vs 更细粒度的状态

**选择**: Prompting → Executing → Reviewing

**原因**:
- 对应用户视角的三个明确阶段
- 易于理解和维护
- 足够表达所有业务逻辑

**备选**: 更多状态（Initializing, WaitingInput, Executing, ShowingResult, WaitingDecision）
- 更精确但过于复杂

---

## 十一、附录

### A. 相关文件清单

**需要修改的文件**:
- `cli/src/flow/flow_tui.rs` - 主要重构目标
- `cli/src/flow/flow_qa.rs` - 接口修改
- `cli/src/flow/flow_standard.rs` - 接口适配
- `cli/src/app.rs` - 插件管理调整
- `cli/src/tui/app.rs` - 部分逻辑迁移
- `cli/src/tui/mod.rs` - 模块导出更新

**需要创建的文件**:
- `cli/src/tui/state.rs` - 新建
- `cli/src/tui/view.rs` - 新建
- `cli/src/tui/input.rs` - 新建
- `cli/src/tui/coordinator.rs` - 新建
- `cli/src/tui/orchestrator.rs` - 新建

**测试文件**:
- `cli/src/tui/state_test.rs`
- `cli/src/tui/input_test.rs`
- `cli/src/tui/coordinator_test.rs`
- `cli/src/tui/orchestrator_test.rs`
- `cli/tests/integration_tui.rs`

---

### B. 参考资料

**Rust 并发编程**:
- [The Rust Book - Fearless Concurrency](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Arc and Mutex patterns](https://doc.rust-lang.org/std/sync/struct.Arc.html)

**TUI 框架**:
- [Ratatui Documentation](https://ratatui.rs/)
- [Crossterm Documentation](https://docs.rs/crossterm/)

**架构模式**:
- [State Pattern](https://refactoring.guru/design-patterns/state)
- [Orchestration Pattern](https://microservices.io/patterns/data/saga.html)

---

### C. 术语表

| 术语 | 定义 |
|-----|------|
| **Orchestrator** | 编排器，协调多个组件完成复杂流程 |
| **RAII** | Resource Acquisition Is Initialization，资源获取即初始化 |
| **Channel** | Rust 中的消息传递通道（mpsc, oneshot） |
| **Arc** | Atomic Reference Counter，原子引用计数 |
| **RwLock** | Read-Write Lock，读写锁 |
| **TUI** | Text User Interface，文本用户界面 |
| **InputReader** | 输入读取器，负责读取键盘/鼠标事件 |
| **StateManager** | 状态管理器，core 模块提供的全局状态管理 |

---

## 十二、总结

这个重构方案旨在解决当前 TUI 架构中的核心问题：

1. **消除 unsafe 代码**，使用 Rust 安全的并发原语（Arc<RwLock<T>>）
2. **分离关注点**，将渲染、输入、执行、状态管理解耦
3. **简化控制流**，用清晰的三阶段状态机替代嵌套循环
4. **防止资源泄漏**，使用 RAII 模式确保资源清理
5. **提升可维护性**，减少代码行数和复杂度

通过渐进式的实施策略，我们可以在不破坏现有功能的前提下，逐步完成重构。每个阶段都有明确的验收标准和测试覆盖，确保质量。

预期实施完成后，TUI 流程将变得：
- ✅ **更安全**：零 unsafe 代码，编译器保证内存安全
- ✅ **更清晰**：单一职责，组件独立
- ✅ **更易维护**：代码量减少 30-40%，复杂度降低 50%+
- ✅ **更易扩展**：新功能添加简单，架构支持

这为后续功能扩展（如：多窗口、自定义主题、历史记录、快照/恢复）打下了坚实的基础。

---

**文档版本**: v1.0  
**最后更新**: 2025年12月29日  
**作者**: GitHub Copilot  
**状态**: 📋 设计阶段 → 待实施
