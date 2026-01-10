# Memex CLI Stdio Protocol Specification

Version: 1.0.0

## Overview

本协议定义了 `memex-cli` 通过标准输入输出（stdio）进行任务传递和结果返回的格式规范。设计目标：

- **安全传输** - 原始文本无需转义，支持任意字符
- **多任务支持** - 单次输入可定义多个任务及依赖关系
- **流式输出** - 实时返回执行进度和结果
- **跨平台兼容** - Windows/Linux/macOS 统一格式

---

## 1. 输入协议（stdin）

### 1.1 基本结构

```
---TASK---
<metadata>
---CONTENT---
<content>
---END---
```

### 1.2 完整语法

```abnf
input           = 1*task-block
task-block      = task-marker metadata content-marker content [end-marker]

task-marker     = "---TASK---" LF
content-marker  = "---CONTENT---" LF
end-marker      = "---END---" LF

metadata        = 1*metadata-line
metadata-line   = key ":" SP value LF
key             = 1*ALPHA
value           = *VCHAR

content         = *OCTET  ; 任意字节，无需转义

LF              = %x0A    ; Unix 换行
CRLF            = %x0D %x0A  ; Windows 换行（兼容）
```

### 1.3 元数据字段

| 字段 | 必填 | 类型 | 说明 |
|------|------|------|------|
| `id` | ✅ | string | 任务唯一标识符，用于依赖引用（见 1.3.1 ID 规则） |
| `backend` | ✅ | enum | AI 后端：`codex` \| `claude` \| `gemini` |
| `workdir` | ✅ | path | 工作目录路径（绝对路径或相对路径） |
| `model` | ❌ | string | 模型名称，如 `gpt-5.2`、`gpt-5.1-codex-max` |
| `model-provider` | ❌ | string | 模型提供商（codex 专用） |
| `dependencies` | ❌ | string | 依赖的任务 ID，逗号分隔 |
| `stream-format` | ❌ | enum | 输出格式：`text` \| `jsonl`，默认 `text` |
| `timeout` | ❌ | integer | 超时时间（秒），默认 300 |
| `retry` | ❌ | integer | 重试次数，默认 0 |
| `files` | ❌ | string | 引用文件路径，逗号分隔（见 1.3.2 文件引用规则） |
| `files-mode` | ❌ | enum | 文件处理模式：`embed` \| `ref` \| `auto`，默认 `auto` |
| `files-encoding` | ❌ | enum | 文件编码：`utf-8` \| `base64` \| `auto`，默认 `auto` |

### 1.3.1 Task ID 规则

#### 格式规范

```abnf
task-id         = identifier *("." identifier)
identifier      = (ALPHA / "_") *(ALPHA / DIGIT / "_" / "-")

; 长度限制
min-length      = 1
max-length      = 128
```

### 1.3.2 文件引用规则

#### 基本语法

```abnf
files           = file-path *("," file-path)
file-path       = relative-path / absolute-path / glob-pattern
relative-path   = "./" *VCHAR / "../" *VCHAR / filename
absolute-path   = "/" *VCHAR / drive-letter ":/" *VCHAR   ; Unix / Windows
glob-pattern    = *VCHAR ("*" / "?" / "[" *VCHAR "]") *VCHAR
```

#### 文件引用字段

| 字段 | 说明 |
|------|------|
| `files` | 文件路径列表，逗号分隔 |
| `files-mode` | 处理模式：`embed`(嵌入内容)、`ref`(仅引用路径)、`auto`(自动判断) |
| `files-encoding` | 编码方式：`utf-8`、`base64`、`auto` |

#### 处理模式详解

| 模式 | 行为 | 适用场景 |
|------|------|----------|
| `embed` | 读取文件内容，嵌入到 prompt 中（文件 ≤ 50KB）<br/>文件 > 50KB 时自动降级为 `ref` | 代码审查、文档分析（小文件） |
| `ref` | 仅传递文件路径和元信息，不读取内容 | 大文件、路径引用 |
| `auto` | **永远使用路径引用（推荐）** | 默认模式，避免读取大量文件内容 |

**新规则（v1.1.0）**：
- `auto` 模式：永远使用 `ref`（不读取文件内容）
- `embed` 模式：文件 ≤ 50KB → 嵌入内容；文件 > 50KB → 自动降级为 `ref`

