# HTTP服务器迁移 - 开发计划

## 概述
将Python FastAPI记忆服务迁移到Rust memex-cli，作为内嵌HTTP服务器暴露记忆服务API，直接复用MemoryPlugin trait实现，移除AI验证，使用现有Gatekeeper机制。

## 任务分解

### Task 1: HTTP基础设施和数据模型
- **ID**: task-1
- **type**: default
- **描述**: 建立HTTP模块核心架构，包括请求/响应数据模型、应用状态管理、基础验证逻辑（替代Python的AI验证）
- **文件范围**:
  - `cli/Cargo.toml` (添加依赖: axum 0.7, tower 0.4, tower-http 0.5, serde)
  - `cli/src/http/mod.rs` (模块声明)
  - `cli/src/http/models.rs` (API请求/响应结构体)
  - `cli/src/http/state.rs` (AppState, ServerStats)
  - `cli/src/http/validation.rs` (基础验证函数)
- **依赖**: 无
- **测试命令**:
  ```bash
  cargo test --package memex-cli --lib http::models -- --nocapture
  cargo test --package memex-cli --lib http::validation -- --nocapture --test-threads=1
  ```
- **测试重点**:
  - `models.rs`: 所有请求/响应结构体的序列化/反序列化（JSON往返测试）
  - `validation.rs`: 边界值测试（空字符串、超长输入、无效格式）
  - `state.rs`: AppState创建、ServerStats并发更新（使用Arc<AtomicU64>）

### Task 2: HTTP路由和服务器生命周期
- **ID**: task-2
- **type**: default
- **描述**: 实现6个API端点handlers、Axum服务器启动/优雅关闭逻辑、中间件（CORS、日志、超时）、状态文件管理
- **文件范围**:
  - `cli/src/http/routes.rs` (路由handlers: search, record_candidate, record_hit, validate, health, shutdown)
  - `cli/src/http/server.rs` (服务器启动/停止、信号处理、状态文件写入到 `~/.memex/servers/`)
  - `cli/src/http/middleware.rs` (CORS仅允许localhost、请求日志、30秒超时)
- **依赖**: task-1
- **测试命令**:
  ```bash
  cargo test --package memex-cli --lib http::routes -- --nocapture --test-threads=1
  cargo test --package memex-cli --lib http::server -- --nocapture --test-threads=1
  ```
- **测试重点**:
  - `routes.rs`:
    - 正常路径：POST /api/v1/search 返回200和匹配结果
    - 错误处理：缺失字段返回400、MemoryPlugin错误返回500
    - GET /health 返回200和服务器统计
    - POST /api/v1/shutdown 触发关闭信号
  - `server.rs`:
    - 服务器启动监听8080端口
    - 优雅关闭（SIGTERM信号处理、等待活跃连接）
    - 状态文件创建/删除（~/.memex/servers/http-8080.pid）
  - `middleware.rs`:
    - CORS preflight请求返回正确头（Access-Control-Allow-Origin: http://localhost:*）
    - 请求日志包含方法、路径、耗时
    - 超时测试（模拟慢handler，30秒触发408）

### Task 3: CLI集成和配置
- **ID**: task-3
- **type**: quick-fix
- **描述**: 添加HTTP服务器CLI子命令、配置结构、集成到main.rs命令分派流程
- **文件范围**:
  - `cli/src/commands/cli.rs` (添加HttpServerArgs结构体到Commands枚举)
  - `cli/src/commands/http_server.rs` (handler函数，调用http::server::start)
  - `cli/src/main.rs` (添加Commands::HttpServer分支)
  - `core/src/config/types.rs` (添加HttpServerConfig结构体到AppConfig)
  - `config.toml` (添加[http_server]配置节)
- **依赖**: task-2
- **测试命令**:
  ```bash
  cargo build --package memex-cli --release
  cargo run --package memex-cli -- http-server --help
  cargo run --package memex-cli -- http-server --port 8080 &
  sleep 2
  curl http://localhost:8080/health
  curl -X POST http://localhost:8080/api/v1/shutdown
  ```
- **测试重点**:
  - CLI参数解析：`--port`, `--host`, `--cors-origins` 参数正确传递到服务器
  - 配置加载：config.toml中的http_server节正确合并到AppConfig
  - 命令执行：`memex-cli http-server` 成功启动服务器且/health可访问
  - 帮助文档：`--help` 显示完整参数说明

### Task 4: 时区迁移（全局Utc→Local）
- **ID**: task-4
- **type**: quick-fix
- **描述**: 替换所有`Utc::now()`为`Local::now()`，修改相关类型从`DateTime<Utc>`到`DateTime<Local>`
- **文件范围**:
  - `core/src/tool_event/stream_json.rs`
  - `core/src/engine/run.rs`
  - `core/src/engine/post.rs`
  - `core/src/engine/pre.rs`
  - `core/src/runner/abort.rs`
  - `core/src/runner/policy.rs`
  - `core/src/gatekeeper/evaluate.rs`
  - `core/src/gatekeeper/trait.rs`
  - `core/src/replay/eval.rs`
  - `core/src/memory/payloads.rs`
  - `plugins/src/gatekeeper/standard.rs`
