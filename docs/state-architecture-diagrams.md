# 状态管理架构图

## 整体架构

```
┌──────────────────────────────────────────────────────────────┐
│                      StateManager                             │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                   管理核心                               │  │
│  │  ┌──────────────┐    ┌────────────────┐               │  │
│  │  │  AppState    │    │  Sessions      │               │  │
│  │  │  应用状态     │    │  HashMap       │               │  │
│  │  │              │    │  <ID, Session> │               │  │
│  │  └──────────────┘    └────────────────┘               │  │
│  │                                                         │  │
│  │  ┌──────────────────────────────────────────────────┐ │  │
│  │  │   Event Broadcaster                              │ │  │
│  │  │   (broadcast::Sender<StateEvent>)                │ │  │
│  │  └──────────────────────────────────────────────────┘ │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                               │
│  API:                                                         │
│  • create_session()      - 创建新会话                         │
│  • get_session()         - 获取会话状态                       │
│  • update_session()      - 更新会话状态                       │
│  • transition_phase()    - 状态转换                          │
│  • complete_session()    - 完成会话                          │
│  • subscribe()           - 订阅事件                          │
└──────────────────────────────────────────────────────────────┘
         │                      │                    │
         │                      │                    │
         ▼                      ▼                    ▼
┌─────────────────┐   ┌──────────────────┐  ┌────────────────┐
│  SessionState   │   │  StateEvent      │  │  Snapshot      │
│                 │   │                  │  │  Manager       │
│  • session_id   │   │  • SessionCreated│  │                │
│  • run_id       │   │  • StateChanged  │  │  • save()      │
│  • status       │   │  • Completed     │  │  • restore()   │
│  • runtime      │   │  • Failed        │  │  • cleanup()   │
│  • metadata     │   │  • ...           │  │                │
└─────────────────┘   └──────────────────┘  └────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────┐
│                      RuntimeState                             │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  • phase: RuntimePhase       当前阶段                   │  │
│  │  • tool_events_count         工具事件计数               │  │
│  │  • memory_hits               记忆命中计数               │  │
│  │  • runner_pid                Runner 进程 ID             │  │
│  │  • gatekeeper_decision       Gatekeeper 决策            │  │
│  │  • metrics: RuntimeMetrics   性能指标                   │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## 状态转换图

```
┌──────────────────────────────────────────────────────────────┐
│                     状态转换流程                              │
└──────────────────────────────────────────────────────────────┘

    ┌──────────┐
    │   Idle   │  空闲状态
    └────┬─────┘
         │
         │ transition_to(Initializing)
         ▼
    ┌──────────────┐
    │ Initializing │  初始化配置、环境变量
    └──────┬───────┘
           │
           │ transition_to(MemorySearch)
           ▼
    ┌──────────────┐
    │ MemorySearch │  记忆检索，注入上下文
    └──────┬───────┘
           │
           │ transition_to(RunnerStarting)
           ▼
    ┌────────────────┐
    │ RunnerStarting │  准备启动 codecli 进程
    └──────┬─────────┘
           │
           │ transition_to(RunnerRunning)
           ▼
    ┌────────────────┐
    │ RunnerRunning  │  codecli 进程运行中
    └──────┬─────────┘
           │
           │ transition_to(ProcessingToolEvents)
           ▼
    ┌───────────────────────┐
    │ ProcessingToolEvents  │  解析和处理工具事件
    └──────┬────────────────┘
           │
           │ transition_to(GatekeeperEvaluating)
           ▼
    ┌───────────────────────┐
    │ GatekeeperEvaluating  │  评估是否写入记忆
    └──────┬────────────────┘
           │
           │ transition_to(MemoryPersisting)
           ▼
    ┌──────────────────┐
    │ MemoryPersisting │  沉淀新知识到记忆
    └──────┬───────────┘
           │
           │ transition_to(Completed)
           ▼
    ┌────────────┐
    │ Completed  │  正常完成
    └────────────┘

                任意阶段
                   │
                   │ 遇到错误
                   ▼
               ┌────────┐
               │ Failed │  异常终止
               └────────┘
```

## 事件流

```
┌──────────────────────────────────────────────────────────────┐
│                      事件发布订阅                             │
└──────────────────────────────────────────────────────────────┘

     StateManager
          │
          │ emit_event(StateEvent)
          ▼
    ┌──────────────────────┐
    │  broadcast::Sender   │
    │   (channel size:     │
    │      1000)           │
    └──────────┬───────────┘
               │
               │ broadcast
               ├──────────┬──────────┬──────────┐
               │          │          │          │
               ▼          ▼          ▼          ▼
        ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐
        │Listener1│ │Listener2│ │Listener3│ │Listener │
        │  日志    │ │  指标    │ │ 事件存储 │ │ N...    │
        └─────────┘ └─────────┘ └─────────┘ └─────────┘

