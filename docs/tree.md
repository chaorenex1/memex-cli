```
memex-cli/                      # Rust CLI 工程（codecli wrapper）
├─ Cargo.toml
├─ Cargo.lock
├─ src/
│  ├─ main.rs                 # CLI 入口（run / replay）
│  ├─ cli.rs                  # clap 定义（Commands, Args）
│  │
│  ├─ runner/                 # 运行与子进程管理
│  │  ├─ mod.rs
│  │  ├─ spawn.rs             # 启动 codex/claude/gemini
│  │  ├─ tee.rs               # stdout/stderr 流式 tee
│  │  ├─ exit.rs              # 退出码/信号/卡死检测
│  │  └─ outcome.rs           # RunOutcome（exit_code/stdout_tail 等）
│  │
│  ├─ tool_event/             # ToolEvent 核心域（地基）
│  │  ├─ mod.rs
│  │  ├─ model.rs             # ToolEvent 统一结构
│  │  ├─ multi_parser.rs      # MultiToolEventParser（Codex/Claude/Gemini/MCP）
│  │  ├─ linker.rs            # request/result 关联器
│  │  └─ metrics.rs           # unmatched / failed / by_tool
│  │
│  ├─ events_out/             # events_out.jsonl 写入
│  │  ├─ mod.rs
│  │  └─ writer.rs            # 异步 JSONL writer（背压/丢弃策略）
│  │
│  ├─ gatekeeper/             # 质量闸门
│  │  ├─ mod.rs
│  │  ├─ config.rs            # GatekeeperConfig
│  │  ├─ signals.rs           # ToolEvent → signals
│  │  ├─ evaluate.rs          # Gatekeeper::evaluate
│  │  └─ decision.rs          # GatekeeperDecision
│  │
│  ├─ memory/                 # Memory API 客户端
│  │  ├─ mod.rs
│  │  ├─ client.rs            # HTTP client
│  │  ├─ models.rs            # SearchMatch / ValidatePayload
│  │  └─ adapters.rs          # search 结果 → 内部结构
│  │
│  ├─ replay/                 # 离线回放与调试
│  │  ├─ mod.rs
│  │  ├─ parse.rs             # 解析 events_out.jsonl
│  │  ├─ aggregate.rs         # run 聚合
│  │  ├─ report.rs            # text/json 报告
│  │  ├─ eval.rs              # rerun gatekeeper（可选）
│  │  └─ diff.rs              # decision diff
│  │
│  ├─ error.rs                # thiserror 分层错误定义
│  └─ util/                   # 通用工具
│     ├─ mod.rs
│     └─ time.rs
│
├─ tests/
│  ├─ fixtures/
│  │  ├─ codex_out.txt        # 真实 Codex 输出
│  │  ├─ gemini_out.txt       # 真实 Gemini 输出
│  │  └─ claude_out.txt       # 真实 Claude 输出
│  │
│  ├─ common/
│  │  └─ mod.rs               # 测试 helper（unmatched/计数）
│  │
│  ├─ tool_events_codex.rs
│  ├─ tool_events_gemini.rs
│  └─ tool_events_claude.rs
│
└─ README.md
```