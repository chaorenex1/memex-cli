# 1. 术语与边界

- **control channel**：mem-codecli → codecli 的 stdin JSONL 写入通道（单向写）。
- **event channel**：codecli → mem-codecli 的 stdout/stderr（双通道）输出，其中包含 JSONL 事件（tool.request/tool.result/…）。
- **断线**：任一通道发生不可恢复的 I/O 错误（EPIPE、BrokenPipe、EOF、连接重置等）。
- **半关闭**：某些通道关闭但进程仍在运行（例如 stdin 写端断了但 stdout 仍输出，或 stdout 关闭但进程未退出）。
- **卡死**：子进程未退出，但在一段时间内无输出/无事件/无进度，且存在 pending 审批或 pending 工具执行。

---

# 2. 故障分类与优先级

按对安全与一致性的影响从高到低：

1. **control channel 写失败（stdin broken/closed）**
   - 影响：无法下发 deny，工具可能失控执行；或 codecli等待审批永远挂住。
   - 策略：默认 **Fail-Closed**（立刻中止会话或强制退出）。

2. **event channel 读失败（stdout/stderr EOF/错误）**
   - 影响：无法观察 tool.request/result，无法审计；也可能误判卡死。
   - 策略：如果 stdout/stderr 都关闭且进程未退出，优先按卡死/异常处理；若仅一个关闭则降级运行但风险提示。

3. **长时间无进展（卡死）**
   - 影响：资源占用、CI 挂死、用户体验差。
   - 策略：分级探测（软探测→硬中止→强杀），并记录可诊断证据。

---

# 3. 基线策略：Fail-Closed + 可配置例外

默认策略建议：

- 当存在 `requires_policy=true` 的 tool.request（即 **pending\_decision > 0**）时：
  - **任何** control channel 故障都必须进入 Fail-Closed：
    - 优先发送 `policy.abort`（若还能写）
    - 否则直接终止 wrapper，并尽力终止子进程（graceful → kill）

- 当没有 pending\_decision 且 audit=off：
  - 可配置为 Fail-Open（即只作为 runner，不阻断执行），但仍记录错误与告警。

建议配置项（你可放到 `mem_codecli.runner` 或 `mem_codecli.control`）：

- `control.fail_mode = "closed" | "open"`
- `control.abort_on_stdin_failure = true`
- `control.abort_on_event_channel_failure = false`（默认 false，仅告警）

---

# 4. 断线（Disconnected）的检测与恢复

## 4.1 control channel（stdin）断线

### 4.1.1 典型信号

- 写入返回 `BrokenPipe` / `EPIPE`
- stdin handle flush/write 返回 `Err`
- child 已退出导致 stdin 关闭

### 4.1.2 恢复策略（推荐：无恢复、直接收敛）

**不建议“重连 stdin”**：子进程 stdin 一旦关闭基本不可恢复，且重连语义不清。

处理流程：

1. 记录事件：`event!(error.kind="control.stdin_broken", action="control_write", ...)`
2. 标记 `control_state = Failed`
3. 若 `pending_decision > 0` 或 `requires_policy` 模式开启：
   - 进入 **Abort Sequence**（见第 6 节）
4. 若无 pending 且允许 fail-open：
   - 降级：停止发送控制命令，仅做透传与审计（如果还能读 stdout/stderr）
   - 但必须在 stderr 输出“审批通道失效，后续工具调用无法阻断”的警告（可配置是否输出）

> 核心：stdin 断了 = 审批能力失效；默认要中止，避免工具在“该阻断时阻断不了”。

---

## 4.2 event channel（stdout/stderr）断线

### 4.2.1 典型信号

- reader task 读取返回 EOF
- reader task I/O error

### 4.2.2 恢复策略

- **单通道断线**（stdout 或 stderr 其中之一）：
  - 标记该通道关闭，继续运行
  - 解析/审计降级：只从剩余通道解析 JSONL
  - 若协议规定事件只会出现在 stdout，则 stdout 断线属于高风险，建议进入更严格模式：
    - 如果 `requires_policy` 已启用：触发卡死检测计时器更激进（见第 7 节）
    - 或直接 abort（可配置）

- **双通道断线**（stdout 与 stderr 都 EOF）但子进程仍未退出：
  - 视为异常：进入卡死检测的“硬”路径（通常直接 abort + kill）
  - 记录证据：最后一次 output 时间、pending 数量、已解析事件数

---

# 5. 半关闭（Half-closed）的策略

半关闭场景常见组合：

## 5.1 stdin 可写，但 stdout/stderr 之一关闭

- 继续运行（低风险）
- 若 stdout 关闭且事件通常走 stdout：提升风险等级（建议：
  - 将 `policy.decide` 改为更保守：默认 deny
  - 或直接 abort（可配置））

## 5.2 stdout/stderr 还在输出，但 stdin 已不可写

- 如果 audit=off 且 fail-open：可以继续透传
- 如果 audit=prompt/auto 且可能出现 requires\_policy：
  - **一律视为不可接受**，执行 abort sequence（Fail-Closed）

## 5.3 child stdin 被 codecli 主动关闭（“不接受控制”）

- 这代表 codecli 未启用 external approval 模式或内部崩溃
- 处理建议：
  - 若配置要求外部审批（例如 `audit=auto|prompt` 且 `control.required=true`），则直接退出并提示“运行时不支持审批通道”
  - 若非必须，降级为审计-only

建议新增配置：

- `control.required = true|false`（当 true 时，握手失败/写失败直接中止）

