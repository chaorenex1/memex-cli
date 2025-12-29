use crate::error::RunnerError;
use crate::events_out::write_wrapper_event;
use crate::gatekeeper::{GatekeeperDecision, GatekeeperPlugin, SearchMatch};
use crate::memory::{
    build_candidate_payloads, build_hit_payload, build_validate_payloads, CandidateDraft,
    CandidateExtractConfig, MemoryPlugin,
};
use crate::runner::{RunOutcome, RunnerResult};
use crate::tool_event::{ToolEventLite, WrapperEvent};

pub(crate) struct PostRunContext<'a> {
    pub project_id: &'a str,
    pub cand_cfg: &'a CandidateExtractConfig,
    pub memory: Option<&'a dyn MemoryPlugin>,
    pub gatekeeper: &'a dyn GatekeeperPlugin,
    pub events_out: Option<&'a crate::events_out::EventsOutTx>,
}

pub(crate) async fn post_run(
    ctx: &PostRunContext<'_>,
    run: &RunnerResult,
    matches: &[SearchMatch],
    shown_qa_ids: Vec<String>,
    user_query: &str,
) -> Result<(RunOutcome, GatekeeperDecision), RunnerError> {
    let run_outcome = RunOutcome {
        exit_code: run.exit_code,
        duration_ms: run.duration_ms,
        stdout_tail: run.stdout_tail.clone(),
        stderr_tail: run.stderr_tail.clone(),
        tool_events: run.tool_events.clone(),
        shown_qa_ids,
        used_qa_ids: crate::gatekeeper::extract_qa_refs(&run.stdout_tail),
    };

    let decision =
        ctx.gatekeeper
            .evaluate(chrono::Utc::now(), matches, &run_outcome, &run.tool_events);

    let mut decision_event =
        WrapperEvent::new("gatekeeper.decision", chrono::Utc::now().to_rfc3339());
    decision_event.run_id = Some(run.run_id.clone());
    decision_event.data = Some(serde_json::json!({
        "decision": serde_json::to_value(&decision).unwrap_or(serde_json::Value::Null),
    }));
    write_wrapper_event(ctx.events_out, &decision_event).await;

    if let Some(mem) = ctx.memory {

        let tool_events_lite: Vec<ToolEventLite> =
            run.tool_events.iter().map(|e| e.into()).collect();

        let candidate_drafts: Vec<CandidateDraft> = if decision.should_write_candidate {
            crate::memory::extract_candidates(
                ctx.cand_cfg,
                user_query,
                &run_outcome.stdout_tail,
                &run_outcome.stderr_tail,
                &tool_events_lite,
            )
        } else {
            vec![]
        };

        if let Some(hit_payload) = build_hit_payload(ctx.project_id, &decision) {
            let _ = mem.record_hit(hit_payload).await;
        }
        for v in build_validate_payloads(ctx.project_id, &decision) {
            let _ = mem.record_validation(v).await;
        }
        if decision.should_write_candidate && !candidate_drafts.is_empty() {
            let payloads = build_candidate_payloads(ctx.project_id, &candidate_drafts);
            for c in payloads {
                let _ = mem.record_candidate(c).await;
            }
        }
    }

    Ok((run_outcome, decision))
}
