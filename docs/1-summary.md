# docs/1.md 功能清单（按模块/文件拆分）

## 核心 MVP
- Cargo.toml: 基础依赖（tokio/clap/serde/serde_json/thiserror/tracing），后续扩展项见文中建议
- src/main.rs: 入口；初始化日志；解析 CLI；调用 runner 并返回 exit code
- src/cli.rs: CLI 参数（codecli_bin/codecli_args/capture_bytes），可扩展 config path
- src/error.rs: 统一错误分层（spawn/io）
- src/runner/mod.rs: 进程生命周期；stdout/stderr tee；控制通道；policy 介入；pending 超时；fail-closed/abort；exit code 归一化
- src/runner/codecli.rs: 启动子进程（stdin/stdout/stderr pipe）
- src/runner/tee.rs: stdout/stderr 流式透传；可扩展为按行旁路
- src/runner/control.rs: stdin JSONL 控制通道（policy decision/abort）
- src/protocol/tool_event.rs: ToolEvent 结构与 JSONL 行解析入口
- src/protocol/policy_cmd.rs: policy.decision / policy.abort 命令结构
- src/util/ring.rs: Ring buffer 用于尾部 capture

## 配置 / Policy / Memory
- src/config.rs: TOML + 环境变量覆盖（project_id/control/policy/memory 等）
- src/policy.rs: PolicyEngine（allow/deny list + 默认策略 + 匹配规则）
- src/memory.rs 或 src/memory_client.rs: MemoryClient（search/hit/candidate/validate）
- src/memory_mapping.rs: GatekeeperDecision/RunOutcome -> memory payload 组装
- src/memory_adapters.rs: search matches -> Gatekeeper SearchMatch 适配
- src/prompt_inject.rs: search 结果注入 prompt（system prepend）

## Gatekeeper
- src/gatekeeper/mod.rs: Gatekeeper 总控 evaluate + signals + 决策
- src/gatekeeper/heuristics.rs: 候选/信号启发式
- src/gatekeeper/redact.rs: 脱敏/敏感信息扫描
- src/gatekeeper_reasons.rs: tool 关联异常摘要 reasons

## QA/验证/候选抽取
- src/qa_ref.rs: 提取 [QA_REF ...] 的 used_qa_ids
- src/validate_signal.rs: validate 信号分级（strong/medium/weak）
- src/candidate_extract.rs: 启发式 CandidateDraft 抽取（stdout/stderr/tool_events）

## ToolEvent 采集与分析链路
- src/tool_event.rs: 前缀行解析/格式化（@@MEM_TOOL_EVENT@@）
- src/tool_event_parser.rs: ToolEventParser trait + PrefixedJsonlParser
- src/tool_event_collect.rs: Collector（按行观察）
- src/tool_event_runtime.rs: Parser + events_out + 内存累积
- src/events_out.rs: events_out 异步落盘（backpressure/丢弃策略）
- src/tool_event_lite.rs: ToolEvent -> ToolEventLite
- src/tool_event_insights.rs: 工具事件统计/摘要
- src/tool_steps.rs / src/tool_steps_lite.rs: 从 tool.request 生成可复用步骤
- src/tool_event_correlate.rs: request/result 关联 + unmatched 指标
- src/wrapper_event.rs: wrapper 自身事件（runner.start/exit 等）
- src/events_out_helpers.rs: 写入 wrapper 事件到 events_out

## Replay / 离线回放
- src/replay/cli.rs: memwrap replay 子命令
- src/replay/model.rs: ReplayRun/WrapperEvent/ToolEvent 模型
- src/replay/parse.rs: 解析 events_out（前缀 ToolEvent + 纯 JSON WrapperEvent）
- src/replay/mod.rs: 聚合 run 并回放
- src/replay/report.rs: text/json 报告输出
- src/replay/override.rs / eval.rs / diff.rs: 调参重跑 Gatekeeper 与 diff（可选）

## 测试与夹具
- tests/fixtures/*.txt: codex/gemini/claude 真实输出样本
- tests/common/mod.rs: 通用解析 + unmatched 统计
- tests/tool_events_codex.rs / tool_events_gemini.rs / tool_events_claude.rs: 解析 golden tests
- Cargo.toml(dev-deps): insta snapshot（可选）
