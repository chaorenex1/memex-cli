# 1. 总体约束与假设

1. codecli（或其 agent runtime）需要支持：

- 输出结构化事件（tool.request/tool.result）到 stdout/stderr（JSONL，建议带前缀）
- 在工具执行前进入“等待审批”状态
- 从 stdin 接收控制命令 JSONL，并据此继续/取消/超时

2. mem-codecli 需要支持：

- 不阻塞用户可见流式输出（stdout passthrough）
- 能并行处理：解析事件、跑 policy、交互 ask、向 stdin 写回控制命令
- 能在非交互（CI）下自动决定（deny 或 default\_action）

---

# 2. 通道与角色定义

## 2.1 数据通道

- codecli **stdout**：普通文本 + 事件 JSONL（推荐前缀）
- codecli **stderr**：错误文本 + 事件 JSONL（可选）
- codecli **stdin**：控制协议 JSONL（mem-codecli 写入）

## 2.2 角色

- **Producer（codecli）**：产生 tool.request 等事件
- **Arbiter（mem-codecli）**：解析事件、审核、回写 decision
- **Executor（codecli 内部工具执行）**：收到 allow 才执行；deny/cancel 则跳过或失败

---

# 3. Stdin 控制协议 JSONL（从 mem-codecli → codecli）

## 3.1 基本 envelope（每行一个 JSON object）

必备字段：

- `v`: number（协议版本，当前 `1`）
- `type`: string（控制命令类型）
- `ts`: string（RFC3339，或 epoch\_ms；推荐 RFC3339）
- `id`: string（**关联 ID**：通常等于 tool.request 的 `id`）
- `run_id`: string（可选但推荐；便于跨进程追踪）

可选字段：

- `trace`: object（扩展诊断字段）
- `nonce`: string（可选；用于防重放/去重）

### 推荐命令类型（最小集合）

- `policy.decision`
- `policy.ping`
- `policy.abort`

---

## 3.2 `policy.decision`（核心）

用于回应某个 `tool.request` 是否允许执行。

字段：

- `decision`: `"allow" | "deny" | "ask"`
  - 对 codecli 来说，`ask` 一般不应出现（ask 在 mem-codecli 内部完成）；如出现，codecli 可视为 deny 或继续等待（不推荐）。
- `reason`: string（短原因；可显示给用户）
- `rule_id`: string?（命中的策略规则 id）
- `constraints`: object?（可选：附加约束，如仅允许某 path/domain）
- `expires_in_ms`: number?（可选：本决策有效期；过期后 codecli 可要求重新审批）

示例（允许）：

```json
{"v":1,"type":"policy.decision","ts":"2025-12-26T22:15:00-05:00","run_id":"r-123","id":"t-001","decision":"allow","reason":"allowed by policy","rule_id":"allow.fs.read"}
```

示例（拒绝）：

```json
{"v":1,"type":"policy.decision","ts":"2025-12-26T22:15:01-05:00","run_id":"r-123","id":"t-002","decision":"deny","reason":"shell execution denied by default","rule_id":"deny.shell.exec"}
```

> 约定：`id` 必须能与 `tool.request.id` 一一对应。\
> codecli 若收到未知 id 的 decision，应忽略并记录警告事件（可输出 tool.progress）。

---

## 3.3 `policy.ping`（可选，用于握手/保活）

用途：

- mem-codecli 启动后告诉 codecli：审批通道可用
- codecli 可用于检测是否启用 external approval 模式

字段：

- `capabilities`: array<string>（例如 `["policy.decision.v1"]`）
- `timeout_ms`: number?（codecli 等待审批默认超时）

示例：

```json
{"v":1,"type":"policy.ping","ts":"2025-12-26T22:14:50-05:00","run_id":"r-123","id":"ping-1","capabilities":["policy.decision.v1"],"timeout_ms":300000}
```

---

## 3.4 `policy.abort`（全局终止）

用途：

- mem-codecli 决定终止会话（例如用户取消、发现敏感泄露、策略要求中止）

字段：

- `reason`: string
- `code`: string?（稳定枚举，如 `user_cancel`, `policy_violation`, `fatal_error`）

示例：

```json
{"v":1,"type":"policy.abort","ts":"2025-12-26T22:20:00-05:00","run_id":"r-123","id":"abort-1","reason":"user cancelled approval","code":"user_cancel"}
```

codecli 行为建议：

- 收到 abort 后尽快停止等待/执行，输出一条 tool.result（ok=false）或直接退出并带明确信息

---

# 4. Stdout 事件协议（codecli → mem-codecli）与审批等待语义

你已经有 `tool.request`/`tool.result` JSONL 约定；为了实现“等待审批”，需要补一个明确的状态语义：

## 4.1 tool.request 必须指示“will\_wait\_for\_policy”

在 `tool.request` 增加字段：

- `requires_policy`: boolean（true 表示必须等外部 decision）

示例：

```json
{"v":1,"type":"tool.request","ts":"2025-12-26T22:11:03-05:00","id":"t-001","tool":"fs.read","action":"read","args":{"path":"README.md"},"requires_policy":true}
```

## 4.2 codecli 等待策略

当 `requires_policy=true`：

- codecli 输出 tool.request 后进入等待
- 等待 stdin 上收到 `policy.decision`（同 id）
- 超时策略（建议）：
  - 超时后默认 deny，并输出 tool.result（ok=false, error="policy timeout"）
  - 或请求重试（再输出一次 tool.request，带新 id；不推荐，容易放大事件量）

---

# 5. Runner 层双向通道管理（mem-codecli 侧）

## 5.1 关键设计点

