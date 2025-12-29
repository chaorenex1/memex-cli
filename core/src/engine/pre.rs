use crate::gatekeeper::{GatekeeperPlugin, SearchMatch};
use crate::memory::{
    merge_prompt, render_memory_context, InjectConfig, MemoryPlugin, QASearchPayload,
};
use crate::runner::RunOutcome;
use crate::tool_event::WrapperEvent;

pub(crate) struct EngineContext<'a> {
    pub project_id: &'a str,
    pub inject_cfg: &'a InjectConfig,
    pub memory: Option<&'a dyn MemoryPlugin>,
    pub gatekeeper: &'a dyn GatekeeperPlugin,
    pub memory_search_limit: u32,
    pub memory_min_score: f32,
}

pub(crate) struct PreRun {
    pub merged_query: String,
    pub shown_qa_ids: Vec<String>,
    pub matches: Vec<SearchMatch>,
    pub memory_search_event: Option<WrapperEvent>,
}

pub(crate) async fn pre_run(ctx: &EngineContext<'_>, user_query: &str) -> PreRun {
    let Some(mem) = ctx.memory else {
        return PreRun {
            merged_query: user_query.to_string(),
            shown_qa_ids: vec![],
            matches: vec![],
            memory_search_event: None,
        };
    };

    let payload = QASearchPayload {
        project_id: ctx.project_id.to_string(),
        query: user_query.to_string(),
        limit: ctx.memory_search_limit,
        min_score: ctx.memory_min_score,
    };

    let matches = match mem.search(payload).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("memory search failed: {}", e);
            return PreRun {
                merged_query: user_query.to_string(),
                shown_qa_ids: vec![],
                matches: vec![],
                memory_search_event: None,
            };
        }
    };

    let mut ev = WrapperEvent::new("memory.search.result", chrono::Utc::now().to_rfc3339());
    ev.data = Some(serde_json::json!({
        "query": user_query,
        "matches": matches.clone(),
    }));

    let run_outcome = RunOutcome {
        exit_code: 0,
        duration_ms: None,
        stdout_tail: String::new(),
        stderr_tail: String::new(),
        tool_events: vec![],
        shown_qa_ids: vec![],
        used_qa_ids: vec![],
    };

    let decision = ctx.gatekeeper.evaluate(
        chrono::Utc::now(),
        &matches,
        &run_outcome,
        &run_outcome.tool_events,
    );

    let memory_ctx = render_memory_context(&decision.inject_list, ctx.inject_cfg);
    let merged = merge_prompt(user_query, &memory_ctx);
    let shown = decision
        .inject_list
        .iter()
        .map(|x| x.qa_id.clone())
        .collect();

    PreRun {
        merged_query: merged,
        shown_qa_ids: shown,
        matches,
        memory_search_event: Some(ev),
    }
}