事件类型：
• SessionCreated        - 会话创建
• SessionStateChanged   - 阶段转换
• ToolEventReceived     - 接收工具事件
• MemoryHit            - 记忆命中
• GatekeeperDecision   - Gatekeeper 决策
• SessionCompleted     - 会话完成
• SessionFailed        - 会话失败
• AppStarted           - 应用启动
• AppShutdown          - 应用关闭
```

## 并发模型

```
┌──────────────────────────────────────────────────────────────┐
│                    线程安全设计                               │
└──────────────────────────────────────────────────────────────┘

     ┌─────────────────────────────────────┐
     │        Arc<StateManagerInner>       │
     │  ┌───────────────────────────────┐  │
     │  │   RwLock<AppState>            │  │  多读单写
     │  └───────────────────────────────┘  │
     │  ┌───────────────────────────────┐  │
     │  │   RwLock<HashMap<SessionId,   │  │  多读单写
     │  │              SessionState>>   │  │
     │  └───────────────────────────────┘  │
     │  ┌───────────────────────────────┐  │
     │  │   broadcast::Sender           │  │  多生产者多消费者
     │  └───────────────────────────────┘  │
     └─────────────────────────────────────┘
                    │
                    │ Clone (Arc)
        ┌───────────┼───────────┐
        │           │           │
        ▼           ▼           ▼
   ┌─────────┐ ┌─────────┐ ┌─────────┐
   │ Thread1 │ │ Thread2 │ │ Thread3 │
   │         │ │         │ │         │
   │ 读状态   │ │ 写状态   │ │ 订阅事件│
   └─────────┘ └─────────┘ └─────────┘
```

## 集成点

```
┌──────────────────────────────────────────────────────────────┐
│              与现有模块的集成点                                │
└──────────────────────────────────────────────────────────────┘

     ┌────────────────┐
     │   CLI Entry    │  命令行入口
     │   (main.rs)    │
     └────────┬───────┘
              │
              │ 创建 StateManager
              ▼
     ┌────────────────┐
     │  run_cmd()     │  运行命令
     └────────┬───────┘
              │
              ├─────► create_session()
              │
              ├─────► Initializing
              │
              ├─────► MemoryClient::search()
              │         └─► increment_memory_hits()
              │
              ├─────► spawn_child()
              │         └─► set_runner_pid()
              │
              ├─────► tee_child_io()
              │         └─► increment_tool_events()
              │
              ├─────► Gatekeeper::evaluate()
              │         └─► set_gatekeeper_decision()
              │
              └─────► complete_session() / fail_session()


  观测点（订阅事件）：
  • events_out writer    - 写入结构化事件
  • tracing spans        - 追踪日志
  • metrics collector    - 性能指标
  • UI dashboard         - 实时监控（未来）
```

## 数据流

```
┌──────────────────────────────────────────────────────────────┐
│                      数据流向                                 │
└──────────────────────────────────────────────────────────────┘

用户命令
   │
   ▼
CLI Args ────────────────────────┐
   │                             │
   │                             ▼
   │                    ┌──────────────────┐
   │                    │  StateManager    │
   │                    │  • create_session│
   │                    └────────┬─────────┘
   │                             │
   ▼                             │
Config Load                      │
   │                             │
   │                             ▼
   │                    phase = Initializing
   │                             │
   ▼                             │
Memory Search ──────────────────►│ memory_hits++
   │                             │
   │                             ▼
   │                    phase = RunnerRunning
   │                             │
   ▼                             │
spawn_child() ──────────────────►│ runner_pid = PID
   │                             │
   │                             │
   ▼                             ▼
tee_child_io()          phase = ProcessingToolEvents
   │                             │
   │                             │
   ├─► ToolEvent ───────────────►│ tool_events_count++
   │                             │
   │                             │
   ▼                             ▼
Gatekeeper::evaluate()  phase = GatekeeperEvaluating
   │                             │
   │                             │
   └─► Decision ────────────────►│ gatekeeper_decision = {...}
                                 │
                                 ▼
                        phase = Completed
                                 │
                                 │
                                 ▼
                        SessionCompleted Event
                                 │
                                 ▼
                          返回 exit_code
```