- **依赖**: 无（可与Task 1-3并行）
- **测试命令**:
  ```bash
  cargo test --workspace -- --nocapture --test-threads=1
  cargo clippy --workspace --all-targets -- -D warnings
  ```
- **测试重点**:
  - 编译通过：所有DateTime类型推断正确
  - 单元测试通过：时间戳相关测试适配Local时区
  - 功能验证：`run.events.jsonl`中时间戳使用系统本地时区（如+08:00）
  - 回放验证：replay/resume命令正确解析Local时区时间戳

## 验收标准
- [ ] HTTP服务器成功启动在8080端口，监听localhost
- [ ] 6个API端点全部实现且返回正确响应格式
- [ ] POST /api/v1/search 正确调用MemoryPlugin::search并返回结果
- [ ] POST /api/v1/record-candidate 正确调用MemoryPlugin::record_candidate
- [ ] POST /api/v1/record-hit 正确调用MemoryPlugin::record_hit
- [ ] POST /api/v1/validate 正确调用MemoryPlugin::record_validation
- [ ] GET /health 返回服务器统计（请求数、启动时间）
- [ ] POST /api/v1/shutdown 触发优雅关闭（等待活跃连接最多30秒）
- [ ] CORS中间件仅允许localhost源（拒绝其他来源）
- [ ] 请求日志记录方法、路径、状态码、耗时
- [ ] 状态文件正确写入~/.memex/servers/http-8080.pid（启动时创建，关闭时删除）
- [ ] 所有Utc::now()已替换为Local::now()（11个文件）
- [ ] CLI子命令`memex-cli http-server --help`显示完整帮助
- [ ] 配置文件config.toml包含[http_server]节且正确加载
- [ ] 所有单元测试通过（cargo test --workspace）
- [ ] 代码覆盖率≥90%（关键路径：routes handlers、validation、server lifecycle）
- [ ] 集成测试通过：完整启动→调用API→优雅关闭流程

## 技术说明

### 架构决策
1. **直接调用MemoryPlugin而非subprocess**:
   - 服务器持有`Arc<dyn MemoryPlugin>`，直接调用trait方法
   - 避免Python版本的subprocess开销
   - 复用memex-cli现有的MemoryServicePlugin实现

2. **移除AI验证，使用Gatekeeper机制**:
   - Python版本的validate_knowledge_llm（AI验证）逻辑移除
   - POST /api/v1/record-candidate仅做基础验证（字段长度、格式检查）
   - 依赖现有GatekeeperPlugin在record_candidate后执行质量门控

3. **固定端口8080和CORS策略**:
   - 默认端口8080（可通过--port覆盖）
   - CORS仅允许`http://localhost:*`（防止外部恶意站点访问）
   - 生产环境建议通过反向代理（nginx/caddy）添加认证

4. **优雅关闭流程**:
   - 监听SIGTERM/SIGINT信号
   - 停止接受新连接，等待活跃连接完成（最多30秒）
   - 删除状态文件~/.memex/servers/http-8080.pid

5. **时区统一为Local**:
   - memex-cli的事件日志、回放功能统一使用本地时区
   - 避免跨时区使用时的混淆（用户看到的时间与系统时间一致）
   - 所有DateTime<Utc>改为DateTime<Local>

### 依赖版本约束
- axum = "0.7" (稳定版，与tower生态兼容)
- tower = "0.4" (中间件基础)
- tower-http = { version = "0.5", features = ["cors", "trace", "timeout"] }
- tokio需要启用"signal"特性（优雅关闭）

### 测试策略
- **单元测试**: 每个模块独立测试（models、validation、routes、middleware）
- **集成测试**: `cli/tests/http_server_integration.rs`完整生命周期测试
  - 启动服务器
  - 使用reqwest客户端调用所有6个端点
  - 验证响应格式和状态码
  - 触发shutdown，验证优雅关闭
- **覆盖率目标**: 90%，重点覆盖：
  - routes handlers（正常路径+错误处理）
  - validation函数（边界值）
  - server启动/关闭逻辑

### 风险与缓解
- **风险1**: MemoryPlugin trait的async调用可能阻塞HTTP请求处理
  - **缓解**: 使用tower-http的TimeoutLayer，30秒超时
- **风险2**: 状态文件路径在Windows下可能权限不足
  - **缓解**: 使用dirs crate获取用户目录，添加文件操作错误处理
- **风险3**: 时区迁移可能破坏现有事件日志解析
  - **缓解**: 保持ISO 8601格式（仅时区后缀从Z变为+HH:MM），replay逻辑兼容两种格式
