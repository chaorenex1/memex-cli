use serde_json::Value;

use super::model::ReplayRun;

pub fn build_report(runs: &[ReplayRun]) -> Value {
    let mut total_tool_events = 0usize;
    let mut runs_with_exit = 0usize;
    let mut runs_with_drop = 0usize;
    let mut runs_with_search = 0usize;

    let mut run_items = Vec::new();

    for r in runs {
        let tool_count = r.tool_events.len();
        total_tool_events += tool_count;
        if r.runner_exit.is_some() {
            runs_with_exit += 1;
        }
        if r.tee_drop.is_some() {
            runs_with_drop += 1;
        }
        if r.search_result.is_some() {
            runs_with_search += 1;
        }

        run_items.push(serde_json::json!({
            "run_id": r.run_id,
            "tool_events": tool_count,
            "has_exit": r.runner_exit.is_some(),
            "has_drop": r.tee_drop.is_some(),
            "has_search": r.search_result.is_some(),
        }));
    }

    serde_json::json!({
        "totals": {
            "runs": runs.len(),
            "tool_events": total_tool_events,
            "runs_with_exit": runs_with_exit,
            "runs_with_drop": runs_with_drop,
            "runs_with_search": runs_with_search,
        },
        "runs": run_items,
    })
}

pub fn format_text(report: &Value) -> String {
    let mut out = String::new();
    let totals = report.get("totals");

    out.push_str("Replay report\n");
    if let Some(t) = totals {
        out.push_str(&format!("runs: {}\n", t.get("runs").unwrap_or(&Value::Null)));
        out.push_str(&format!(
            "tool_events: {}\n",
            t.get("tool_events").unwrap_or(&Value::Null)
        ));
        out.push_str(&format!(
            "runs_with_exit: {}\n",
            t.get("runs_with_exit").unwrap_or(&Value::Null)
        ));
        out.push_str(&format!(
            "runs_with_drop: {}\n",
            t.get("runs_with_drop").unwrap_or(&Value::Null)
        ));
        out.push_str(&format!(
            "runs_with_search: {}\n",
            t.get("runs_with_search").unwrap_or(&Value::Null)
        ));
    }

    if let Some(runs) = report.get("runs").and_then(|v| v.as_array()) {
        for r in runs {
            out.push_str(&format!("- run_id: {}\n", r.get("run_id").unwrap_or(&Value::Null)));
            out.push_str(&format!("  tool_events: {}\n", r.get("tool_events").unwrap_or(&Value::Null)));
            out.push_str(&format!("  has_exit: {}\n", r.get("has_exit").unwrap_or(&Value::Null)));
            out.push_str(&format!("  has_drop: {}\n", r.get("has_drop").unwrap_or(&Value::Null)));
            out.push_str(&format!("  has_search: {}\n", r.get("has_search").unwrap_or(&Value::Null)));
        }
    }

    out
}
