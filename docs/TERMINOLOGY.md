# 术语定义 (Terminology)

本文档明确 memex-cli 项目中的核心术语，消除歧义。

## run_id

**定义**: 单次执行的唯一标识符（通常是 UUID）

**用途**:
- 关联所有 wrapper events 和 tool events
- Replay/Resume 功能的主键
- 持久化到 `run.events.jsonl` 文件
- 作为执行追踪和审计的核心标识

**数据结构**:
- `WrapperEvent.run_id: Option<String>` - 包装器事件标识
- `ToolEvent.run_id: Option<String>` - 工具事件标识
- `ReplayRun.run_id: String` - Replay 会话标识
- `RunnerResult.run_id: Option<String>` - Runner 返回的标识

**生成时机**:
- 默认：CLI 在 `main.rs` 中生成 UUID
- Resume 模式：从 `--resume-id` 参数获取
- Backend 返回：某些 backend 可能返回自己生成的 ID（作为 effective run_id）

**示例**:
```bash
# 执行命令，run_id 自动生成
memex run "echo hello"
# run_id: a1b2c3d4-5e6f-7890-abcd-ef1234567890

# Replay 使用 run_id
memex replay a1b2c3d4-5e6f-7890-abcd-ef1234567890

# Resume 继续执行
memex resume a1b2c3d4-5e6f-7890-abcd-ef1234567890
```

---

## session_id

**定义**: 在不同上下文中有不同含义的标识符

### 用途 1: HTTP 服务器实例标识符

**语义**: 标识 HTTP 服务器进程（**非**单次执行）

**特点**:
- 一个 HTTP 服务器实例可以处理多个 run
- 写入状态文件: `~/.memex/servers/memex-{session_id}.state`
- 用于 Python hooks 定位服务器状态

**数据结构**:
- `AppState.session_id: String` - HTTP 服务器实例 ID
- `HealthResponse.session_id: String` - 健康检查返回的服务器 ID

**示例**:
```bash
# 启动 HTTP 服务器，指定 session_id
memex http-server --session-id my-server-123

# 状态文件位置
~/.memex/servers/memex-my-server-123.state

# 健康检查返回
curl http://localhost:8080/health
{"session_id": "my-server-123", ...}
```

### 用途 2: run_id 的别名（CLI 层）

**语义**: 在 CLI 参数层与 `run_id` 等价

**使用场景**:
- `RecordSessionArgs.session_id` - 实际作为 run_id 传递给 memory service
- 与外部系统集成（如 Claude 的 session_id）
- 向后兼容性考虑

**内部处理**: CLI 层接受 `session_id` 参数后，内部统一转换为 `run_id` 传递给核心引擎

**示例**:
```bash
# Memory 命令中的 session_id（实际是 run_id）
memex memory record-session --session-id a1b2c3d4-... --transcript "..."
```

### 用途 3: Backend 输出兼容

**语义**: 某些 backend 返回 `session_id` 而非 `run_id`

**处理逻辑**: `core/src/tool_event/run_id_extract.rs` 支持从多种字段名提取：
```rust
for key in ["session_id", "sessionId", "run_id", "runId", "thread_id"]
```

**示例**:
- Gemini backend 返回: `{"type":"init", "session_id":"..."}`
- 核心引擎将其作为 `effective run_id` 使用

---

## 术语选择指南

**何时使用 run_id**:
- ✅ Core 引擎层（`core/src/engine/*`, `core/src/runner/*`）
- ✅ 事件系统（`core/src/tool_event/*`, `core/src/events_out/*`）
- ✅ Replay/Resume 功能（`core/src/replay/*`）
- ✅ 文档和注释中的主要术语
- ✅ 数据结构字段名

**何时使用 session_id**:
- ✅ HTTP 服务器上下文（`cli/src/http/*`）- 指服务器实例 ID
- ✅ CLI 参数层（`cli/src/commands/cli.rs`）- 作为 run_id 别名
- ✅ Backend 输出解析（`run_id_extract.rs`）- 兼容多种格式
- ✅ 与外部系统集成文档

---

## 代码示例

### 核心引擎使用 run_id

```rust
// core/src/engine/run.rs
pub struct RunWithQueryArgs<'a> {
    pub run_id: String,  // ← 主要术语
    pub project_id: &'a str,
    // ...
}

// 执行后获取 effective run_id
let effective_run_id = run_result.run_id.clone();
```

### HTTP 服务器使用 session_id

```rust
// cli/src/http/state.rs
pub struct AppState {
    pub session_id: String,  // ← 服务器实例 ID
    // ...
}
```

### CLI 层 session_id 作为 run_id 别名

```rust
// cli/src/commands/cli.rs
pub struct RecordSessionArgs {
    /// Session ID (internally treated as run_id for execution tracking)
    #[arg(long)]
    pub session_id: String,  // ← 内部转换为 run_id
    // ...
}
```

---

## 历史背景

### 为何存在两个术语？

1. **Backend 兼容性**: 不同 AI backend（CodeCli, AiService, Gemini）使用不同的术语
2. **HTTP 服务器语义**: HTTP server 需要独立的实例标识符（与单次执行无关）
3. **外部集成**: 与 Claude API 等外部系统集成时使用 session_id

### 设计原则

- **核心统一**: 内部数据模型和持久化格式统一使用 `run_id`
- **边界适配**: 在系统边界（CLI 参数、HTTP API、Backend 输出）支持 `session_id`
- **文档清晰**: 明确区分两者的语义和使用场景

---

## 常见误解

### ❌ 误解 1: session_id 和 run_id 是不同的概念

**事实**: 在核心执行引擎层，两者是**同义词**（见 `core/src/engine/run.rs:100` 注释）。差异仅在于命名习惯和来源。

### ❌ 误解 2: HTTP server 的 session_id 等于 run_id

**事实**: HTTP server 的 `session_id` 是**服务器实例标识符**，一个服务器可以处理多个 run，它们有不同的 run_id。

### ❌ 误解 3: 应该全局统一为单一术语

**事实**: HTTP 服务器层的 `session_id` 有独立语义，不应改为 `run_id`。分层使用是合理的设计。

---

## 相关文件

**核心数据结构**:
- `core/src/tool_event/wrapper_event.rs` - WrapperEvent.run_id
- `core/src/tool_event/model.rs` - ToolEvent.run_id
- `core/src/runner/types.rs` - RunnerResult.run_id
- `core/src/replay/model.rs` - ReplayRun.run_id

**HTTP 服务器**:
- `cli/src/http/state.rs` - AppState.session_id
- `cli/src/http/models.rs` - HealthResponse.session_id

**CLI 参数**:
- `cli/src/commands/cli.rs` - RecordSessionArgs, HttpServerArgs

**Backend 兼容**:
- `core/src/tool_event/run_id_extract.rs` - 多字段名支持

**执行流程**:
- `core/src/engine/run.rs` - 核心执行逻辑
- `cli/src/main.rs` - run_id 生成

---

## 参考资料

- 架构文档: `CLAUDE.md` - Project Overview 章节
- 协议文档: `docs/STDIO_PROTOCOL.md` - 使用 run_id
- 集成任务: `integration_task.md` - session_id 不一致问题讨论
