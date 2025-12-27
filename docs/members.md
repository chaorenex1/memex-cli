# Members 职责划分（workspace）

## 依赖方向（强约束）

- `memex-cli` → 依赖 `memex-core` 与 `memex-plugins`
- `memex-plugins` → 依赖 `memex-core`
- `memex-core` → 不依赖其他 member

> 目标：避免环依赖；保证核心逻辑可复用（在线 run / 离线 replay / 测试）。

## memex-core（领域内核 + 可复用引擎）

负责：

- 领域模型与协议：ToolEvent/WrapperEvent、GatekeeperDecision/SearchMatch、memory payload 等
- 纯逻辑：tool_event 解析与关联统计、gatekeeper 决策、candidate 提取、replay 聚合与 diff
- 引擎：runner `run_session`（面向抽象的 `RunnerSession`）与 fail-closed 控制策略
- 配置 schema：`AppConfig` 及子配置结构
- 错误语义与可观测事件输出（events_out）

接口归属（唯一来源）：

- `memex_core::runner::{RunnerSession, RunnerPlugin, PolicyPlugin, PolicyAction, RunnerStartArgs}`
- `memex_core::gatekeeper::GatekeeperPlugin`
- `memex_core::memory::MemoryPlugin`

## memex-plugins（实现层 / 适配器）

负责：

- Runner 实现：如 spawn `codecli`、replay session
- Memory 实现：如 HTTP 调用 memory service
- Policy 实现：基于 config 的 allow/deny/ask 规则
- Gatekeeper 实现：标准/自定义实现（通常委托 core 的 Gatekeeper 逻辑）

原则：

- **不再定义重复的 trait/模型**；只实现/复用 `memex-core` 的接口与类型。

## memex-cli（装配层 / 产品入口）

负责：

- 命令行参数解析（clap）与配置加载/覆盖
- 选择并实例化 plugins（runner/memory/policy/gatekeeper），注入 core 引擎
- 输出形态（text/jsonl）与退出码

原则：

- CLI 不承载业务决策逻辑；尽量只做 wiring。
