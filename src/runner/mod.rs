mod exit;
mod outcome;
mod spawn;
mod tee;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::cli::Args;
use crate::error::RunnerError;
use crate::config::{load_default, PolicyConfig, PolicyRule};
use crate::events_out::{start_events_out, EventsOutTx};
use crate::events_out::write_wrapper_event;
use crate::gatekeeper::{Gatekeeper, GatekeeperConfig};
use crate::memory::{
    build_candidate_payloads, build_hit_payload, build_validate_payloads, extract_candidates,
    merge_prompt, parse_search_matches, render_memory_context, CandidateExtractConfig, MemoryClient,
    QASearchPayload,
};
use crate::tool_event::ToolEvent;
use crate::tool_event::ToolEventLite;
use crate::tool_event::PrefixedJsonlParser;
use crate::tool_event::ToolEventRuntime;
use crate::util::RingBytes;
use crate::tool_event::WrapperEvent;

pub use outcome::RunOutcome;

pub async fn run(args: Args) -> Result<i32, RunnerError> {
    let cfg = load_default().map_err(|e| RunnerError::Spawn(e.to_string()))?;

    let user_query = args.codecli_args.join(" ");

    let events_out_tx = start_events_out(&cfg.events_out)
        .await
        .map_err(RunnerError::Spawn)?;

    let memory = if cfg.memory.enabled && !cfg.memory.base_url.trim().is_empty() {
        Some(
            MemoryClient::new(
                cfg.memory.base_url.clone(),
                cfg.memory.api_key.clone(),
                cfg.memory.timeout_ms,
            )
            .map_err(|e| RunnerError::Spawn(e.to_string()))?,
        )
    } else {
        None
    };

    let run_id = uuid::Uuid::new_v4().to_string();

    let (_merged_query, shown_qa_ids) = build_merged_prompt(
        memory.as_ref(),
        &cfg.project_id,
        &user_query,
        &cfg.gatekeeper,
        events_out_tx.as_ref(),
        &run_id,
    )
    .await;
    let mut start_event = WrapperEvent::new("runner.start", Utc::now().to_rfc3339());
    start_event.run_id = Some(run_id.clone());
    start_event.data = Some(serde_json::json!({
        "cmd": args.codecli_bin.clone(),
        "args": args.codecli_args.clone(),
    }));
    write_wrapper_event(events_out_tx.as_ref(), &start_event).await;

    let mut child = spawn::spawn(&args)?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdin = child.stdin.take().unwrap();

    let ring_out = RingBytes::new(args.capture_bytes);
    let ring_err = RingBytes::new(args.capture_bytes);

    let (line_tx, mut line_rx) = mpsc::channel::<tee::LineTap>(1024);
    let out_task = tee::pump_stdout(stdout, ring_out.clone(), line_tx.clone());
    let err_task = tee::pump_stderr(stderr, ring_err.clone(), line_tx);

    let (ctl_tx, mut ctl_rx) = mpsc::channel::<serde_json::Value>(128);
    let mut ctl = ControlChannel::new(stdin);
    let fail_closed = cfg.control.fail_mode.as_str() == "closed";

    let (writer_err_tx, mut writer_err_rx) = mpsc::channel::<String>(1);
    let ctl_task = tokio::spawn(async move {
        while let Some(v) = ctl_rx.recv().await {
            if let Err(e) = ctl.send(&v).await {
                let _ = writer_err_tx
                    .send(format!("stdin write failed: {}", e))
                    .await;
                break;
            }
        }
    });

    let _policy = PolicyEngine::new(cfg.policy.clone());

    let pending: HashMap<String, Instant> = HashMap::new();
    let decision_timeout = Duration::from_millis(cfg.control.decision_timeout_ms);
    let mut tick = tokio::time::interval(Duration::from_millis(1000));

    let parser = PrefixedJsonlParser::new("@@MEM_TOOL_EVENT@@");
    let mut tool_runtime = ToolEventRuntime::new(parser, events_out_tx.clone());

    let (exit_status, abort_reason) = {
        let wait_fut = child.wait();
        tokio::pin!(wait_fut);

        let mut status = None;
        let mut reason = None;

        loop {
            tokio::select! {
                res = &mut wait_fut => {
                    status = Some(res.map_err(|e: std::io::Error| RunnerError::Spawn(e.to_string())));
                    break;
                }

                maybe_err = writer_err_rx.recv() => {
                    if let Some(msg) = maybe_err {
                        tracing::error!(error.kind="control.stdin_broken", error.message=%msg);
                        if fail_closed {
                            reason = Some("control channel broken");
                            break;
                        } else {
                            tracing::warn!("control channel broken, continuing in fail-open mode");
                        }
                    }
                }

                tap = line_rx.recv() => {
                    if let Some(tap) = tap {
                        tool_runtime.observe_line(&tap.line).await;
                    }
                }

                _ = tick.tick() => {
                     let now = Instant::now();
                     let mut timed_out = false;
                     for (_, t0) in pending.iter() {
                         if now.duration_since(*t0) > decision_timeout {
                             timed_out = true;
                             break;
                         }
                     }
                     if timed_out {
                         tracing::error!(error.kind="control.decision_timeout");
                         if fail_closed {
                             reason = Some("decision timeout");
                             break;
                         }
                     }
                }
            }
        }
        (status, reason)
    };

    if let Some(reason) = abort_reason {
        abort_sequence(&mut child, &ctl_tx, &run_id, cfg.control.abort_grace_ms, reason).await;
        return Ok(40);
    }

    drop(ctl_tx);
    ctl_task.abort();
    out_task.await.ok();
    err_task.await.ok();

    let status = exit_status.unwrap()?;
    let exit_code = exit::normalize_exit(status);
    let start_time = Instant::now();

    let stdout_tail = String::from_utf8_lossy(&ring_out.to_bytes()).to_string();
    let stderr_tail = String::from_utf8_lossy(&ring_err.to_bytes()).to_string();

    let tool_events = tool_runtime.take_events();
    let dropped = tool_runtime.dropped_events_out();
    if dropped > 0 {
        let mut ev = WrapperEvent::new("tee.drop", Utc::now().to_rfc3339());
        ev.run_id = Some(run_id.clone());
        ev.data = Some(serde_json::json!({ "dropped_lines": dropped }));
        write_wrapper_event(events_out_tx.as_ref(), &ev).await;
    }

    let matches = vec![];

    let run_outcome = RunOutcome {
        exit_code,
        duration_ms: Some(start_time.elapsed().as_millis() as u64),
        stdout_tail: stdout_tail.clone(),
        stderr_tail: stderr_tail.clone(),
        tool_events: tool_events.clone(),
        shown_qa_ids: shown_qa_ids.clone(),
        used_qa_ids: crate::gatekeeper::extract_qa_refs(&stdout_tail),
    };

    let decision = Gatekeeper::evaluate(&cfg.gatekeeper, Utc::now(), &matches, &run_outcome);
    let mut decision_event = WrapperEvent::new("gatekeeper.decision", Utc::now().to_rfc3339());
    decision_event.run_id = Some(run_id.clone());
    decision_event.data = Some(serde_json::json!({
        "decision": serde_json::to_value(&decision).unwrap_or(serde_json::Value::Null),
    }));
    write_wrapper_event(events_out_tx.as_ref(), &decision_event).await;

    if let Some(mem) = &memory {
        let cand_cfg = CandidateExtractConfig::default();
        let tool_events_lite: Vec<ToolEventLite> = tool_events.iter().map(|e| e.into()).collect();

        let candidate_drafts = if decision.should_write_candidate {
            extract_candidates(
                &cand_cfg,
                &user_query,
                &run_outcome.stdout_tail,
                &run_outcome.stderr_tail,
                &tool_events_lite,
            )
        } else {
            vec![]
        };

        post_run_memory_reporting(mem, &cfg.project_id, &decision, candidate_drafts).await;
    }

    let mut exit_event = WrapperEvent::new("runner.exit", Utc::now().to_rfc3339());
    exit_event.run_id = Some(run_id);
    exit_event.data = Some(serde_json::json!({
        "exit_code": run_outcome.exit_code,
        "duration_ms": run_outcome.duration_ms,
        "stdout_tail": run_outcome.stdout_tail,
        "stderr_tail": run_outcome.stderr_tail,
        "used_qa_ids": run_outcome.used_qa_ids,
        "shown_qa_ids": run_outcome.shown_qa_ids,
    }));
    write_wrapper_event(events_out_tx.as_ref(), &exit_event).await;

    Ok(exit_code)
}