#### 编码方式详解

| 编码 | 行为 | 适用类型 |
|------|------|----------|
| `utf-8` | UTF-8 文本读取 | `.txt`, `.md`, `.py`, `.js`, `.json`, `.yaml` 等 |
| `base64` | Base64 编码 | `.png`, `.jpg`, `.pdf`, `.zip` 等二进制文件 |
| `auto` | 根据文件类型自动选择 | 默认模式 |

#### 路径格式示例

```yaml
# 单个文件
files: ./src/main.py

# 多个文件
files: ./src/main.py, ./src/utils.py, ./README.md

# 绝对路径
files: /home/user/project/config.yaml

# Windows 路径
files: C:/Users/dev/project/main.py

# Glob 模式
files: ./src/*.py
files: ./src/**/*.ts
files: ./tests/test_*.py

# 混合使用
files: ./src/main.py, ./src/**/*.py, ../shared/utils.py
```

#### 使用示例

**示例 1：代码审查（嵌入模式）**

```
---TASK---
id: code-review
backend: claude
files: ./src/auth.py, ./src/user.py
files-mode: embed
---CONTENT---
审查以上代码文件，检查：
1. 安全漏洞
2. 代码风格
3. 性能问题
---END---
```

处理后的 prompt：
```
审查以上代码文件，检查：
1. 安全漏洞
2. 代码风格
3. 性能问题

---FILE: ./src/auth.py---
import hashlib
def authenticate(user, password):
    ...
---END FILE---

---FILE: ./src/user.py---
class User:
    def __init__(self, name):
        ...
---END FILE---
```

**示例 2：图片分析（Base64 编码）**

```
---TASK---
id: image-analyze
backend: gemini
files: ./screenshot.png
files-mode: embed
files-encoding: base64
---CONTENT---
分析这张截图中的 UI 设计问题
---END---
```

**示例 3：大文件引用（引用模式）**

```
---TASK---
id: process-data
backend: codex
files: ./data/large_dataset.csv
files-mode: ref
workdir: /home/user/project
---CONTENT---
处理数据文件，生成统计报告
---END---
```

**示例 4：Glob 模式批量引用**

```
---TASK---
id: test-all
backend: codex
files: ./tests/**/*.py
files-mode: embed
---CONTENT---
检查所有测试文件，确保覆盖率达标
---END---
```

#### 文件内容嵌入格式

当 `files-mode: embed` 时，文件内容按以下格式插入：

**文本文件：**
```
---FILE: <filepath>---
<file content>
---END FILE---
```

**二进制文件（Base64）：**
```
---FILE: <filepath> [base64]---
<base64 encoded content>
---END FILE---
```

**带元信息：**
```
---FILE: <filepath>---
<!-- size: 1234 bytes, modified: 2026-01-09T10:00:00Z, encoding: utf-8 -->
<file content>
---END FILE---
```

#### 安全限制

| 限制 | 值 | 说明 |
|------|-----|------|
| 单文件最大 | 10 MB | 超过则自动切换为 `ref` 模式 |
| 总文件数 | 100 | 单任务最多引用文件数 |
| 总大小 | 50 MB | 所有嵌入文件总大小 |
| 路径遍历 | 禁止 | 不允许 `../../etc/passwd` 等 |
| 符号链接 | 可配置 | 默认跟随，可禁用 |

#### 错误处理

| 错误 | 代码 | 说明 |
|------|------|------|
| FILE_NOT_FOUND | 60 | 文件不存在 |
| FILE_ACCESS_DENIED | 61 | 无读取权限 |
| FILE_TOO_LARGE | 62 | 文件超过大小限制 |
| TOO_MANY_FILES | 63 | 文件数超过限制 |
| INVALID_PATH | 64 | 无效路径格式 |
| PATH_TRAVERSAL | 65 | 检测到路径遍历攻击 |
| GLOB_NO_MATCH | 66 | Glob 模式无匹配文件 |
| ENCODING_ERROR | 67 | 文件编码错误 |

### 1.3.3 Task ID 命名规则

#### 命名规则

