# Run ID / Resume / Env / Perf 说明

本文档面向 `memex-cli` 的使用者与维护者，说明以下能力：

- `run_id` 与外部 CLI `session_id` 的对接与一致性保证
- `resume`（继续会话）参数如何映射到不同后端（codex/claude/gemini）
- 环境变量透传到子进程（后端 CLI）
- tool event 解析（prefixed JSONL + stream-json）
- 在 tests 中运行压测（perf）

---

## 1. 概念：run_id vs session_id

- `memex-cli` 内部需要一个 **run_id** 来关联：wrapper events、tool events、gatekeeper 决策等。
- 外部 CLI（例如 gemini）在 `stream-json` 输出里会提供 **session_id**（示例：`{"type":"init","session_id":"..."}`）。

### 1.1 当前策略（优先 session_id）

1. `memex-cli` 启动时仍会先生成/接收一个临时的 run_id（例如 `uuid` 或 `resume --run-id` 传入）。
2. 运行时从子进程输出中 **best-effort** 抽取 `session_id/run_id`。
3. 一旦发现 `session_id`，将其作为 **effective_run_id**：
   - 写入每条 `ToolEvent.run_id`
   - 用于 `run.end` / `gatekeeper.decision` / `tee.drop` 等 wrapper events

### 1.2 wrapper events 的 ID 一致性

为了避免出现：
- `run.start` 先写了 UUID
- 后续事件又写了 session_id

当前实现会将“早期 wrapper events”缓存，等拿到 `effective_run_id` 后统一写出，从而保证同一次运行的 wrapper events `run_id` 一致。

注意：这样会导致 `run.start` 的写出时机变晚（但 `ts` 仍是创建时刻）。如果后续需要“实时 run.start”，建议引入 `run.id_map`（UUID → session_id）映射事件，而不是让同一条事件流里 run_id 前后变化。

---

## 2. Resume：继续会话

CLI 提供：

- `memex-cli resume --run-id <ID> ...`

其中 `<ID>` 会作为 `recover_run_id` 传入，并进一步传给后端策略作为 `resume_id`。

### 2.1 不同后端的 resume 参数映射

后端策略会按可执行文件名（包含关键字）做 best-effort 映射：

- **codex**：`codex exec ... resume <id> <prompt>`
- **claude**：追加 `-r <id>`
- **gemini**：追加 `-r <id>`（例如 `-r latest` 或具体 session id）

> 说明：不同版本 CLI 的 resume 参数可能有差异；当前实现以你提供的样例为准，后续可以按真实 CLI 行为再收敛。

---

## 3. 环境变量透传（子进程）

`run` / `resume` 子命令支持多次指定：

- `--env KEY=VALUE`

行为：

- 以当前进程的 `std::env::vars()` 作为 base
- 对每个 `--env` 进行覆盖/追加（同名 KEY 以 CLI 指定为准）
- 最终传入后端进程的启动参数（用于 API key、代理、调试开关等）

---

## 4. tool event 解析（prefixed JSONL + stream-json）

### 4.1 两种输入格式

1. **Prefixed JSONL**：项目自定义前缀 `@@MEM_TOOL_EVENT@@...` 的 JSON 行
2. **stream-json**：外部 CLI 输出的“纯 JSON 行”（例如 gemini 的 `tool_use/tool_result`、codex 的 `item.started/item.completed`）

当前通过 `CompositeToolEventParser` 兼容两者：

- 优先尝试 prefixed parser
- 再尝试 stream-json parser

### 4.2 为什么 parser 里 `ToolEvent.run_id` 是 None

`stream_json.rs` 的 `parse_line` 返回 `ToolEvent` 时会设置 `run_id: None`，原因是：

- parser 只负责“把一行映射成事件”
- `run_id/session_id` 属于“关联信息”，在 runtime 层统一补齐

runtime 会在 `observe_line` 中：

- 从 JSON 行 best-effort 抽取 `session_id/run_id`
- 对产出的 `ToolEvent` 自动填充 `run_id`

---

## 5. tests 中的压测（perf）

压测放在 `memex-core` 的集成测试里，标记为 `#[ignore]`，默认不会跑。

### 5.1 编译

```bash
cargo test -p memex-core --test perf_tool_event_runtime
```

### 5.2 执行（打印吞吐）

```bash
cargo test -p memex-core --test perf_tool_event_runtime -- --ignored --nocapture
```

### 5.3 调整规模

```bash
set MEMEX_PERF_LINES=200000
cargo test -p memex-core --test perf_tool_event_runtime -- --ignored --nocapture
```

输出会包含：

- lines/s（每秒处理行数）
- events/s（每秒产生事件数）

注意：压测不对耗时做断言，避免 CI/不同机器抖动。
