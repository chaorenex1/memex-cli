# 数据流概览
## 1.1 CLI → Runner 主流程

**入口**

- `src/main.rs`
  - `main() -> Result<()>`
    - `let cli = cli::Cli::parse();`
    - `dispatch(cli.command).await`

**命令分发**

- `src/cli.rs`
  - `enum Commands { Run(RunArgs), Replay(ReplayArgs) }`
- `src/main.rs`
  - `async fn dispatch(cmd: Commands) -> Result<()>`
    - `Commands::Run(args) => run_cmd(args).await`
    - `Commands::Replay(args) => replay_cmd(args).await`

**Run 命令执行**

- `src/runner/mod.rs`
  - `pub async fn run_cmd(args: RunArgs) -> Result<i32, AppError>`

推荐内部步骤（顺序固定）：

1. `config::load(args.config_path) -> AppConfig`
2. `events_out::writer::start_events_out(cfg.events_out) -> Option<EventsOutTx>`
3. 写 wrapper 事件：\
   `events_out::helpers::write_wrapper_event(tx, WrapperEvent::runner_start(...))`
4. `memory::client::MemoryClient::new(cfg.memory) -> MemoryClient`
5. `memory.search`（注入前置）
   - `memory::client::search(&client, query) -> serde_json::Value`（顶层 list）
   - `memory::adapters::parse_search_matches(&value) -> Vec<SearchMatch>`
   - 写 wrapper 事件：`memory.search.result`（包含 query + matches）
6. `runner::spawn::spawn_child(args, cfg) -> tokio::process::Child`
7. `runner::tee::tee_child_io(child, tx, &mut parser, &mut collector) -> RunOutcome`
8. `tool_event::linker::correlate_request_result(&collector.events) -> CorrelationStats`
9. `gatekeeper::evaluate::Gatekeeper::evaluate(cfg.gatekeeper, now, &matches, &run_outcome, &collector.events) -> GatekeeperDecision`
10. 写 wrapper 事件：`gatekeeper.decision`
11. `memory.validate`（可选/按决策）

- `memory::client::validate(&client, payload) -> ValidateResp`

12. `candidate_extract`（按 should\_write\_candidate）

- `candidate::extract::extract_candidates(..., tool_events_lite) -> Vec<CandidateDraft>`
- `memory::client::upsert_candidate(...)`（可选）

13. 写 wrapper 事件：`runner.exit`（包含 exit\_code + duration + stdout\_tail/stderr\_tail + shown/used ids）
14. 返回 exit code

---

## 1.2 stdout/stderr tee：逐行 → ToolEvent

这是“在线数据流”的核心：**每一行 stdout/stderr**都可能产出 0..n 个 ToolEvent。

**推荐函数链**

- `src/runner/tee.rs`
  - `pub async fn tee_child_io(...) -> Result<RunOutcome, AppError>`

内部拆为两条任务（stdout/stderr），每行执行同一条函数链：

### A. 读取一行

- `tokio::io::BufReader::new(stdout).lines().next_line().await`

### B. tee 输出（原样转发）

- `println!("{line}")` 或写到外部 sink

### C. 解析工具事件（多 CLI）

- `tool_event::multi_parser::MultiToolEventParser::parse_line(&mut self, line: &str) -> Vec<ToolEvent>`

### D. 收集与落盘（events\_out）

对每个 `ToolEvent ev`：

- `tool_event::collector::ToolEventCollector::push(ev.clone())`
- `events_out::writer::EventsOutTx::send_line(serde_json::to_string(&ev)?)`

> 说明：推荐**统一写纯 JSONL**，让 replay 与在线一致。

---

## 1.3 ToolEvent → 关联指标 → Gatekeeper signals

**关联统计**

- `src/tool_event/linker.rs` 或 `src/tool_event_correlate.rs`
  - `pub fn correlate_request_result(events: &[ToolEvent]) -> CorrelationStats`

**signals 构造**

- `src/gatekeeper/signals.rs`
  - `pub fn build_signals(matches: &[SearchMatch], run: &RunOutcome, tool_corr: &CorrelationStats) -> serde_json::Value`

**Gatekeeper 决策**

- `src/gatekeeper/evaluate.rs`
  - `pub fn evaluate(cfg: &GatekeeperConfig, now: DateTime<Utc>, matches: &[SearchMatch], run: &RunOutcome, tool_events: &[ToolEvent]) -> GatekeeperDecision`

内部调用顺序建议固定为：

1. `let insights = tool_event_insights::build_tool_insights(tool_events);`
2. `let corr = &insights.correlation;`
3. `let signals = gatekeeper::signals::build_signals(..., corr);`
4. `let reasons = gatekeeper_reasons::summarize_tool_corr_anomalies(corr);`
5. fail-closed 规则（如 `dangling_requests > 0`）直接影响决策字段