| 规则 | 说明 | 示例 |
|------|------|------|
| 字符集 | 字母、数字、下划线、连字符、点 | `task-1`, `step_2`, `phase.1` |
| 起始字符 | 必须以字母或下划线开头 | ✅ `task1` ❌ `1task` ❌ `-task` |
| 大小写 | 区分大小写，推荐小写 | `Task1` ≠ `task1` |
| 长度 | 1-128 字符 | - |
| 唯一性 | 同一输入中不可重复 | - |
| 保留字 | 不可使用系统保留字 | 见下方列表 |

#### 时间戳格式

Task ID 支持嵌入时间戳以确保唯一性和可追溯性：

| 格式 | 模式 | 示例 |
|------|------|------|
| 紧凑型 | `{prefix}-{YYYYMMDDHHmmss}` | `task-20260109143052` |
| 带毫秒 | `{prefix}-{YYYYMMDDHHmmss}-{ms}` | `task-20260109143052-123` |
| Unix 时间戳 | `{prefix}-{unix_seconds}` | `task-1736430652` |
| Unix 毫秒 | `{prefix}-{unix_ms}` | `task-1736430652123` |
| ISO 简化 | `{prefix}-{YYYY}-{MM}-{DD}T{HH}-{mm}` | `task-2026-01-09T14-30` |
| 日期 + 序号 | `{prefix}-{YYYYMMDD}-{seq}` | `task-20260109-001` |

#### 时间戳命名示例

```
# 紧凑时间戳（推荐）
task-20260109143052
build-20260109143052
deploy-20260109143052

# 带毫秒（高并发场景）
task-20260109143052-123
task-20260109143052-456

# Unix 时间戳
task-1736430652
task-1736430652123

# 日期 + 序号（人类可读）
task-20260109-001
task-20260109-002
build-20260109-001

# 前缀 + 时间戳 + 后缀
auth-20260109143052-design
auth-20260109143052-implement
db-20260109143052-migrate

# 层级结构 + 时间戳
auth.20260109143052.design
auth.20260109143052.implement
```

#### 自动生成 Task ID

当未指定 `id` 时，系统自动生成：

```
格式: task-{YYYYMMDDHHmmss}-{random4}
示例: task-20260109143052-a1b2
```

生成规则：
- 前缀：`task`
- 时间戳：14 位紧凑格式
- 随机后缀：4 位小写字母数字，避免同一秒内冲突

#### 时间戳验证正则

```regex
# 紧凑型 (YYYYMMDDHHmmss)
^[a-zA-Z_][a-zA-Z0-9_\-\.]*-\d{14}(-\d{1,3})?$

# Unix 时间戳 (10位秒 或 13位毫秒)
^[a-zA-Z_][a-zA-Z0-9_\-\.]*-\d{10,13}$

# 日期+序号
^[a-zA-Z_][a-zA-Z0-9_\-\.]*-\d{8}-\d{3,}$
```

#### 各语言生成示例

**Go:**
```go
import (
    "fmt"
    "math/rand"
    "time"
)

func GenerateTaskID(prefix string) string {
    ts := time.Now().Format("20060102150405")
    suffix := fmt.Sprintf("%04x", rand.Intn(0xFFFF))
    if prefix == "" {
        prefix = "task"
    }
    return fmt.Sprintf("%s-%s-%s", prefix, ts, suffix)
}

// 输出: task-20260109143052-a1b2
```

**Rust:**
```rust
use chrono::Utc;
use rand::Rng;

fn generate_task_id(prefix: Option<&str>) -> String {
    let ts = Utc::now().format("%Y%m%d%H%M%S");
    let suffix: u16 = rand::thread_rng().gen();
    let prefix = prefix.unwrap_or("task");
    format!("{}-{}-{:04x}", prefix, ts, suffix)
}

// 输出: task-20260109143052-a1b2
```

**TypeScript:**
```typescript
function generateTaskId(prefix: string = 'task'): string {
  const now = new Date();
  const ts = now.toISOString()
    .replace(/[-:T]/g, '')
    .slice(0, 14);
  const suffix = Math.random().toString(36).slice(2, 6);
  return `${prefix}-${ts}-${suffix}`;
}

// 输出: task-20260109143052-a1b2
```

#### 保留字（禁止使用）

```
_root, _start, _end, _all, _none, _self, _parent
__internal__, __system__, __meta__
```

