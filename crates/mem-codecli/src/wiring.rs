use std::sync::Arc;
use memex_core::AppContext;
use memex_core::runner::codecli::CodecliRunner;
use memex_core::gatekeeper::heuristics::SimpleGatekeeper;
use memex_core::policy::ConsoleApprover;
use mem_client::reqwest_client::HttpMemoryClient;
use policy_engine::engine::PolicyEngine;

pub fn build_context() -> Result<AppContext, String> {
    let config = memex_core::config::load::load_default();
    
    let runner = Arc::new(CodecliRunner);
    let memory = Arc::new(HttpMemoryClient::new());
    let gatekeeper = Arc::new(SimpleGatekeeper);
    let policy = Arc::new(PolicyEngine::allow_all());
    let approver = Arc::new(ConsoleApprover);

    Ok(AppContext::new(
        config,
        runner,
        memory,
        gatekeeper,
        policy,
        approver,
    ))
}
