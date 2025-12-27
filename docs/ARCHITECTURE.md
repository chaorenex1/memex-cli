## 一、总体目标（一句话）

> 用 **Rust** 包装 `codecli`，在**不破坏原有 CLI 体验**的前提下，引入\
> **记忆检索 / 记忆沉淀 / 工具审批 / 可观测 / 可恢复** 的外部控制层。

---

## 二、整体架构（分层模型）

```
┌──────────────────────────────┐
│          User / CI           │
└─────────────┬────────────────┘
              │ stdin/stdout/stderr
┌─────────────▼────────────────┐
│        mem-codecli (Rust)     │
│                                │
│  ┌──────── Orchestrator ─────┐ │
│  │                            │ │
│  │  Context → Search → Inject │ │
│  │        ↓                   │ │
│  │     Runner (codecli)       │ │
│  │        ↓                   │ │
│  │  ToolEvent → Policy        │ │
│  │        ↓                   │ │
│  │  Gatekeeper → Candidate    │ │
│  │        ↓                   │ │
│  │  Hit / Validate            │ │
│  └────────────────────────────┘ │
│                                │
│  stdout/stderr tee + stdin ctl │
└─────────────┬────────────────┘
              │
┌─────────────▼────────────────┐
│           codecli             │
│  (LLM + tools + agent logic)  │
└──────────────────────────────┘
```

---

## 三、核心能力拆解（做什么）

### 1. Runner（进程执行 + 流式 tee）

- 启动 `codecli` 子进程
- **stdout / stderr 实时透传**
- **旁路 capture（ring buffer）**
- **stdin 专用为控制通道（JSONL）**
- 跨平台（Unix / Windows）

### 2. Memory 接驳

- `search`：生成上下文，注入 prompt
- `hit`：记忆命中回传
- `candidate`：新知识沉淀
- `validate`：质量闭环

### 3. Tool 审批（Policy）

- 从 stdout/stderr 解析 `tool.request`
- allow / deny / ask
- **可阻断执行（stdin 控制）**

### 4. Gatekeeper

- 判断是否值得写入记忆
- 防重复、防泄密、防一次性信息
- 产出结构化 candidate

### 5. Observability

- 全链路 tracing span
- 结构化事件
- 诊断包（失败时）

### 6. Error / Recovery

- 明确的 error 分层
- Fail-Closed 默认策略
- 断线 / 半关闭 / 卡死 可控恢复

---

## 四、模块边界（怎么拆）

### 1. 关键 trait 边界

| 模块                | 责任         | 是否可 mock |
| ----------------- | ---------- | -------- |
| `Runner`          | 进程管理 + I/O | ✅        |
| `MemoryClient`    | HTTP API   | ✅        |
| `Gatekeeper`      | 质量判断       | ✅        |
| `PolicyEngine`    | 工具审批       | ✅        |
| `ToolEventParser` | JSONL 解析   | ✅        |
| `Approver`        | 人工审批       | ✅        |

→ **所有复杂逻辑都在 trait 后面，CLI 只是 wiring**

---

## 五、stdout / stderr 流式 tee（关键实现点）

### 1. 并发模型

- 两个 reader task：stdout / stderr
- 一个 writer（parent stdout / stderr）
- 一个 ring buffer（尾部字节）
- 一个 wait task（exit status）

### 2. 行为保证

- **永不阻塞主输出**
- capture 只保留最后 N bytes
- UTF-8 解码只在“需要字符串”的地方做（lossy）

### 3. 退出码归一化

- Unix：
  - 正常 exit → code
  - signal → `128 + signal`
- Windows：
  - 只使用 exit code
  - signal 仅用于日志

---

## 六、stdin 控制协议（JSONL）

### 1. 用途

> **mem-codecli 控制 codecli 是否执行工具**

### 2. 最小命令集

- `policy.decision`（allow / deny）
- `policy.abort`（终止会话）
- （可选）`policy.ping`

### 3. 关联规则

- `policy.decision.id == tool.request.id`
- 幂等、单次发送

---

## 七、ToolEvent JSONL 协议（stdout/stderr）

### 1. 核心事件

- `tool.request`
- `tool.result`
- `tool.progress`（可选）

### 2. 识别规则

- 推荐前缀：`@@MEM_TOOL_EVENT@@ {json}`
- 或纯 JSON 且包含 `{v, type}`

### 3. 不阻塞原则

- 解析失败 ≠ 失败执行
- 只记数、打日志、继续跑

---

## 八、错误分层（thiserror）

### 1. 分层结构

```
CliError
 └── CoreError
      └── DependencyError
           ├── RunnerError
           ├── MemoryError
           ├── PolicyError
           ├── ParseError
           └── RedactError
```

### 2. 退出码语义（稳定）

| Exit Code | 含义          |
| --------- | ----------- |
| 0         | 成功          |
| 10        | CLI / 参数错误  |
| 11        | 配置错误        |
| 20        | Runner 失败   |
| 30        | Memory 服务失败 |
| 31        | Memory 鉴权失败 |
| 40        | Policy 拒绝   |
| 50        | 内部错误        |

---

## 九、Fail-Closed 控制通道恢复策略（重点）

### 1. stdin 写失败

- **默认：立刻 Abort**
- 不能“继续跑”，避免失控执行

### 2. stdout/stderr 断线

- 单通道断线：降级
- 双通道断线 + 子进程存活：视为卡死 → Abort

### 3. 子进程卡死检测

- pending decision 超时
- pending exec 超时
- 长时间无 output / event
- → 软探测 → Abort → 强杀

---

## 十、信号转发（跨平台）

### Unix

- 捕获 SIGINT / SIGTERM
- 转发给 child / 进程组
- TERM → KILL 升级

### Windows

- 优先 `CTRL_BREAK_EVENT`
- 不可用则 `TerminateProcess`
- 文档明确：Windows 可能非优雅

---

## 十一、推荐最小实现顺序（非常重要）

### Phase 1（MVP，可跑）

1. Runner + stdout/stderr tee
2. Memory search + inject
3. Gatekeeper + candidate
4. Error 分层 + exit code

### Phase 2（安全）

5. ToolEventParser
6. PolicyEngine（allow / deny）
7. stdin `policy.decision`

### Phase 3（稳定）

8. Fail-Closed + Abort Sequence
9. Hang Detection
10. Diagnostics bundle

---

## 十二、一句话总结（设计哲学）

> **mem-codecli 不是“替代 codecli”，而是一个\
> 可审计、可恢复、可进化的外部控制壳。**
>
> - stdout/stderr：**永远真实**
> - stdin：**唯一控制点**
> - 失败：**默认关闭（Fail-Closed）**
> - 设计：**trait 优先，协议优先**

---