#### 推荐命名模式

```
# 模式 1: 动作-对象
design-api
implement-auth
test-models
deploy-service

# 模式 2: 阶段编号
step-1-init
step-2-process
step-3-validate

# 模式 3: 层级结构（用点分隔）
auth.design
auth.implement
auth.test
db.schema
db.migrate

# 模式 4: 带序号
task-001
task-002
subtask-001a
subtask-001b

# 模式 5: 语义化
fetch-user-data
transform-json
save-to-db
send-notification
```

#### 验证正则表达式

```regex
^[a-zA-Z_][a-zA-Z0-9_\-\.]{0,127}$
```

#### 各语言验证示例

**Go:**
```go
import "regexp"

var taskIDRegex = regexp.MustCompile(`^[a-zA-Z_][a-zA-Z0-9_\-\.]{0,127}$`)
var reserved = map[string]bool{
    "_root": true, "_start": true, "_end": true,
    "_all": true, "_none": true, "_self": true, "_parent": true,
}

func ValidateTaskID(id string) error {
    if id == "" {
        return errors.New("task id cannot be empty")
    }
    if len(id) > 128 {
        return errors.New("task id exceeds 128 characters")
    }
    if reserved[id] || strings.HasPrefix(id, "__") {
        return fmt.Errorf("task id '%s' is reserved", id)
    }
    if !taskIDRegex.MatchString(id) {
        return fmt.Errorf("task id '%s' contains invalid characters", id)
    }
    return nil
}
```

**Rust:**
```rust
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    static ref TASK_ID_REGEX: Regex = 
        Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_\-\.]{0,127}$").unwrap();
}

const RESERVED: &[&str] = &["_root", "_start", "_end", "_all", "_none", "_self", "_parent"];

fn validate_task_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("task id cannot be empty".into());
    }
    if id.len() > 128 {
        return Err("task id exceeds 128 characters".into());
    }
    if RESERVED.contains(&id) || id.starts_with("__") {
        return Err(format!("task id '{}' is reserved", id));
    }
    if !TASK_ID_REGEX.is_match(id) {
        return Err(format!("task id '{}' contains invalid characters", id));
    }
    Ok(())
}
```

**TypeScript:**
```typescript
const TASK_ID_REGEX = /^[a-zA-Z_][a-zA-Z0-9_\-\.]{0,127}$/;
const RESERVED = new Set(['_root', '_start', '_end', '_all', '_none', '_self', '_parent']);

function validateTaskId(id: string): void {
  if (!id) throw new Error('task id cannot be empty');
  if (id.length > 128) throw new Error('task id exceeds 128 characters');
  if (RESERVED.has(id) || id.startsWith('__')) {
    throw new Error(`task id '${id}' is reserved`);
  }
  if (!TASK_ID_REGEX.test(id)) {
    throw new Error(`task id '${id}' contains invalid characters`);
  }
}
```

#### 依赖引用规则

`dependencies` 字段引用其他任务 ID：

```
---TASK---
id: task-3
dependencies: task-1, task-2
---CONTENT---
...
```

| 规则 | 说明 |
|------|------|
| 分隔符 | 逗号 `,`（逗号后空格可选） |
| 顺序 | 无特殊含义，并行检查 |
| 自引用 | 禁止（会报 CIRCULAR_DEPENDENCY 错误） |
| 前向引用 | 允许（引用后定义的任务） |
| 不存在引用 | 报 DEPENDENCY_ERROR 错误 |

**有效示例：**
```
dependencies: task-1
dependencies: task-1, task-2
dependencies: task-1,task-2,task-3
dependencies: auth.design, db.schema
```

**无效示例：**
```
dependencies: task-1; task-2     # 错误分隔符
dependencies: task-1 task-2      # 缺少分隔符
dependencies: self               # 自引用
dependencies: 1-task             # 无效 ID 格式
```

### 1.4 基本输入示例

**单任务：**

```
---TASK---
id: hello-world
backend: codex
workdir: /home/user/project
---CONTENT---
编写 Hello World 程序
---END---
```

**多任务（带依赖）：**

```
---TASK---
id: design
backend: gemini
workdir: /home/user/project
---CONTENT---
设计 API 接口
---END---

---TASK---
id: implement
backend: codex
workdir: /home/user/project
dependencies: design
---CONTENT---
实现 API
---END---
```

