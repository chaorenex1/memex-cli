# 1. Observability Span 命名规范（tracing）

## 1.1 基本原则

1. **全链路关联**：每个 span 必带 `run_id`, `project_id`
2. **稳定命名**：span 名称固定，字段承载变量
3. **分层清晰**：`mem_codecli.*` 为顶层前缀；子模块二级分类
4. **事件可机读**：关键动作用 `event!`，带 `action`, `status`, `latency_ms`, `error.kind`

---

## 1.2 Span 层级与命名

顶层：

- `mem_codecli.run`：一次 `run` 子命令执行的根 span\
  字段建议：
  - `run_id`
  - `project_id`
  - `memory_on`
  - `inject_mode`
  - `gatekeeper`
  - `audit`
  - `redact`
  - `codecli.program`

子流程 spans（建议都作为 `mem_codecli.run` 的 children）：

1. `mem_codecli.context.build`
   - `query_len`, `session_file`, `query_sources`

2. `mem_codecli.memory.search`
   - `limit`, `min_score`, `http.status`（成功时也写）
   - 事件：`memory.search.result`（`items`, `top1_score`）

3. `mem_codecli.prompt.inject`
   - `inject_items`, `inject_chars`, `target`(system/user/both)

4. `mem_codecli.runner.spawn`
   - `program`, `args_len`, `cwd`

5. `mem_codecli.runner.stream`
   - `stream_stdout`, `stream_stderr`, `captured_bytes_stdout`, `captured_bytes_stderr`

6. `mem_codecli.tool.parse`
   - `source`(stdout/stderr), `lines_seen`, `events_parsed`, `errors`

7. `mem_codecli.policy.decide`
   - `tool`, `action`, `decision`(allow/deny/ask), `rule_id`

8. `mem_codecli.memory.hit`
   - `refs`, `used_count`, `shown_count`, `http.status`

9. `mem_codecli.gatekeeper.evaluate`
   - `stdout_len`, `stderr_len`
   - 事件：`gatekeeper.decision`（`write_candidate`, `reason_count`）

10. `mem_codecli.memory.candidate`
    - `question_len`, `answer_len`, `tags_count`, `http.status`

11. `mem_codecli.memory.validate`
    - `strong_signal`, `signal_strength`, `http.status`

---

## 1.3 事件字段约定（建议）

所有关键事件统一字段：

- `action`: `"search" | "inject" | "spawn" | "stream" | "parse" | "decide" | "hit" | "candidate" | "validate"`
- `status`: `"ok" | "error" | "denied" | "skipped"`
- `latency_ms`: u64
- `error.kind`: 稳定枚举字符串（例如 `"memory.timeout"`, `"runner.spawn"`, `"parse.invalid_json"`）
- `error.message`: 简短可读（脱敏后）

---

# 2. ToolEventParser trait + JSONL 事件格式约定

目标：从 codecli 的 stdout/stderr 中解析“结构化工具事件”，驱动 policy 审核与审计日志。解析器必须支持：

- **JSONL**（一行一个 JSON object）
- **混合输出**（普通文本与 JSONL 夹杂）
- **容错**：解析失败不得阻塞主输出；错误计数可观测

---

## 2.2 JSONL 事件格式约定（稳定 schema）

### 2.2.1 基本约定

- 一行一个 JSON object（UTF-8）
- 必须包含：
  - `v`: number（schema version，当前 `1`）
  - `type`: string（事件类型）
  - `ts`: string（RFC3339 或 epoch\_ms 二选一；推荐 RFC3339）
  - `id`: string（事件 id；同一工具调用 request/result 用同 id 关联）
- 可选：
  - `run_id`: string（若 codecli 提供；否则 mem-codecli 在审计侧补齐）
  - `trace`: object（扩展字段）

### 2.2.2 事件类型

#### A) Tool Request

`type = "tool.request"`

必需字段：

- `tool`: string
- `action`: `"read" | "write" | "net" | "exec"`
- `args`: object|array|string|number|boolean|null（建议 object）
  可选字段：