async fn post_run_memory_reporting(
    mem: &MemoryClient,
    project_id: &str,
    decision: &crate::gatekeeper::GatekeeperDecision,
    candidate_drafts: Vec<crate::memory::CandidateDraft>,
) {
    if let Some(hit_payload) = build_hit_payload(project_id, decision) {
        let _ = mem.send_hit(hit_payload).await;
    }

    for v in build_validate_payloads(project_id, decision) {
        let _ = mem.send_validate(v).await;
    }

    if decision.should_write_candidate && !candidate_drafts.is_empty() {
        let payloads = build_candidate_payloads(project_id, &candidate_drafts);
        for c in payloads {
            let _ = mem.send_candidate(c).await;
        }
    }
}

async fn abort_sequence(
    child: &mut tokio::process::Child,
    ctl_tx: &mpsc::Sender<serde_json::Value>,
    run_id: &str,
    abort_grace_ms: u64,
    reason: &str,
) {
    let abort = PolicyAbortCmd::new(run_id.to_string(), reason.to_string(), Some("policy_violation".into()));
    let _ = ctl_tx.send(serde_json::to_value(abort).unwrap()).await;
    tokio::time::sleep(Duration::from_millis(abort_grace_ms)).await;
    let _ = child.kill().await;
}

async fn build_merged_prompt(
    memory: Option<&MemoryClient>,
    project_id: &str,
    user_query: &str,
    gk_cfg: &GatekeeperConfig,
    events_out: Option<&EventsOutTx>,
    run_id: &str,
) -> (String, Vec<String>) {
    if memory.is_none() {
        return (user_query.to_string(), vec![]);
    }
    let mem = memory.unwrap();

    let payload = QASearchPayload {
        project_id: project_id.to_string(),
        query: user_query.to_string(),
        limit: 6,
        min_score: 0.2,
    };

    let raw_res = mem.search(payload).await;
    if let Err(e) = raw_res {
        tracing::warn!("memory search failed: {}", e);
        return (user_query.to_string(), vec![]);
    }
    let raw = raw_res.unwrap();
    let mut ev = WrapperEvent::new("memory.search.result", Utc::now().to_rfc3339());
    ev.run_id = Some(run_id.to_string());
    ev.data = Some(serde_json::json!({
        "query": user_query,
        "matches": raw.clone(),
    }));
    write_wrapper_event(events_out, &ev).await;

    let matches = match parse_search_matches(&raw) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("parse search matches failed: {}", e);
            vec![]
        }
    };

    let dummy_run = RunOutcome {
        exit_code: 0,
        duration_ms: None,
        stdout_tail: "".to_string(),
        stderr_tail: "".to_string(),
        tool_events: vec![],
        shown_qa_ids: vec![],
        used_qa_ids: vec![],
    };

    let decision = Gatekeeper::evaluate(gk_cfg, Utc::now(), &matches, &dummy_run);

    let inject_cfg = crate::memory::InjectConfig::default();
    let memory_ctx = render_memory_context(&decision.inject_list, &inject_cfg);
    let merged = merge_prompt(user_query, &memory_ctx);
    let shown = decision.inject_list.iter().map(|x| x.qa_id.clone()).collect();

    (merged, shown)
}

