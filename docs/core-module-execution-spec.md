# Core 模块执行规范（文件划分与依赖边界）

适用范围：`core/src/**` 下所有模块（如 `runner/`、`memory/`、`tool_event/`、`gatekeeper/`、`replay/`、`events_out/`、`config/`、`error/`、`util/`）。

目标：
- 入口文件可读、职责单一、可发现性强。
- 公共 API 稳定；实现细节可自由拆分与替换。
- 依赖方向清晰，避免循环依赖与“反向依赖业务模块”。

---

## 1. 目录与文件角色（统一约定）

### 1.1 `mod.rs`（装配器 / 门面）
允许：
- `pub mod ...` / `mod ...`
- `pub use ...`
- 少量常量/类型别名（仅用于导出层面）

禁止：
- 业务流程函数实现（例如 `run_*`、`*_cmd`、核心算法）
- IO（读写文件、网络、进程）
- 大段逻辑（超过 ~30 行即应拆出）

### 1.2 `types.rs`（数据结构）
职责：
- DTO/Args/Result/Config 片段/领域对象
- `Default`、简单构造器、纯函数转换

禁止：
- IO
- 跨模块的业务调用（`crate::xxx::do_something()` 这类流程调用）

### 1.3 `traits.rs` / `plugin_trait.rs`（扩展点接口）
职责：
- 对外可扩展接口定义（plugins 会实现的 trait）
- 约束输入/输出类型

规则：
- trait 定义应尽量依赖稳定的 `types.rs`，避免把具体实现类型泄漏为公共 API。

### 1.4 `error.rs`（错误语义）
职责：
- 本模块错误类型（`thiserror::Error`）
- 错误分类与转换（`From`）

规则：
- 错误信息表达“语义”，不要把日志策略写进错误类型。

### 1.5 `cmd.rs` / `run.rs`（主入口流程）
职责：
- 对外“主入口函数”（例如 `replay_cmd`、`run_session`）
- 负责组装依赖、串联流程、定义边界（输入校验、错误映射）

规则：
- 复杂步骤拆到 `service.rs`/`runtime.rs`/`parse.rs`/`render.rs` 等文件。

### 1.6 `service.rs` / `runtime.rs`（实现主体）
职责：
- 状态机/执行器/运行时
- 可以持有状态（client、tx、parser、缓存等）

规则：
- 允许依赖 IO 层（如 `tokio::fs`、`reqwest`），但要把 IO 聚合在少数实现文件中。

### 1.7 `parse.rs` / `parser.rs`（解析）
职责：
- 文本/JSONL/事件流解析为结构化数据

规则：
- 不做业务决策；解析失败用明确错误返回。

### 1.8 `render.rs` / `format.rs`（渲染）
职责：
- 结构化数据 → 文本/JSON（纯函数）

规则：
- 不做 IO；输出由调用方写出。

### 1.9 `helpers.rs`（小工具）
职责：
- 字符串截断、正则、小型转换等纯函数

规则：
- 避免引入重量级依赖；避免把业务流程塞进 helpers。

---

## 2. 可见性与公共 API

- `pub` 只用于：对外稳定的类型/trait/入口函数。
- 实现细节默认 `pub(crate)` 或私有（`mod foo;` + `fn`）。
- 对外导出统一从模块 `mod.rs` 做 `pub use`，避免调用方深挖到 `crate::module::internal_file::X`。

**判定标准**：如果 `plugins` 或 `cli` 会用到，就考虑作为公共 API；否则不要 `pub`。

---

## 3. 依赖方向（强约束）

### 3.1 基础层（不得反向依赖业务模块）
- `core::util`
- `core::error`
- `core::config`（尤其是类型层 `config/types.rs`）

禁止：
- `core::config` 反向依赖 `core::gatekeeper` / `core::memory` / `core::runner` 等业务模块。

### 3.2 业务层依赖建议
推荐依赖顺序（从“更基础”到“更业务”）：
- `tool_event`（协议/事件模型）
- `events_out`（事件落盘/输出）
- `runner`（进程/流采集/事件观测）
- `memory`、`gatekeeper`（业务决策与拼装）
- `replay`（离线分析/重放）

说明：这不是绝对层级，但一旦出现循环依赖，应优先通过：
- 抽取 `types.rs` 到更基础层
- 用输入/输出结构体替代“直接调用对方内部函数”
来断环。

---

## 4. 命名规范

- 文件名：`snake_case.rs`，与职责匹配（`types.rs`、`run.rs`、`runtime.rs`、`parser.rs`）。
- 类型名：`UpperCamelCase`；函数名：`snake_case`。
- `cmd` 后缀用于 CLI/子命令入口；`run` 用于执行流程入口；`runtime/service` 用于带状态实现。

---

## 5. IO 与副作用边界

- IO 只能出现在：`load.rs`、`writer.rs`、`client.rs`、`run.rs/runtime.rs/service.rs`。
- `types.rs`、`render.rs`、`parse.rs`、`helpers.rs` 必须保持纯逻辑（可测试、无副作用）。

---

## 6. 异常处理与日志

- 错误：用 `Result<T, ModuleError>` 或现有 `CliError/RunnerError` 分层传递。
- 日志：在“流程入口/运行时层”（`run.rs`/`runtime.rs`）记录；解析/渲染层不要随意 `tracing::info!`。

---

## 7. 重构执行流程（落地步骤）

当一个模块入口文件（通常是 `mod.rs` 或单个大文件）超过 ~300 行或职责混杂时，按以下步骤拆分：
1. 先抽出 `types.rs`（只移动类型与纯函数）。
2. 再抽出 `traits.rs`（插件接口）或 `error.rs`（错误类型）。
3. 把主流程函数移动到 `run.rs`/`cmd.rs`。
4. 把解析/格式化分别移动到 `parser.rs`/`render.rs`。
5. 保持原对外路径：在 `mod.rs` 中 `pub use` 回原名字，避免破坏调用方。
6. 每一步都跑 `cargo check`，保证小步可回滚。

---

## 8. Review 清单（必须过）

- `mod.rs` 是否只做装配与导出？
- 公共 API 是否只在 `mod.rs` 统一导出？
- 是否出现基础层反向依赖业务层？
- 解析/渲染是否保持纯逻辑、无 IO？
- 错误类型是否表达语义、避免夹带日志策略？
- 新增文件命名是否符合角色（types/run/runtime/parser/render）？

---

## 9. 允许的例外

- 极小模块（< ~80 行）允许暂时只用一个文件，但仍建议保留“入口薄、逻辑清晰”的结构。
- 性能关键路径允许合并文件以减少跳转，但必须在文档或 PR 说明里注明原因。