- `rationale`: string

示例（JSONL 单行）：

```json
{"v":1,"type":"tool.request","ts":"2025-12-26T22:11:03-05:00","id":"t-001","tool":"fs.read","action":"read","args":{"path":"README.md"},"rationale":"Need repo overview."}
```

#### B) Tool Result

`type = "tool.result"`

必需字段：

- `ok`: boolean
- `output`: any（成功时建议 object）
  可选字段：
- `error`: string（ok=false 时）

示例：

```json
{"v":1,"type":"tool.result","ts":"2025-12-26T22:11:03-05:00","id":"t-001","ok":true,"output":{"bytes":1024,"snippet":"..."}}
```

#### C) Tool Progress（可选）

`type = "tool.progress"`

字段：

- `stage`: string
- `message`: string?
- `percent`: number?（0..100）

示例：

```json
{"v":1,"type":"tool.progress","ts":"2025-12-26T22:11:04-05:00","id":"t-001","stage":"download","percent":35.0}
```

---

## 2.3 行识别规则（混合输出兼容）

为最大兼容现实 CLI 输出，建议支持两种识别方式（两者都可开）：

1. **前缀模式（推荐，误判率最低）**\
   行以固定前缀开头，例如：
   - `@@MEM_TOOL_EVENT@@ ` + JSON\
     parser 规则：检测前缀后截取 JSON 部分解析

2. **纯 JSON 模式（容错，误判率略高）**\
   如果一行以 `{` 开头并能解析出 `v` 和 `type` 字段，则视为事件行，否则忽略

建议默认启用：前缀模式 + 纯 JSON 模式（但纯 JSON 模式要求必须包含 `v`/`type`，避免误把普通 JSON 打印当事件）。

---

## 2.4 解析与策略执行时序（推荐）

1. runner 流式输出时，`tee` 同步把每一行送入 `ToolEventParser`
2. 若解析到 `tool.request`：
   - 进入 span `mem_codecli.policy.decide`
   - `PolicyEngine.decide()` 返回 allow/deny/ask
   - deny/ask 结果以 **事件** 打到日志（审计），并（如果可行）回写给 codecli（见下）
3. 若解析到 `tool.result`：
   - 记录 outcome（ok/err），并可更新 hit/validate 信号（可选）

### 关于“回写给 codecli”

是否能“阻止工具执行”取决于 codecli 运行时是否支持外部审批回调。为了不把 mem-codecli 与 codecli 深度绑定，建议：

- **最小实现**：只做审计与提示（deny 时在 stdout 输出警告并返回 exit code 40）
- **增强实现**：约定一个 stdin 控制通道（例如 JSONL command）让 codecli 等待审批后再执行工具

如果你要支持增强实现，建议再定义一套 **control JSONL**（`type="policy.decision"`）通过 stdin 回传给 codecli；这里先不展开，避免把协议做大。

---

# 3. 与 PolicyEngine 的对接约束（重要）

从 `tool.request` 映射到 `PolicyEngine`：

- `tool`：直接使用事件中的 `tool`
- `action`：严格映射枚举 read/write/net/exec（未知 action → `ParseError::SchemaMismatch` 或归类为 `exec` 并提高风险）
- `args`：进入 policy 前应进行 **redact**（至少 basic），避免审计日志泄漏 secrets
- `rationale`：可选，用于 ask 模式时给用户展示

---

# 4. 最小落地清单（你实现时按这个收口）

1. `thiserror` 枚举按上述分层建立；CLI 层做 exit code 映射
2. 所有顶层动作都有 span：`mem_codecli.run` + 子 spans
3. 实现 `JsonlToolEventParser`（前缀 + 纯 JSON 两模式）
4. `ToolEventParser` 解析失败只记 `parse_error_count`，不得阻塞流式输出
5. `policy.decide` 的结果写结构化 event（含 tool/action/decision/rule\_id）

---