pub struct ControlChannel {
    stdin: tokio::process::ChildStdin,
}

impl ControlChannel {
    pub fn new(stdin: tokio::process::ChildStdin) -> Self {
        Self { stdin }
    }

    pub async fn send<T: Serialize>(&mut self, msg: &T) -> std::io::Result<()> {
        let line = serde_json::to_string(msg).unwrap();
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await
    }
}

#[derive(Debug, Serialize)]
struct PolicyAbortCmd {
    pub v: u8,
    #[serde(rename = "type")]
    pub ty: &'static str,
    pub ts: String,
    pub run_id: String,
    pub id: String,
    pub reason: String,
    pub code: Option<String>,
}

impl PolicyAbortCmd {
    pub fn new(run_id: String, reason: String, code: Option<String>) -> Self {
        Self {
            v: 1,
            ty: "policy.abort",
            ts: chrono::Utc::now().to_rfc3339(),
            run_id,
            id: "abort-1".into(),
            reason,
            code,
        }
    }
}

#[derive(Clone)]
pub struct PolicyEngine {
    cfg: PolicyConfig,
}

#[derive(Debug)]
pub struct PolicyDecision {
    pub decision: &'static str,
    pub reason: String,
    pub rule_id: Option<String>,
}

impl PolicyEngine {
    pub fn new(cfg: PolicyConfig) -> Self {
        Self { cfg }
    }

    pub fn decide(&self, req: &ToolEvent) -> PolicyDecision {
        if self.cfg.mode == "off" {
            return PolicyDecision {
                decision: "allow",
                reason: "policy off".into(),
                rule_id: Some("policy.off".into()),
            };
        }

        if let Some((idx, rule)) = find_match(&self.cfg.denylist, req) {
            return PolicyDecision {
                decision: "deny",
                reason: rule.reason.clone().unwrap_or_else(|| "denied by rule".into()),
                rule_id: Some(format!("denylist[{}]", idx)),
            };
        }

        if let Some((idx, rule)) = find_match(&self.cfg.allowlist, req) {
            return PolicyDecision {
                decision: "allow",
                reason: rule.reason.clone().unwrap_or_else(|| "allowed by rule".into()),
                rule_id: Some(format!("allowlist[{}]", idx)),
            };
        }

        let d = self.cfg.default_action.as_str();
        if d == "allow" {
            PolicyDecision {
                decision: "allow",
                reason: "allowed by default_action".into(),
                rule_id: Some("default.allow".into()),
            }
        } else {
            PolicyDecision {
                decision: "deny",
                reason: "denied by default_action".into(),
                rule_id: Some("default.deny".into()),
            }
        }
    }
}

fn find_match<'a>(rules: &'a [PolicyRule], req: &ToolEvent) -> Option<(usize, &'a PolicyRule)> {
    let tool_name = req.tool.as_deref().unwrap_or("");
    for (i, r) in rules.iter().enumerate() {
        if !tool_match(&r.tool, tool_name) {
            continue;
        }
        if let Some(a) = &r.action {
            let ra = a.as_str();
            let qa = req.action.as_deref().unwrap_or("");
            if ra != qa {
                continue;
            }
        }
        return Some((i, r));
    }
    None
}

fn tool_match(pat: &str, tool: &str) -> bool {
    if pat == "*" {
        return true;
    }
    if pat.ends_with('*') {
        let prefix = pat.trim_end_matches('*');
        return tool.starts_with(prefix);
    }
    tool.starts_with(pat)
}