> 📖 完整示例请参阅 [STDIO_EXAMPLES.md](./STDIO_EXAMPLES.md)

### 1.5 内容转义规则

**无需转义** - `---CONTENT---` 与 `---END---`（或下一个 `---TASK---`）之间的内容完全原样保留。

唯一限制：内容中不能出现独占一行的 `---END---` 或 `---TASK---`。如需包含这些字符串，可：

```
方法1：添加前缀空格
 ---END---

方法2：使用 HTML 实体
&#45;&#45;&#45;END&#45;&#45;&#45;

方法3：拆分字符串
---EN + D---
```

---

## 2. 输出协议（stdout）

### 2.1 输出格式选择

根据 `stream-format` 参数选择输出格式：

| 格式 | 用途 | 特点 |
|------|------|------|
| `text` | 人类阅读 | 直接输出文本，适合终端显示 |
| `jsonl` | 程序处理 | 每行一个 JSON 对象，适合解析和存储 |

### 2.2 JSONL 输出格式

每行一个独立的 JSON 对象，包含以下字段：

```typescript
interface OutputEvent {
  v: 1;                          // 协议版本
  type: EventType;               // 事件类型
  ts: string;                    // ISO 8601 时间戳
  run_id: string;                // 运行 ID (UUID)
  task_id?: string;              // 任务 ID（多任务时）
  
  // 根据 type 不同，以下字段可选
  action?: string;               // 执行的动作
  args?: object;                 // 动作参数
  output?: string;               // 输出内容
  error?: string;                // 错误信息
  code?: number;                 // 错误代码
  progress?: number;             // 进度 0-100
  metadata?: object;             // 额外元数据
}

type EventType = 
  | "run.start"           // 运行开始
  | "run.end"             // 运行结束
  | "task.start"          // 任务开始
  | "task.end"            // 任务结束
  | "assistant.thinking"  // 思考中
  | "assistant.output"    // 输出内容
  | "assistant.action"    // 执行动作
  | "tool.call"           // 工具调用
  | "tool.result"         // 工具结果
  | "error"               // 错误
  | "warning"             // 警告
  | "info"                // 信息
  | "debug";              // 调试
```

### 2.3 事件类型详解

#### 2.3.1 run.start

运行开始事件，包含全局信息。

```jsonl
{"v":1,"type":"run.start","ts":"2026-01-09T10:00:00.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","metadata":{"total_tasks":3,"backend":"codex","model":"gpt-5.2"}}
```

#### 2.3.2 task.start

单个任务开始。

```jsonl
{"v":1,"type":"task.start","ts":"2026-01-09T10:00:01.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"task-1-design","metadata":{"dependencies":[],"backend":"gemini"}}
```

#### 2.3.3 assistant.thinking

模型思考过程（可选输出）。

```jsonl
{"v":1,"type":"assistant.thinking","ts":"2026-01-09T10:00:02.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"task-1-design","output":"分析用户认证系统需求..."}
```

#### 2.3.4 assistant.output

模型输出内容（流式）。

```jsonl
{"v":1,"type":"assistant.output","ts":"2026-01-09T10:00:03.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"task-1-design","output":"CREATE TABLE users (\n  id SERIAL PRIMARY KEY,\n  ..."}
```

#### 2.3.5 assistant.action

执行动作（如文件操作）。

```jsonl
{"v":1,"type":"assistant.action","ts":"2026-01-09T10:00:04.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"task-1-design","action":"write_file","args":{"path":"schema.sql","content":"..."}}
```

#### 2.3.6 tool.call / tool.result

工具调用和结果。

```jsonl
{"v":1,"type":"tool.call","ts":"2026-01-09T10:00:05.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"task-2-implement","action":"shell","args":{"command":"python -m pytest"}}
{"v":1,"type":"tool.result","ts":"2026-01-09T10:00:06.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"task-2-implement","output":"...","code":0}
```

#### 2.3.7 task.end

任务结束。

```jsonl
{"v":1,"type":"task.end","ts":"2026-01-09T10:00:10.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"task-1-design","metadata":{"status":"success","duration_ms":9000}}
```

#### 2.3.8 error

错误事件。

