pub mod config;
pub mod decision;
pub mod evaluate;
pub mod gatekeeper_reasons;
pub mod signals;

pub use config::GatekeeperConfig;
pub use decision::{GatekeeperDecision, HitRef, InjectItem, SearchMatch, ValidatePlan};
pub use evaluate::Gatekeeper;
pub use signals::{grade_validation_signal, SignalHeuristics, ValidationSignal};

use regex::Regex;
use std::collections::BTreeSet;

pub fn extract_qa_refs(text: &str) -> Vec<String> {
    let re = Regex::new(r"\[QA_REF\s+([A-Za-z0-9_\-]+)\]").expect("valid regex");
    let mut set = BTreeSet::new();

    for cap in re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            set.insert(m.as_str().to_string());
        }
    }

    set.into_iter().collect()
}