---

# 6. Abort Sequence（统一中止序列）

当触发 Fail-Closed 或不可恢复异常时，统一按以下序列执行，确保可诊断、可观测、可控退出：

1. **记录 span/事件**
   - span：`mem_codecli.control.abort`
   - fields：`reason`, `pending_decision`, `pending_exec`, `last_output_age_ms`

2. **尝试发送 `policy.abort`（若 stdin 仍可写）**
   - 超时时间：`abort_write_timeout_ms`（建议 500ms\~2000ms）
   - 发送后等待子进程自行退出：`abort_grace_ms`（建议 2s\~10s）

3. **若未退出：发送 OS 信号**
   - Unix：`SIGTERM`，等待 `term_grace_ms`（2s\~5s）
   - 仍未退出：`SIGKILL`
   - Windows：优先 `CTRL_BREAK_EVENT`（若使用了进程组），否则 `TerminateProcess`

4. **返回退出码**
   - 例如 `40`（policy/approval failure）或 `20`（runner failure）或 `50`（内部异常）
   - 并在 stderr 输出简短原因（脱敏）

5. **保留诊断证据（可选落盘）**
   - ring buffer tail（stdout/stderr 各 N KB）
   - pending 表摘要（id/tool/action/age）
   - 最近一次 tool.request 与未响应的 decision id

---

# 7. 子进程卡死检测（Hang Detection）

卡死检测必须区分两类：

- **审批等待卡死**：pending\_decision>0 且 codecli 等待 decision
- **工具执行卡死**：已 allow，但长时间没有 tool.result 或 progress

## 7.1 观测指标（必须可测）

维护以下时间戳/计数：

- `last_stdout_at`, `last_stderr_at`
- `last_event_at`（解析到任意 ToolEvent）
- `pending_decisions: HashMap<id, created_at>`
- `pending_exec: HashMap<id, allowed_at>`（允许后等待 result 的）
- `last_progress_by_id: HashMap<id, ts>`
- `child_alive: bool`

## 7.2 阈值与计时器（建议配置）

- `hang.idle_output_ms`：无任何 stdout/stderr 输出的阈值（默认 120\_000ms）
- `hang.idle_event_ms`：无事件阈值（默认 120\_000ms）
- `hang.decision_timeout_ms`：等待 decision 的最大时间（默认 300\_000ms）
- `hang.exec_timeout_ms`：等待 tool.result 的最大时间（默认 600\_000ms）
- `hang.probe_interval_ms`：探测周期（默认 1000\~2000ms）

## 7.3 探测逻辑（分级）

### Level 1：软探测（Soft Probe）

触发条件（任一满足）：

- pending\_decision 存在且 `now - created_at > decision_timeout_ms`
- pending\_exec 存在且 `now - allowed_at > exec_timeout_ms`
- `now - last_output_at > idle_output_ms` 且 `child_alive`

动作：

- 记录事件：`mem_codecli.hang.suspected`
- 若 stdin 可写，发送 `policy.ping` 或 `policy.abort` 之前先尝试发送轻量 `policy.ping`（可选）
- 如果 codecli 支持自定义事件，可要求其输出 tool.progress（此为扩展，不强依赖）

### Level 2：硬中止（Hard Abort）

触发条件：

- Level 1 触发后再持续 `hard_grace_ms`（建议 10s\~30s）无进展
- 或 stdout/stderr 双 EOF 且进程仍活着

动作：

- 执行 Abort Sequence（第 6 节）

### Level 3：强制杀死（Kill）

触发条件：

- Hard Abort 后超过 `term_grace_ms` 未退出

动作：

- SIGKILL / TerminateProcess
- 返回 runner failure 退出码

---

# 8. 审批相关的幂等与重放防护

在不稳定 I/O 下，重复发送 decision 可能发生。为避免状态错乱：

1. mem-codecli 对每个 `tool.request.id` 的 decision 只发送一次（正常路径）
2. 若发生“未知是否写入成功”的场景（例如 write 返回超时但不确定是否送达）：
   - 采取 **At-most-once**：不重发，改走 abort（更安全）
   - 或启用 `nonce` 与 codecli ack（扩展，见下）

## 8.1 可选增强：Decision ACK（推荐但非 MVP）

如果你愿意扩协议，可让 codecli 在收到 `policy.decision` 后输出：

- `type="policy.ack"`, `id=<tool_id>`, `decision=<allow/deny>`

这样 mem-codecli 能确认送达，减少误杀。此为增强项，不强制。

---

# 9. 诊断与落盘建议（可选但强烈推荐）

当发生断线/卡死/强杀时，建议写一个诊断包（JSON）：

- `run_id`, `project_id`
- `timestamps`
- `exit_reason`, `exit_code`
- `pending_decisions`, `pending_exec`
- `tail_stdout`, `tail_stderr`（截断）
- `last_events`（最近 N 条 ToolEvent）
- `policy_decisions`（按 id）

配置：

- `diagnostics.enabled = true`
- `diagnostics.dir = ".mem-codecli/diagnostics"`
- `diagnostics.max_tail_bytes = 65536`

---

# 10. 最小可落地实现清单（按优先级）

1. **stdin 写失败 → Fail-Closed + Abort Sequence**
2. **pending\_decision 超时 → Abort Sequence**
3. **pending\_exec 超时 → Abort Sequence**
4. **stdout/stderr 双 EOF 且子进程存活 → Abort Sequence**
5. 记录 spans/events：`control.abort`, `hang.suspected`, `runner.kill`

---