```jsonl
{"v":1,"type":"error","ts":"2026-01-09T10:00:11.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","task_id":"task-2-implement","error":"Connection timeout","code":504}
```

#### 2.3.9 run.end

运行结束。

```jsonl
{"v":1,"type":"run.end","ts":"2026-01-09T10:01:00.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","metadata":{"status":"success","total_tasks":3,"completed":3,"failed":0,"duration_ms":60000}}
```

### 2.4 Text 输出格式

纯文本流式输出，设计原则：
- **内容优先** - AI 输出内容直接显示，无前缀干扰
- **状态极简** - 仅在关键节点显示简短状态标记
- **人类可读** - 像对话一样自然，不像机器日志

#### 2.4.1 状态标记

| 标记 | 含义 | 使用场景 |
|------|------|----------|
| `▶` | 任务开始 | 任务启动时 |
| `✓` | 成功完成 | 任务成功结束 |
| `✗` | 失败 | 任务失败 |
| `⟳` | 重试中 | 正在重试 |
| `⏸` | 等待中 | 等待依赖完成 |
| `»` | 动作 | 执行文件操作等 |
| `⚠` | 警告 | 非致命问题 |
| `📄` | 文件 | 加载文件 |

#### 2.4.2 格式语法

```abnf
output          = *line
line            = status-line / content-line / summary-line

status-line     = marker SP message [meta] LF
marker          = "▶" / "✓" / "✗" / "⟳" / "⏸" / "»" / "⚠" / "📄"
message         = task-id / action-desc
meta            = SP "(" details ")" / SP duration / SP "←" SP dependencies

content-line    = text LF              ; AI 输出内容，原样显示
summary-line    = separator result LF  ; 运行总结
separator       = "───────────────────────────" LF
```

#### 2.4.3 单任务输出示例

```
▶ hello (codex/gpt-5.2)

```python
print("Hello, World!")
```

✓ hello 1.2s
```

#### 2.4.4 多任务依赖输出示例

```
▶ design-api (gemini)

设计 REST API 接口...

```yaml
openapi: 3.0.3
paths:
  /users:
    get:
      summary: 获取用户列表
```

✓ design-api 5.3s

▶ implement-api (codex/gpt-5.2) ← design-api

根据设计实现代码...

```python
@app.get("/users")
def get_users():
    return db.query(User).all()
```

» 写入 main.py
✓ implement-api 8.7s

▶ test-api (codex) ← implement-api

```python
def test_get_users():
    response = client.get("/users")
    assert response.status_code == 200
```

» 写入 test_main.py
» 运行 pytest
✓ test-api 4.2s

───────────────────────────
✓ 完成 3/3 任务 (18.2s)
```

#### 2.4.5 并行任务输出示例