---

# 2）离线回放数据流（`memwrap replay --events ...`）

离线 replay 的核心要求是：\
**尽量复用在线相同的 ToolEvent 结构与 correlate/gatekeeper 逻辑**，避免两套口径。

## 2.1 CLI → Replay 主流程

- `src/replay/cli.rs`
  - `ReplayArgs { events, run_id, format, set, rerun_gatekeeper }`

- `src/replay/mod.rs`
  - `pub async fn replay_cmd(args: ReplayArgs) -> Result<(), AppError>`

顺序：

1. `replay::aggregate::replay_events_file(path, run_id_filter) -> Vec<ReplayRun>`
2. `replay::report::build_report(runs) -> ReplayReport`
3. 若 `--rerun-gatekeeper`：
   - `replay::override_::apply_overrides(base_cfg, args.set) -> GatekeeperConfig`
   - 对每个 run：
     - `replay::eval::rerun_gatekeeper_for_run(run, &gk_cfg) -> GatekeeperReplayResult`
     - `replay::diff::diff_gatekeeper_decision(baseline, rerun) -> DecisionDiff`
4. 输出：
   - `replay::report::format_text(&report)` 或 `serde_json::to_string_pretty(&report)`

---

## 2.2 events\_out.jsonl → ReplayRun 聚合

**逐行解析**

- `src/replay/parse.rs`
  - `pub fn parse_line(line: &str) -> ReplayLine`
    - `ReplayLine::Tool(ToolEvent)`（type=tool.request/tool.result）
    - `ReplayLine::Wrapper(WrapperEvent)`（runner.start/exit/memory.search.result/gatekeeper.decision）
    - `ReplayLine::Unknown`

**聚合**

- `src/replay/aggregate.rs`（或 `replay/mod.rs` 内）
  - `pub async fn replay_events_file(path: &str, filter_run_id: Option<&str>) -> Result<Vec<ReplayRun>, String>`

聚合策略（函数级）：

- `attach_wrapper(run: &mut ReplayRun, w: WrapperEvent)`
  - runner.start / exit / tee.drop / memory.search.result / gatekeeper.decision
- `run.tool_events.push(tool_event)`
  - tool\_event 通过“当前 run\_id 上下文”挂靠（最近一次 runner.start 的 run\_id）

---

## 2.3 ReplayRun → tool\_corr → rerun gatekeeper

**相关统计**

- 直接复用在线函数：
  - `tool_event::linker::correlate_request_result(&run.tool_events)`

**重跑 Gatekeeper**

- `src/replay/eval.rs`
  - `pub fn rerun_gatekeeper_for_run(run: &ReplayRun, cfg: &GatekeeperConfig) -> GatekeeperReplayResult`

内部需要两个提取函数（建议明确定义）：

1. 从 `runner.exit` 提取 RunOutcome：

- `fn build_run_outcome_from_exit(run: &ReplayRun) -> RunOutcome`

2. 从 `memory.search.result` 提取 matches：

- `memory::adapters::parse_search_matches(&matches_value) -> Vec<SearchMatch>`

然后：

- `Gatekeeper::evaluate(cfg, now, &matches, &outcome, &run.tool_events)`

**决策对比**

- `src/replay/diff.rs`
  - `pub fn diff_gatekeeper_decision(baseline: Option<&Value>, rerun: &Value) -> DecisionDiff`

---

# 3）一张“函数级调用链”速查表

## 在线 run（关键链）

1. `run_cmd()`
2. `start_events_out()`
3. `MemoryClient::search()` → `parse_search_matches()`
4. `spawn_child()`
5. `tee_child_io()`
   - `read_line()`
   - `MultiToolEventParser::parse_line()`
   - `collector.push()`
   - `EventsOutTx::send_line()`
6. `correlate_request_result()`
7. `Gatekeeper::evaluate()` → `build_signals()` / `summarize_tool_corr_anomalies()`
8. `MemoryClient::validate()`（可选）
9. `extract_candidates()` → `MemoryClient::upsert_candidate()`（可选）
10. `write_wrapper_event(runner.exit)`

## 离线 replay（关键链）

1. `replay_cmd()`
2. `replay_events_file()` → `parse_line()` → `attach_wrapper()`
3. `build_report()` → `correlate_request_result()`
4. `apply_overrides()`（可选）
5. `rerun_gatekeeper_for_run()` → `Gatekeeper::evaluate()`（可选）
6. `diff_gatekeeper_decision()`（可选）
7. `format_text()` / `json print`

---