1. **stdin 写入必须串行化**：单 writer task，避免并发写导致 JSONL 粘包
2. **stdout/stderr 读取并行**：两个 reader task，解析事件与透传文本
3. **审批队列与状态表**：

- `pending[id] = ToolRequestEvent + timestamps + retries`
- 决策完成后写回 stdin 并从 pending 移除

4. **背压与超时**：

- policy 决策/交互 ask 有超时（例如 5 分钟）
- stdin 写入若失败 → 认为子进程不可控，触发 abort 或退出

---

## 5.2 Runner trait 增强接口（建议新增一个“控制通道”抽象）

在 `RunnerOutput` 之外，增强为可持续交互的 session：

```rust
pub struct RunnerSession {
    pub status: tokio::sync::oneshot::Receiver<i32>, // child exit code when ends
    pub control_tx: tokio::sync::mpsc::Sender<ControlCommand>,
    pub event_rx: tokio::sync::mpsc::Receiver<ParsedEvent>, // tool events + diagnostics
}
```

其中：

```rust
pub enum ControlCommand {
    StdinJsonl(serde_json::Value),   // write line to child stdin
    Abort { reason: String },        // convenience -> policy.abort
}

pub enum ParsedEvent {
    Tool(crate::tool_events::ToolEvent),
    ParseError { line: String, err: String },
    OutputLine { stream: &'static str, line: String }, // optional for debugging
}
```

Runner 接口新增：

```rust
#[async_trait]
pub trait Runner {
    async fn start_session(&self, /* spec + stream */) -> anyhow::Result<RunnerSession>;
}
```

> 说明：如果你不想改动原 Runner trait，也可以在 `CodecliRunner` 内部暴露一个更高级的 `start_session()`，由 orchestrator 使用；MockRunner 也按此实现。

---

## 5.3 双向通道任务编排（推荐）

启动 codecli 后，mem-codecli 运行以下 tasks：

### Task A：stdout reader（透传 + 事件解析）

- 逐行读取 stdout
- 将非事件行原样写到 parent stdout
- 将事件行解析为 `ToolEvent` 送入 `event_rx`
- 解析失败：计数 + 作为 `ParsedEvent::ParseError` 上报（不影响输出）

### Task B：stderr reader（同上）

- 逐行读取 stderr
- 原样写到 parent stderr
- 可选解析事件

### Task C：stdin writer（唯一写入者）

- 从 `control_tx` 取 `ControlCommand`
- `serde_json::to_string()` + `\n` 写入 child stdin
- flush（可选：批量 flush）
- 写失败：上报 fatal，触发退出流程

### Task D：policy arbiter（审批编排器，核心）

- 从 `event_rx` 取 `ToolEvent::ToolRequest`
- 调用 `PolicyEngine.decide()`：
  - allow/deny：立刻发 `policy.decision`
  - ask：调用 `Approver` 与用户交互，再发 decision
- 维护 pending 表、超时、审计 span

### Task E：process wait

- 等待 child 退出码
- 通知 orchestrator 收尾（hit/candidate/validate 等）

---

# 6. 状态机与一致性规则

## 6.1 每个 tool call 的状态机（按 id）

`New` →（收到 tool.request）→ `PendingDecision`

- 收到 decision(allow) → `Allowed` →（codecli 执行后）→ `Completed`（tool.result ok=true）
- 收到 decision(deny) → `Denied` → `Completed`（tool.result ok=false 或 codecli 直接失败）
- 超时 → `DeniedTimeout` → `Completed`
- stdin 写失败 → `FailedTransport`（触发全局 abort）

一致性规则：

- `policy.decision.id` 必须匹配 pending id，否则丢弃
- 重复的 tool.request（同 id）：
  - 若 pending 已存在，忽略或输出 debug event（防止抖动）
- 重复的 policy.decision（同 id）：
  - 若已决策，忽略（幂等）

---

# 7. 交互 ask 的 UX 约定（prompt 模式）

当 `AuditMode=prompt` 且 policy 返回 `Ask`：

- mem-codecli 在 **stderr** 输出一个简洁审批提示（避免污染 stdout 的主要内容）
- 提示内容包含：
  - tool、action、关键 args（脱敏）
  - rationale（如有）
  - 默认选择（建议默认 deny）
- 输入读取来源：
  - 若 stdin 被占用写控制协议，**审批输入必须来自 mem-codecli 的 parent stdin**（也就是用户终端输入），并与 child stdin 分离
  - 这点非常关键：child stdin 是控制通道，不能混用为用户输入

工程建议：

- `Approver` 读 parent stdin（TTY），child stdin 专用于控制命令
- 非交互环境：直接按 `default_action`（通常 deny）

---

# 8. 协议兼容与降级

1. 若 codecli 不支持等待审批：

- mem-codecli 仍可解析 tool.request 做审计，但无法阻止执行
- 此时 policy 只能作为“告警”，并可在发现高风险时选择 `policy.abort`（如果 codecli 支持）或直接终止 wrapper

2. 若事件输出缺失字段：

- parser 返回 `ParseError::MissingField`
- 统计错误并继续；可配置达到阈值后 abort（防止协议漂移导致失控）

---

# 9. 最小实现（MVP）建议

先实现 **allow/deny** 两种决策闭环，不实现 constraints 与 ping：

- codecli 输出：`tool.request(requires_policy=true)`、`tool.result`
- mem-codecli：解析 request → policy 决策 → 写入 stdin `policy.decision`
- codecli：收到 decision 后执行或返回失败 result

这样你能最快验证“阻断执行”的关键链路是否成立。