```
▶ 并行执行 3 个任务...

  ▶ python-sort (codex)
  ▶ go-sort (codex)  
  ▶ rust-sort (codex)

  --- python-sort ---
  ```python
  def quicksort(arr):
      if len(arr) <= 1:
          return arr
      pivot = arr[len(arr) // 2]
      ...
  ```
  ✓ python-sort 3.2s

  --- go-sort ---
  ```go
  func quicksort(arr []int) []int {
      ...
  }
  ```
  ✓ go-sort 3.8s

  --- rust-sort ---
  ```rust
  fn quicksort<T: Ord>(arr: &[T]) -> Vec<T> {
      ...
  }
  ```
  ✓ rust-sort 4.1s

▶ compare (claude) ← python-sort, go-sort, rust-sort

| 语言 | 行数 | 性能 |
|------|------|------|
| Python | 8 | ⭐⭐ |
| Go | 16 | ⭐⭐⭐⭐ |
| Rust | 12 | ⭐⭐⭐⭐⭐ |

✓ compare 5.2s

───────────────────────────
✓ 完成 4/4 任务 (9.3s, 并行加速 1.8x)
```

#### 2.4.6 文件引用输出示例

```
▶ code-review (claude)
  📄 src/auth.py (2.3KB)
  📄 src/user.py (1.8KB)

## 代码审查

### 安全问题
- 第23行: SQL注入风险
- 第45行: 密码明文存储

### 建议
1. 使用参数化查询
2. 使用 bcrypt 哈希

✓ code-review 6.5s
```

#### 2.4.7 错误与重试输出示例

```
▶ unstable-task (codex)

正在处理...

⚠ 即将超时 (8s/10s)
✗ 超时

⟳ 重试 1/2

正在处理...完成！

✓ unstable-task 5.1s (重试1次)
```

**重试失败场景：**

```
▶ broken-task (codex)

✗ 连接超时

⟳ 重试 1/2
✗ 连接超时

⟳ 重试 2/2  
✗ 连接超时

───────────────────────────
✗ 失败 0/1 任务 - broken-task: 超时 (重试2次后放弃)
```

#### 2.4.8 输出模式选项

| 模式 | 参数 | 效果 |
|------|------|------|
| 默认 | - | 简洁状态 + 内容 |
| 详细 | `--verbose` | 添加时间戳和调试信息 |
| 静默 | `--quiet` | 只输出 AI 内容 |
| ASCII | `--ascii` | 用 ASCII 替代 Unicode 符号 |

**详细模式 (--verbose)：**

```
[10:00:00] ▶ task-1 (codex/gpt-5.2)
[10:00:00]   工作目录: /home/user/project
[10:00:00]   文件: src/*.py (5个, 12KB)

内容输出...

[10:00:05] » 写入 output.py (234行)
[10:00:05] » 运行 python -m pytest
[10:00:08] ✓ task-1 8.2s
```

**静默模式 (--quiet)：**

```
print("Hello, World!")
```

**ASCII 模式 (--ascii)：**

| Unicode | ASCII |
|---------|-------|
| `▶` | `>` |
| `✓` | `[OK]` |
| `✗` | `[FAIL]` |
| `⟳` | `[RETRY]` |
| `»` | `>>` |
| `⚠` | `[WARN]` |
| `📄` | `-` |

```
> task-1 (codex)

print("Hello")

[OK] task-1 1.2s
```

### 2.5 错误输出（stderr）

错误和警告输出到 stderr，格式：

```
[ERROR] 2026-01-09T10:00:11Z task_id=task-2-implement: Connection timeout (code=504)
[WARN]  2026-01-09T10:00:12Z task_id=task-3-test: Retrying (attempt 2/3)
[DEBUG] 2026-01-09T10:00:13Z Parsed 3 tasks from input
```

---

## 3. 错误代码

| Code | 名称 | 说明 |
|------|------|------|
| 0 | SUCCESS | 成功 |
| 1 | GENERAL_ERROR | 通用错误 |
| 2 | PARSE_ERROR | 输入解析错误 |
| 3 | VALIDATION_ERROR | 参数验证错误 |
| 10 | TASK_NOT_FOUND | 任务不存在 |
| 11 | DEPENDENCY_ERROR | 依赖解析错误 |
| 12 | CIRCULAR_DEPENDENCY | 循环依赖 |
| 20 | BACKEND_ERROR | 后端错误 |
| 21 | MODEL_NOT_FOUND | 模型不存在 |
| 22 | QUOTA_EXCEEDED | 配额超限 |
| 30 | TIMEOUT | 超时 |
| 31 | CANCELLED | 用户取消 |
| 40 | NETWORK_ERROR | 网络错误 |
| 41 | AUTH_ERROR | 认证错误 |
| 50 | TOOL_ERROR | 工具执行错误 |
| 51 | PERMISSION_DENIED | 权限拒绝 |
| 60 | FILE_NOT_FOUND | 引用文件不存在 |
| 61 | FILE_ACCESS_DENIED | 文件无读取权限 |
| 62 | FILE_TOO_LARGE | 文件超过大小限制 |
| 63 | TOO_MANY_FILES | 引用文件数超过限制 |
| 64 | INVALID_PATH | 无效文件路径格式 |
| 65 | PATH_TRAVERSAL | 检测到路径遍历攻击 |
| 66 | GLOB_NO_MATCH | Glob 模式无匹配文件 |
| 67 | ENCODING_ERROR | 文件编码读取错误 |

---

## 4. 命令行接口

### 4.1 基本用法

```bash
# 从 stdin 读取
memex-cli run --stdin < tasks.md

# 管道输入
cat tasks.md | memex-cli run --stdin

# Heredoc 输入 (Bash)
memex-cli run --stdin <<'EOF'
---TASK---
id: example
backend: codex
workdir: /home/user/project
---CONTENT---
编写 Hello World 程序
---END---
EOF

# Here-String 输入 (PowerShell)
@'
---TASK---
id: example
backend: codex
workdir: C:\Users\dev\project
---CONTENT---
编写 Hello World 程序
---END---
'@ | memex-cli run --stdin
```

### 4.2 参数覆盖

命令行参数可覆盖输入中的默认值：

```bash
memex-cli run --stdin \
  --backend codex \
  --model gpt-5.2 \
  --stream-format jsonl \
  --timeout 600 \
  < tasks.md
```

### 4.3 输出重定向

```bash
# 分离 stdout 和 stderr
memex-cli run --stdin < tasks.md > output.jsonl 2> error.log

# 实时查看输出
memex-cli run --stdin < tasks.md | tee output.jsonl

# 解析 JSONL 输出
memex-cli run --stdin < tasks.md | jq 'select(.type == "assistant.output") | .output'
```

---

## 5. 恢复运行

### 5.1 Resume 命令

```bash
memex-cli resume \
  --run-id <RUN_ID> \
  --stdin <<'EOF'
---TASK---
id: continue-task
backend: codex
workdir: /home/user/project
---CONTENT---
基于之前的结果，继续优化代码...
---END---
EOF
```

### 5.2 Resume 输出（Text 格式）

```
⟳ 恢复运行 run_id=550e8400...
  上下文: 3 个任务, 2500 tokens

▶ continue-task (codex) ← [历史上下文]

继续优化...

✓ continue-task 3.2s

───────────────────────────
✓ 完成 1/1 任务 (3.2s)
```

### 5.3 Resume 输出（JSONL 格式）

恢复运行时会先输出历史上下文：

```jsonl
{"v":1,"type":"run.resume","ts":"2026-01-09T11:00:00.000Z","run_id":"550e8400-e29b-41d4-a716-446655440000","metadata":{"original_run_id":"550e8400-e29b-41d4-a716-446655440000","resumed_at":"2026-01-09T11:00:00.000Z"}}
{"v":1,"type":"context.history","ts":"2026-01-09T11:00:00.001Z","run_id":"550e8400-e29b-41d4-a716-446655440000","output":"[Previous context loaded: 3 tasks, 2500 tokens]"}
```

---

## 6. 完整示例

> 📖 完整的从简单到复杂的示例请参阅 [STDIO_EXAMPLES.md](./STDIO_EXAMPLES.md)
>
> 包含：
> - 单任务示例
> - 指定模型
> - 超时与重试
> - 文件引用
> - 多任务依赖
> - 并行执行
> - 代码审查 + 自动修复
> - 完整项目生成

---

## 7. 安全考虑

1. **输入验证** - 所有元数据字段需验证格式和长度
2. **内容隔离** - `---CONTENT---` 后的内容视为不可信数据
3. **路径检查** - `workdir` 需验证防止路径遍历攻击
4. **超时保护** - 强制最大超时时间限制
5. **资源限制** - 限制单次运行的任务数量和总输出大小

---

## 8. 字段速查表

| 字段 | 必填 | 默认值 | 说明 |
|------|------|--------|------|
| `id` | ✅ | - | 任务唯一标识（支持时间戳格式） |
| `backend` | ✅ | - | AI 后端：codex/claude/gemini |
| `workdir` | ✅ | - | 工作目录路径 |
| `model` | ❌ | 后端默认 | 模型名称 |
| `model-provider` | ❌ | - | 模型提供商 |
| `dependencies` | ❌ | - | 依赖任务ID，逗号分隔 |
| `stream-format` | ❌ | text | 输出格式：text/jsonl |
| `timeout` | ❌ | 300 | 超时时间（秒） |
| `retry` | ❌ | 0 | 重试次数 |
| `files` | ❌ | - | 引用文件路径，逗号分隔 |
| `files-mode` | ❌ | auto | 文件模式：embed/ref/auto |
| `files-encoding` | ❌ | auto | 文件编码：utf-8/base64/auto |

---

## 9. 版本历史

| 版本 | 日期 | 变更 |
|------|------|------|
| 1.0.0 | 2026-01-09 | 初始版本 |
