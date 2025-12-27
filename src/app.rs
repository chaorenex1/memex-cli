use chrono::Utc;

use crate::cli::{Args, RunArgs};
use crate::config::load_default;
use crate::error::RunnerError;
use crate::events_out::{start_events_out, EventsOutTx};
use crate::events_out::write_wrapper_event;
use crate::gatekeeper::{Gatekeeper, GatekeeperConfig, SearchMatch};
use crate::memory::{
    build_candidate_payloads, build_hit_payload, build_validate_payloads, extract_candidates,
    merge_prompt, parse_search_matches, render_memory_context, CandidateExtractConfig, MemoryClient,
    QASearchPayload,
};
use crate::runner::{run_child_process, RunOutcome, RunnerResult};
use crate::tool_event::ToolEventLite;
use crate::tool_event::WrapperEvent;

pub async fn run_app(mut args: Args, run_args: Option<RunArgs>, run_id: Option<String>) -> Result<i32, RunnerError> {
    let mut cfg = load_default().map_err(|e| RunnerError::Spawn(e.to_string()))?;

    if let Some(ra) = &run_args {
        args.codecli_bin = ra.backend.clone();
        args.codecli_args = Vec::new();
        
        if let Some(model) = &ra.model {
            args.codecli_args.push("--model".to_string());
            args.codecli_args.push(model.clone());
        }

        if ra.stream {
            args.codecli_args.push("--stream".to_string());
        }

        if let Some(prompt) = &ra.prompt {
            args.codecli_args.push(prompt.clone());
        } else if let Some(path) = &ra.prompt_file {
            let content = std::fs::read_to_string(path)
                .map_err(|e| RunnerError::Spawn(format!("failed to read prompt file: {}", e)))?;
            args.codecli_args.push(content);
        } else if ra.stdin {
            use std::io::Read;
            let mut content = String::new();
            std::io::stdin().read_to_string(&mut content)
                .map_err(|e| RunnerError::Spawn(format!("failed to read prompt from stdin: {}", e)))?;
            args.codecli_args.push(content);
        }

        if let Some(pid) = &ra.project_id {
            cfg.project_id = pid.clone();
        }
        if let Some(url) = &ra.memory_base_url {
            cfg.memory.base_url = url.clone();
        }
        if let Some(key) = &ra.memory_api_key {
            cfg.memory.api_key = key.clone();
        }
    }

    let stream_format = run_args.as_ref().map(|ra| ra.stream_format.as_str()).unwrap_or("text");

    if stream_format == "jsonl" {
        cfg.events_out.enabled = true;
        cfg.events_out.path = "stdout:".to_string();
    }

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

    let run_id = run_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let (_merged_query, shown_qa_ids, matches) = build_merged_prompt(
        memory.as_ref(),
        &cfg.project_id,
        &user_query,
        &cfg.gatekeeper,
        events_out_tx.as_ref(),
        &run_id,
    )
    .await;

    let mut start_event = WrapperEvent::new("run.start", Utc::now().to_rfc3339());
    start_event.run_id = Some(run_id.clone());
    start_event.data = Some(serde_json::json!({
        "cmd": args.codecli_bin.clone(),
        "args": args.codecli_args.clone(),
    }));
    write_wrapper_event(events_out_tx.as_ref(), &start_event).await;

    let run_result = run_child_process(&args, &cfg.control, events_out_tx.clone(), &run_id, stream_format).await?;

    if run_result.dropped_lines > 0 {
        let mut ev = WrapperEvent::new("tee.drop", Utc::now().to_rfc3339());
        ev.run_id = Some(run_id.clone());
        ev.data = Some(serde_json::json!({ "dropped_lines": run_result.dropped_lines }));
        write_wrapper_event(events_out_tx.as_ref(), &ev).await;
    }

    let run_outcome: RunOutcome = build_run_outcome(&run_result, shown_qa_ids);

    let decision = Gatekeeper::evaluate(
        &cfg.gatekeeper,
        Utc::now(),
        &matches,
        &run_outcome,
        &run_result.tool_events,
    );

    let mut decision_event = WrapperEvent::new("gatekeeper.decision", Utc::now().to_rfc3339());
    decision_event.run_id = Some(run_id.clone());
    decision_event.data = Some(serde_json::json!({
        "decision": serde_json::to_value(&decision).unwrap_or(serde_json::Value::Null),
    }));
    write_wrapper_event(events_out_tx.as_ref(), &decision_event).await;

    if let Some(mem) = &memory {
        let cand_cfg = CandidateExtractConfig::default();
        let tool_events_lite: Vec<ToolEventLite> =
            run_result.tool_events.iter().map(|e| e.into()).collect();

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

    let mut exit_event = WrapperEvent::new("run.end", Utc::now().to_rfc3339());
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

    Ok(run_outcome.exit_code)
}

fn build_run_outcome(run: &RunnerResult, shown_qa_ids: Vec<String>) -> RunOutcome {
    RunOutcome {
        exit_code: run.exit_code,
        duration_ms: run.duration_ms,
        stdout_tail: run.stdout_tail.clone(),
        stderr_tail: run.stderr_tail.clone(),
        tool_events: run.tool_events.clone(),
        shown_qa_ids,
        used_qa_ids: crate::gatekeeper::extract_qa_refs(&run.stdout_tail),
    }
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

async fn build_merged_prompt(
    memory: Option<&MemoryClient>,
    project_id: &str,
    user_query: &str,
    gk_cfg: &GatekeeperConfig,
    events_out: Option<&EventsOutTx>,
    run_id: &str,
) -> (String, Vec<String>, Vec<SearchMatch>) {
    if memory.is_none() {
        return (user_query.to_string(), vec![], vec![]);
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
        return (user_query.to_string(), vec![], vec![]);
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

    let run_outcome = RunOutcome {
        exit_code: 0,
        duration_ms: None,
        stdout_tail: "".to_string(),
        stderr_tail: "".to_string(),
        tool_events: vec![],
        shown_qa_ids: vec![],
        used_qa_ids: vec![],
    };

    let decision =
        Gatekeeper::evaluate(gk_cfg, Utc::now(), &matches, &run_outcome, &run_outcome.tool_events);

    let inject_cfg = crate::memory::InjectConfig::default();
    let memory_ctx = render_memory_context(&decision.inject_list, &inject_cfg);
    let merged = merge_prompt(user_query, &memory_ctx);
    let shown = decision.inject_list.iter().map(|x| x.qa_id.clone()).collect();

    (merged, shown, matches)
}
