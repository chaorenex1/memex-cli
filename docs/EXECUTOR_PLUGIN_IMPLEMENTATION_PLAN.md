# Executor 插件化实施计划

**版本**: 1.0.0
**日期**: 2026-01-11
**状态**: Phase 1-6 已完成

---

## 目录

1. [项目概览](#1-项目概览)
2. [架构设计](#2-架构设计)
3. [实施路线图](#3-实施路线图)
4. [代码框架](#4-代码框架)
5. [迁移检查清单](#5-迁移检查清单)
6. [测试计划](#6-测试计划)
7. [风险评估](#7-风险评估)

---

## 1. 项目概览

### 1.1 背景

**当前问题**：
- Executor 模块硬依赖 `StdioTask`（11处耦合点）
- 文件处理逻辑散布在 stdio 模块，无法复用
- 输出渲染借用 stdio 模块功能
- 配置分散在 StdioConfig 和 ExecutionOpts

**目标**：
- 彻底解除 executor 与 stdio 的耦合
- 实现插件化架构（遵循现有 memex-core/plugins 模式）
- 支持自定义处理器、渲染器、策略
- 提供配置驱动的插件加载

### 1.2 设计原则

1. **依赖反转**：Core 定义 trait → Plugins 实现 → CLI 组装
2. **单向依赖**：Plugins → Core（无循环依赖）
3. **运行时多态**：`Arc<dyn Trait>` 支持插件热切换
4. **配置驱动**：通过 `config.toml` 控制插件行为
5. **向后兼容**：保留 stdio 模块作为 deprecated

### 1.3 成功标准

**必须达成**：
- ✅ executor 模块零依赖 stdio（通过 `cargo tree` 验证）
- ✅ 所有旧测试用例通过
- ✅ 性能不低于旧版（基准测试）
- ✅ 公共 API 向后兼容（适配层）

**期望达成**：
- ✅ 代码覆盖率 ≥80%（各层独立测试）
- ✅ 文档完整（每个 trait 有示例）
- ✅ 支持自定义插件（扩展案例）

---

## 2. 架构设计

### 2.1 三层架构

```
┌─────────────────────────────────────────┐
│  memex-core (定义抽象)                    │
│  ├── executor/traits/                   │
│  │   ├── TaskProcessorPlugin           │
│  │   ├── OutputRendererPlugin          │
│  │   ├── RetryStrategyPlugin           │
│  │   └── ConcurrencyStrategyPlugin     │
│  └── executor/core/ (算法核心)           │
└─────────────────────────────────────────┘
              ↑ 依赖（单向）
┌─────────────────────────────────────────┐
│  memex-plugins (实现具体)                 │
│  └── executor/                          │
│      ├── processors/FileProcessorPlugin │
│      ├── renderers/JsonlRendererPlugin  │
│      └── strategies/AdaptivePlugin      │
└─────────────────────────────────────────┘
              ↑ 使用
┌─────────────────────────────────────────┐
│  memex-cli (组装)                        │
│  └── factory::build_* (动态构建)          │
└─────────────────────────────────────────┘
```

### 2.2 插件化组件清单

| 组件类型 | Core Trait | Plugins 实现 | Factory 函数 | 优先级 |
|---------|-----------|-------------|-------------|--------|
| **任务处理器** | `TaskProcessorPlugin` | FileProcessorPlugin, PromptEnhancerPlugin | `build_task_processors` | ⭐⭐⭐ |
| **输出渲染器** | `OutputRendererPlugin` | JsonlRendererPlugin, TextRendererPlugin | `build_renderer` | ⭐⭐⭐ |
| **重试策略** | `RetryStrategyPlugin` | ExponentialBackoffPlugin, LinearRetryPlugin | `build_retry_strategy` | ⭐⭐ |
| **并发策略** | `ConcurrencyStrategyPlugin` | AdaptiveConcurrencyPlugin, FixedConcurrencyPlugin | `build_concurrency_strategy` | ⭐⭐ |

### 2.3 文件结构

```
core/src/executor/
├── mod.rs
├── types/
│   ├── mod.rs
│   ├── task.rs          # ExecutableTask
│   ├── config.rs        # ExecutionConfig
│   ├── result.rs        # ExecutionResult, TaskResult
│   └── error.rs         # ExecutorError, ProcessorError
├── traits/
│   ├── mod.rs
│   ├── processor.rs     # TaskProcessorPlugin trait ⭐
│   ├── renderer.rs      # OutputRendererPlugin trait ⭐
│   └── strategy.rs      # RetryStrategyPlugin, ConcurrencyStrategyPlugin ⭐
└── core/
    ├── mod.rs
    ├── graph.rs         # TaskGraph<T: TaskLike>
    ├── scheduler.rs     # Scheduler
    └── engine.rs        # ExecutionEngine

plugins/src/executor/
├── mod.rs
├── processors/
│   ├── mod.rs
│   ├── files.rs         # FileProcessorPlugin (移植 stdio/files.rs)
│   ├── prompt.rs        # PromptEnhancerPlugin
│   └── context.rs       # ContextInjectorPlugin
├── renderers/
│   ├── mod.rs
│   ├── text.rs          # TextRendererPlugin
│   └── jsonl.rs         # JsonlRendererPlugin
└── strategies/
    ├── mod.rs
    ├── retry.rs         # ExponentialBackoffPlugin, LinearRetryPlugin
    └── concurrency.rs   # AdaptiveConcurrencyPlugin, FixedConcurrencyPlugin
```

---

## 3. 实施路线图

### 总览

```
总计：10 天
Phase 1: Core Trait 定义（2天）       → 6个文件，约500行 ✅
Phase 2: Plugins 实现（3天）         → 8个文件，约1200行 ✅
Phase 3: Factory 集成（1天）         → 1个文件，约200行 ✅
Phase 4: Core Engine 重构（2天）     → 3个文件，约400行 ✅
Phase 5: CLI 集成（1天）             → 2个文件，约150行 ✅
Phase 6: 废弃旧代码（1天）           → 删除3个文件，测试 ✅
```

---

### Phase 1: Core Trait 定义（2天）

#### 任务清单

| ID | 文件 | 内容 | 行数 | 依赖 | 验收 |
|----|------|------|------|------|------|
| **P1.1** | `core/src/executor/types/task.rs` | ExecutableTask, TaskMetadata | ~80 | - | 编译通过 |
| **P1.2** | `core/src/executor/types/config.rs` | ExecutionConfig, 子配置结构 | ~120 | - | 编译通过 |
| **P1.3** | `core/src/executor/types/error.rs` | ExecutorError, ProcessorError | ~60 | - | 编译通过 |
| **P1.4** | `core/src/executor/traits/processor.rs` | TaskProcessorPlugin trait | ~80 | P1.1 | trait 可实现 |
| **P1.5** | `core/src/executor/traits/renderer.rs` | OutputRendererPlugin trait, RenderEvent | ~100 | - | trait 可实现 |
| **P1.6** | `core/src/executor/traits/strategy.rs` | RetryStrategyPlugin, ConcurrencyStrategyPlugin | ~80 | - | trait 可实现 |

#### 代码框架示例

见 [4. 代码框架](#4-代码框架) 章节。

#### 验收标准

```bash
# 编译检查
cd core
cargo clippy --workspace --all-targets -- -D warnings

# 确认无依赖 stdio
cargo tree -p memex-core | grep stdio
# 预期输出：空（无结果）

# 文档生成
cargo doc --no-deps --open
# 预期：所有 trait 有文档说明
```

---

### Phase 2: Plugins 实现（3天）

#### 任务清单

| ID | 文件 | 内容 | 行数 | 依赖 | 验收 |
|----|------|------|------|------|------|
| **P2.1** | `plugins/src/executor/processors/files.rs` | FileProcessorPlugin（移植 stdio/files.rs） | ~400 | P1.4 | 单元测试通过 |
| **P2.2** | `plugins/src/executor/processors/prompt.rs` | PromptEnhancerPlugin | ~80 | P1.4 | 单元测试通过 |
| **P2.3** | `plugins/src/executor/processors/context.rs` | ContextInjectorPlugin | ~60 | P1.4 | 单元测试通过 |
| **P2.4** | `plugins/src/executor/renderers/jsonl.rs` | JsonlRendererPlugin | ~150 | P1.5 | 输出格式验证 |
| **P2.5** | `plugins/src/executor/renderers/text.rs` | TextRendererPlugin | ~120 | P1.5 | 输出格式验证 |
| **P2.6** | `plugins/src/executor/strategies/retry.rs` | ExponentialBackoffPlugin, LinearRetryPlugin | ~150 | P1.6 | 单元测试通过 |
| **P2.7** | `plugins/src/executor/strategies/concurrency.rs` | AdaptiveConcurrencyPlugin, FixedConcurrencyPlugin | ~120 | P1.6 | 单元测试通过 |
| **P2.8** | `plugins/src/executor/mod.rs` | 模块导出 | ~30 | 所有 | 编译通过 |

#### 关键任务：P2.1 FileProcessorPlugin 移植

**移植内容**（从 `stdio/files.rs`）：
- `resolve_files()` 函数 → `FileProcessorPlugin::resolve_files_internal()`
- `compose_prompt()` 函数 → `FileProcessorPlugin::compose_prompt_internal()`
- `process_single_file()` 函数 → 内部方法
- `read_file_with_mmap()` 函数 → 内部方法
- `read_file_cached()` 函数 → 内部方法
- 全局 `FILE_CACHE` → 保留为 static

**性能要求**：
- ✅ mmap 优化保留（大文件 >10MB）
- ✅ LRU 缓存保留（缓存大小可配置）
- ✅ 并发限制保留（Semaphore 16个文件）
- ✅ 早期取消保留（AtomicBool）

#### 验收标准

```bash
# 单元测试
cd plugins
cargo test executor::processors::files

# 性能基准测试
cargo bench --bench file_processor

# 预期：性能不低于旧版 stdio/files.rs
```

---

### Phase 3: Factory 集成（1天）

#### 任务清单

| ID | 文件 | 内容 | 行数 | 依赖 | 验收 |
|----|------|------|------|------|------|
| **P3.1** | `plugins/src/factory.rs` | `build_task_processors()` | ~50 | P2.1-P2.3 | 返回正确插件链 |
| **P3.2** | `plugins/src/factory.rs` | `build_renderer()` | ~30 | P2.4-P2.5 | 根据format返回正确插件 |
| **P3.3** | `plugins/src/factory.rs` | `build_retry_strategy()` | ~30 | P2.6 | 根据配置返回正确插件 |
| **P3.4** | `plugins/src/factory.rs` | `build_concurrency_strategy()` | ~30 | P2.7 | 根据配置返回正确插件 |

#### 代码框架示例

```rust
// plugins/src/factory.rs

use std::sync::Arc;
use memex_core::executor::traits::*;
use memex_core::api as core_api;

use crate::executor::processors::files::FileProcessorPlugin;
use crate::executor::renderers::{JsonlRendererPlugin, TextRendererPlugin};
use crate::executor::strategies::*;

/// 构建任务处理器插件链
pub fn build_task_processors(
    cfg: &core_api::ExecutorConfig,
) -> Vec<Arc<dyn TaskProcessorPlugin>> {
    let mut processors: Vec<Arc<dyn TaskProcessorPlugin>> = Vec::new();

    // 文件处理器
    if cfg.file_processing.enabled {
        let file_processor = FileProcessorPlugin::new(cfg.file_processing.clone().into());
        processors.push(Arc::new(file_processor));
    }

    // 按优先级排序
    processors.sort_by(|a, b| b.priority().cmp(&a.priority()));

    processors
}

/// 构建输出渲染器插件
pub fn build_renderer(format: &str, cfg: &core_api::OutputConfig)
    -> Arc<dyn OutputRendererPlugin>
{
    match format {
        "jsonl" => Arc::new(JsonlRendererPlugin::new(cfg.pretty_print)),
        "text" => Arc::new(TextRendererPlugin::new(cfg.ascii_only)),
        _ => Arc::new(TextRendererPlugin::new(false)),
    }
}

// ... 其他构建函数
```

#### 验收标准

```bash
# 集成测试
cargo test factory::build_task_processors
cargo test factory::build_renderer

# 预期：根据配置正确构建插件
```

---

### Phase 4: Core Engine 重构（2天）

#### 任务清单

| ID | 文件 | 内容 | 行数 | 依赖 | 验收 |
|----|------|------|------|------|------|
| **P4.1** | `core/src/executor/core/engine.rs` | ExecutionEngine 接受插件参数 | ~150 | P1.4-P1.6 | 集成测试通过 |
| **P4.2** | `core/src/executor/core/graph.rs` | 泛型化 TaskGraph<T: TaskLike> | ~100 | P1.1 | 单元测试通过 |
| **P4.3** | `core/src/executor/core/scheduler.rs` | 集成 ConcurrencyStrategyPlugin | ~80 | P1.6 | 并发控制验证 |

#### 关键变更：ExecutionEngine

**旧签名**（硬编码 planner）：
```rust
pub async fn execute_tasks<F>(
    tasks: Vec<StdioTask>,
    ctx: &AppContext,
    opts: &ExecutionOpts,
    planner: F,  // ← 硬编码函数
) -> Result<ExecutionResult, ExecutorError>
```

**新签名**（插件化）：
```rust
pub struct ExecutionEngine {
    processors: Vec<Arc<dyn TaskProcessorPlugin>>,
    renderer: Arc<dyn OutputRendererPlugin>,
    retry_strategy: Arc<dyn RetryStrategyPlugin>,
    concurrency_strategy: Arc<dyn ConcurrencyStrategyPlugin>,
}

impl ExecutionEngine {
    pub fn builder() -> ExecutionEngineBuilder { ... }

    pub async fn execute<T: TaskLike>(
        &self,
        tasks: Vec<T>,
        ctx: &AppContext,
    ) -> Result<ExecutionResult, ExecutorError> {
        // 使用注入的插件执行任务
    }
}
```

#### 验收标准

```bash
# 集成测试
cargo test executor::engine

# 端到端测试（使用真实插件）
cargo test --test executor_integration

# 预期：所有任务正确执行，插件正确调用
```

---

### Phase 5: CLI 集成（1天）

#### 任务清单

| ID | 文件 | 内容 | 行数 | 依赖 | 验收 |
|----|------|------|------|------|------|
| **P5.1** | `cli/src/commands/stdio.rs` | 使用 factory 构建插件 | ~80 | P3.1-P3.4 | 端到端测试通过 |
| **P5.2** | `core/src/api.rs` | 导出新的 executor API | ~30 | P4.1 | 编译通过 |
| **P5.3** | `config.toml` | 新增 executor 配置段 | ~40 | - | 配置加载验证 |

#### 代码示例：cli/commands/stdio.rs

```rust
use memex_plugins::factory;
use memex_core::executor::ExecutionEngine;

pub async fn handle_stdio(args: StdioArgs, ctx: &AppContext) -> Result<i32> {
    // 1. 解析任务
    let tasks = parse_stdio_tasks(&input)?;

    // 2. 构建插件（通过 factory）
    let processors = factory::build_task_processors(&ctx.cfg().executor);
    let renderer = factory::build_renderer(&args.stream_format, &ctx.cfg().executor.output);
    let retry_strategy = factory::build_retry_strategy(&ctx.cfg().executor.retry);
    let concurrency_strategy = factory::build_concurrency_strategy(&ctx.cfg().executor.concurrency);

    // 3. 构建执行引擎（注入插件）
    let engine = ExecutionEngine::builder()
        .processors(processors)
        .renderer(renderer)
        .retry_strategy(retry_strategy)
        .concurrency_strategy(concurrency_strategy)
        .build();

    // 4. 执行任务
    let result = engine.execute(tasks, ctx).await?;

    Ok(if result.failed > 0 { 1 } else { 0 })
}
```

#### 配置文件示例：config.toml

```toml
[executor]

# 文件处理配置
[executor.file_processing]
enabled = true
enable_mmap = true
mmap_threshold_mb = 10
enable_cache = true
cache_size = 100
max_files = 100
max_total_size_mb = 200

# 输出配置
[executor.output]
format = "jsonl"  # or "text"
pretty_print = false
ascii_only = false

# 重试策略
[executor.retry]
strategy = "exponential-backoff"
base_delay_ms = 100
max_delay_ms = 5000
max_attempts = 3

# 并发策略
[executor.concurrency]
strategy = "adaptive"
min_concurrency = 2
max_concurrency = 32
base_concurrency = 8
cpu_threshold_low = 50.0
cpu_threshold_high = 80.0
```

#### 验收标准

```bash
# 端到端测试（真实输入）
echo '{"id":"test","content":"echo hello"}' | cargo run -- stdio

# 预期：输出正确的 JSONL 格式，任务执行成功
```

---

### Phase 6: 废弃旧代码（1天）

#### 任务清单

| ID | 文件 | 内容 | 依赖 | 验收 |
|----|------|------|------|------|
| **P6.1** | 删除 `stdio/executor.rs` | 1526行旧执行器代码 | P5.1-P5.3 | cargo build 通过 |
| **P6.2** | 删除 `stdio/adapter.rs` | 121行旧适配器代码 | P5.1-P5.3 | cargo build 通过 |
| **P6.3** | 标记 `stdio/files.rs` | 添加 `#[deprecated]` 属性 | P2.1 | 编译警告出现 |
| **P6.4** | 更新 `stdio/mod.rs` | 移除废弃模块导出 | P6.1-P6.3 | 编译通过 |
| **P6.5** | 端到端测试 | 运行所有集成测试 | 所有 | 100%通过 |
| **P6.6** | 性能基准测试 | 对比新旧版本性能 | 所有 | 性能不回退 |

**进度**: P6.1-P6.6 已完成。

#### 验收标准

```bash
# 确认 stdio 模块零引用
rg "stdio::(executor|adapter|files)" --type rust
# 预期输出：仅 deprecated 警告

# 运行全部测试
cargo test --workspace

# 性能基准测试
cargo bench --bench executor_perf

# 预期：新版性能 ≥ 旧版的 95%
```

---

## 4. 代码框架

### 4.1 Core Trait 定义

#### TaskProcessorPlugin Trait

```rust
// core/src/executor/traits/processor.rs

use async_trait::async_trait;
use crate::executor::types::{ExecutableTask, ProcessorError};
use std::collections::HashMap;

/// 任务处理器插件（在执行前转换任务）
#[async_trait]
pub trait TaskProcessorPlugin: Send + Sync {
    /// 插件名称（唯一标识）
    fn name(&self) -> &str;

    /// 处理优先级（数字越大越先执行）
    fn priority(&self) -> i32 {
        0
    }

    /// 处理任务
    async fn process(
        &self,
        task: &ExecutableTask,
        context: &ProcessContext,
    ) -> Result<ProcessedTask, ProcessorError>;

    /// 是否可并行执行（与其他处理器）
    fn is_parallelizable(&self) -> bool {
        true
    }
}

/// 处理上下文
#[derive(Debug, Clone)]
pub struct ProcessContext {
    pub dependency_outputs: HashMap<String, String>,
    pub run_id: String,
    pub stage_id: usize,
    pub app_config: std::sync::Arc<crate::config::AppConfig>,
}

/// 处理后的任务
#[derive(Debug, Clone)]
pub struct ProcessedTask {
    pub original: ExecutableTask,
    pub enhanced_content: String,
    pub metadata: ProcessMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessMetadata {
    pub files: Vec<FileInfo>,
    pub custom: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
}
```

#### OutputRendererPlugin Trait

```rust
// core/src/executor/traits/renderer.rs

use crate::executor::types::{ExecutionResult, TaskResult};

/// 输出渲染器插件（控制输出格式）
pub trait OutputRendererPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn format(&self) -> &str;
    fn supports_streaming(&self) -> bool { false }
    fn render(&self, event: &RenderEvent);
}

/// 渲染事件（统一事件类型）
#[derive(Debug, Clone)]
pub enum RenderEvent {
    RunStart { run_id: String, total_tasks: usize, total_stages: usize },
    Plan { run_id: String, stages: Vec<Vec<String>> },
    StageStart { run_id: String, stage_id: usize, task_ids: Vec<String> },
    TaskStart { run_id: String, task_id: String, stage_id: usize },
    TaskProgress { run_id: String, task_id: String, progress: f32, message: Option<String> },
    TaskComplete { run_id: String, task_id: String, result: TaskResult },
    StageEnd { run_id: String, stage_id: usize },
    RunEnd { run_id: String, result: ExecutionResult },
}
```

#### Strategy Plugin Traits

```rust
// core/src/executor/traits/strategy.rs

use std::time::Duration;

/// 重试策略插件
pub trait RetryStrategyPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn next_delay(&self, attempt: u32, error: &str) -> Option<Duration>;
    fn max_attempts(&self) -> u32;
    fn should_retry(&self, attempt: u32, error: &str) -> bool {
        attempt < self.max_attempts() && !self.is_fatal_error(error)
    }
    fn is_fatal_error(&self, _error: &str) -> bool { false }
}

/// 并发控制策略插件
pub trait ConcurrencyStrategyPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn calculate_concurrency(&self, context: &ConcurrencyContext) -> usize;
}

#[derive(Debug, Clone)]
pub struct ConcurrencyContext {
    pub cpu_usage: f32,
    pub available_cpus: usize,
    pub memory_usage: f32,
    pub active_tasks: usize,
    pub base_concurrency: usize,
}
```

---

### 4.2 Plugins 实现示例

#### FileProcessorPlugin

```rust
// plugins/src/executor/processors/files.rs

use async_trait::async_trait;
use memex_core::executor::traits::{
    TaskProcessorPlugin, ProcessContext, ProcessedTask, ProcessMetadata, FileInfo
};
use memex_core::executor::types::{ExecutableTask, ProcessorError};

pub struct FileProcessorPlugin {
    config: FileProcessingConfig,
}

#[derive(Debug, Clone)]
pub struct FileProcessingConfig {
    pub enable_mmap: bool,
    pub mmap_threshold_mb: u64,
    pub enable_cache: bool,
    pub cache_size: usize,
    pub max_files: usize,
    pub max_total_size_mb: u64,
}

impl FileProcessorPlugin {
    pub fn new(config: FileProcessingConfig) -> Self {
        Self { config }
    }

    // 内部方法（从 stdio/files.rs 移植）
    async fn resolve_files_internal(&self, task: &ExecutableTask) -> Result<Vec<ResolvedFile>, anyhow::Error> {
        // TODO: 移植 stdio/files.rs::resolve_files 逻辑
        todo!()
    }

    fn compose_prompt_internal(&self, content: &str, files: &[ResolvedFile]) -> String {
        // TODO: 移植 stdio/files.rs::compose_prompt 逻辑
        todo!()
    }
}

#[async_trait]
impl TaskProcessorPlugin for FileProcessorPlugin {
    fn name(&self) -> &str { "file-processor" }
    fn priority(&self) -> i32 { 100 }

    async fn process(
        &self,
        task: &ExecutableTask,
        _context: &ProcessContext,
    ) -> Result<ProcessedTask, ProcessorError> {
        if task.metadata.files.is_empty() {
            return Ok(ProcessedTask {
                original: task.clone(),
                enhanced_content: task.content.clone(),
                metadata: ProcessMetadata::default(),
            });
        }

        let resolved_files = self.resolve_files_internal(task).await
            .map_err(|e| ProcessorError::ProcessFailed(e.to_string()))?;

        let enhanced_content = self.compose_prompt_internal(&task.content, &resolved_files);

        let file_infos: Vec<FileInfo> = resolved_files
            .iter()
            .map(|f| FileInfo { path: f.display_path.clone(), size: f.size })
            .collect();

        Ok(ProcessedTask {
            original: task.clone(),
            enhanced_content,
            metadata: ProcessMetadata { files: file_infos, custom: Default::default() },
        })
    }
}
```

#### JsonlRendererPlugin

```rust
// plugins/src/executor/renderers/jsonl.rs

use memex_core::executor::traits::{OutputRendererPlugin, RenderEvent};
use serde_json::json;
use chrono::Utc;

pub struct JsonlRendererPlugin {
    pretty_print: bool,
}

impl JsonlRendererPlugin {
    pub fn new(pretty_print: bool) -> Self {
        Self { pretty_print }
    }
}

impl OutputRendererPlugin for JsonlRendererPlugin {
    fn name(&self) -> &str { "jsonl-renderer" }
    fn format(&self) -> &str { "jsonl" }
    fn supports_streaming(&self) -> bool { true }

    fn render(&self, event: &RenderEvent) {
        let json_value = match event {
            RenderEvent::TaskStart { run_id, task_id, stage_id } => json!({
                "v": 1,
                "type": "task.start",
                "run_id": run_id,
                "task_id": task_id,
                "stage_id": stage_id,
                "ts": Utc::now().to_rfc3339(),
            }),
            // ... 其他事件
            _ => return,
        };

        let output = if self.pretty_print {
            serde_json::to_string_pretty(&json_value).unwrap()
        } else {
            serde_json::to_string(&json_value).unwrap()
        };

        println!("{}", output);
    }
}
```

---

## 5. 迁移检查清单

### 5.1 stdio/files.rs 移植清单

从 `core/src/stdio/files.rs` 移植到 `plugins/src/executor/processors/files.rs`

| 功能 | 源位置 | 目标位置 | 状态 | 备注 |
|-----|-------|---------|------|------|
| **数据结构** |
| `ResolvedFile` | files.rs:32-39 | 保留为内部结构 | ⬜ 待移植 | 不对外暴露 |
| `ResolvedContent` | files.rs:42-46 | 保留为内部结构 | ⬜ 待移植 | 不对外暴露 |
| `FILE_CACHE` | files.rs:49-57 | 保留为 static | ⬜ 待移植 | 全局 LRU 缓存 |
| **核心函数** |
| `resolve_files()` | files.rs:249-369 | `resolve_files_internal()` | ⬜ 待移植 | 作为插件内部方法 |
| `compose_prompt()` | files.rs:372-464 | `compose_prompt_internal()` | ⬜ 待移植 | 作为插件内部方法 |
| `process_single_file()` | files.rs:134-247 | 内部方法 | ⬜ 待移植 | 私有方法 |
| **优化特性** |
| `read_file_with_mmap()` | files.rs:60-89 | 内部方法 | ⬜ 待移植 | Level 3.1 优化 |
| `read_file_cached()` | files.rs:92-132 | 内部方法 | ⬜ 待移植 | Level 3.3 优化 |
| 并发限制（Semaphore） | files.rs:266 | 内部常量 | ⬜ 待移植 | 16个文件并发 |
| 早期取消（AtomicBool） | files.rs:268 | 内部变量 | ⬜ 待移植 | 错误时取消 |
| **辅助函数** |
| `format_file_metadata()` | files.rs:467-480 | 内部方法 | ⬜ 待移植 | 元数据格式化 |

### 5.2 stdio/adapter.rs 废弃清单

`core/src/stdio/adapter.rs` 整体废弃，功能由插件系统替代

| 功能 | 原位置 | 替代方案 | 状态 |
|-----|-------|---------|------|
| `wrap_planner_with_files()` | adapter.rs:42-86 | `TaskProcessorPlugin::process()` | ⬜ 待替换 |
| `configure_stdio_optimizations()` | adapter.rs:101-107 | 配置传递给插件构造函数 | ⬜ 待替换 |
| `finalize_stdio_optimizations()` | adapter.rs:118-120 | 由渲染器插件管理 | ⬜ 待替换 |

### 5.3 stdio/executor.rs 废弃清单

`core/src/stdio/executor.rs` 整体废弃，功能由新 executor 核心替代

| 功能 | 原位置 | 替代方案 | 状态 |
|-----|-------|---------|------|
| `execute_single_task()` | executor.rs:188-672 | `ExecutionEngine::execute()` | ⬜ 待替换 |
| `execute_task()` | executor.rs:1468-1526 | `ExecutionEngine::execute()` | ⬜ 待替换 |
| 内联文件处理 | executor.rs:202-203 | `FileProcessorPlugin` | ⬜ 待替换 |
| 内联重试逻辑 | executor.rs:236-358 | `RetryStrategyPlugin` | ⬜ 待替换 |

---

## 6. 测试计划

### 6.1 单元测试

#### Core Trait 测试

```bash
# 测试文件：core/tests/executor_traits.rs
cargo test --test executor_traits

# 测试覆盖：
# - TaskProcessorPlugin 可实现性
# - OutputRendererPlugin 可实现性
# - RetryStrategyPlugin 可实现性
# - ConcurrencyStrategyPlugin 可实现性
```

#### Plugins 实现测试

```bash
# FileProcessorPlugin
cargo test --test file_processor
# 覆盖：文件解析、mmap、缓存、并发限制

# JsonlRendererPlugin
cargo test --test jsonl_renderer
# 覆盖：事件渲染、JSONL格式验证

# AdaptiveConcurrencyPlugin
cargo test --test adaptive_concurrency
# 覆盖：CPU负载计算、并发数调整
```

### 6.2 集成测试

```bash
# 端到端测试（真实任务执行）
cargo test --test executor_integration

# 测试场景：
# - 单任务执行
# - 多任务 DAG 执行
# - 文件处理 + 渲染
# - 重试策略触发
# - 并发控制验证
```

### 6.3 性能基准测试

```bash
# 文件处理性能
cargo bench --bench file_processor_perf

# 对比项：
# - 小文件处理（<1MB）：新版 vs 旧版
# - 大文件处理（>10MB）：mmap 是否生效
# - 缓存命中率：LRU 缓存效果
# - 并发处理：16个文件并发吞吐量

# 预期标准：新版性能 ≥ 旧版的 95%
```

### 6.4 向后兼容性测试

```bash
# 使用旧配置文件运行
cp config.toml.old config.toml
cargo run -- stdio < input.jsonl

# 预期：兼容层自动转换，功能正常
```

---

## 7. 风险评估

### 7.1 技术风险

| 风险 | 严重性 | 概率 | 缓解措施 | 验证方法 |
|-----|-------|------|---------|---------|
| **性能回退** | 高 | 中 | Phase 2 后立即基准测试，优化热路径 | `cargo bench` |
| **破坏性变更** | 高 | 低 | Phase 4 提供适配层，向后兼容 | 集成测试 |
| **内存泄漏** | 中 | 低 | Arc 引用计数检查，Valgrind 验证 | `valgrind` |
| **并发安全** | 中 | 中 | 使用 `Arc<RwLock>` 保护共享状态 | 压力测试 |

### 7.2 项目风险

| 风险 | 严重性 | 概率 | 缓解措施 | 应对计划 |
|-----|-------|------|---------|---------|
| **开发延期** | 中 | 中 | 分阶段验收，允许并行开发 | 调整优先级 |
| **测试覆盖不足** | 中 | 中 | Phase 2-4 强制单元测试 | 补充测试用例 |
| **接口不稳定** | 低 | 低 | Phase 1 冻结 trait 设计 | 社区评审 |

### 7.3 回滚计划

**触发条件**：
- Phase 6 性能基准测试失败（新版 < 旧版 90%）
- 关键集成测试失败（>10% 用例失败）
- 发现严重 bug（数据丢失、内存泄漏等）

**回滚步骤**：
1. 恢复 `stdio/executor.rs`, `stdio/adapter.rs` 删除的代码（git revert）
2. 移除 `plugins/src/executor/` 新增代码
3. 恢复 `cli/commands/stdio.rs` 旧调用方式
4. 运行完整测试套件验证

**数据备份**：
- Phase 6 前创建 git tag `pre-executor-plugin`
- 保留完整的性能基准数据（旧版 vs 新版）

---

## 8. 附录

### 8.1 术语表

| 术语 | 定义 |
|-----|------|
| **TaskProcessorPlugin** | 任务处理器插件，在执行前转换任务（文件处理、Prompt增强等） |
| **OutputRendererPlugin** | 输出渲染器插件，控制输出格式（text, jsonl, html等） |
| **RetryStrategyPlugin** | 重试策略插件，定义重试逻辑（延迟计算、错误判断等） |
| **ConcurrencyStrategyPlugin** | 并发策略插件，动态调整并发数（基于CPU、内存等） |
| **Factory** | 工厂模式，根据配置动态构建插件实例 |

### 8.2 参考文档

- [EXECUTOR_DESIGN.md](./EXECUTOR_DESIGN.md) - 执行器核心设计
- [ARCHITECTURE_ANALYSIS.md](./ARCHITECTURE_ANALYSIS.md) - 架构分析
- [STDIO_PROTOCOL.md](./STDIO_PROTOCOL.md) - STDIO 协议规范
- [CLAUDE.md](../CLAUDE.md) - 项目集成指南

### 8.3 联系方式

**技术负责人**：待定
**评审委员会**：待定
**问题报告**：GitHub Issues

---

**文档状态**: ✅ 实施完成
**下次更新**: 需要时补充维护说明
