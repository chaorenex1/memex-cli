pub mod aggregate;
pub mod diff;
pub mod eval;
pub mod model;
pub mod override_;
pub mod parse;
pub mod report;

use crate::config::load_default;
use crate::cli::ReplayArgs;

pub use parse::parse_events_file;

pub fn replay_cmd(args: ReplayArgs) -> Result<(), String> {
    let runs = aggregate::replay_events_file(&args.events, args.run_id.as_deref())?;
    let mut runs = aggregate::aggregate_runs(runs);

    if args.rerun_gatekeeper {
        let base_cfg = load_default().map_err(|e| e.to_string())?;
        let gk_cfg = override_::apply_overrides(base_cfg.gatekeeper, &args.set)?;

        for run in runs.iter_mut() {
            let rerun = eval::rerun_gatekeeper_for_run(run, &gk_cfg);
            let baseline = run
                .gatekeeper_decision
                .as_ref()
                .and_then(|w| w.data.as_ref())
                .and_then(|d| d.get("decision"));
            let diff = diff::diff_gatekeeper_decision(baseline, &rerun.decision_json);

            run.derived = serde_json::json!({
                "rerun_gatekeeper": {
                    "skipped": rerun.skipped,
                    "skip_reason": rerun.skip_reason,
                    "decision": rerun.decision_json,
                    "diff": {
                        "has_baseline": diff.has_baseline,
                        "changed": diff.changed,
                        "summary_lines": diff.summary_lines,
                    },
                },
            });
        }
    }

    let report = report::build_report(&runs);

    if args.format == "json" {
        let s = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        println!("{s}");
    } else {
        let s = report::format_text(&report);
        println!("{s}");
    }

    Ok(())
}
