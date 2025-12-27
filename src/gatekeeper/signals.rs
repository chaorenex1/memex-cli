use regex::Regex;

#[derive(Debug, Clone)]
pub struct ValidationSignal {
    pub result: String,
    pub signal_strength: String,
    pub strong_signal: bool,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct SignalHeuristics {
    pub success_patterns: Vec<Regex>,
    pub fail_patterns: Vec<Regex>,
}

impl Default for SignalHeuristics {
    fn default() -> Self {
        let success = vec![
            Regex::new(r"(?i)\btests?\s+passed\b").unwrap(),
            Regex::new(r"(?i)\ball\s+tests?\s+passed\b").unwrap(),
            Regex::new(r"(?i)\bbuild\s+succeeded\b").unwrap(),
            Regex::new(r"(?i)\bcompile(d)?\s+success(fully)?\b").unwrap(),
            Regex::new(r"(?i)\bfinished\b.*\bsuccess\b").unwrap(),
            Regex::new(r"(?i)\bpass(ed)?\b").unwrap(),
            Regex::new(r"(?i)\bok\b").unwrap(),
        ];

        let fail = vec![
            Regex::new(r"(?i)\bfailed\b").unwrap(),
            Regex::new(r"(?i)\berror\b").unwrap(),
            Regex::new(r"(?i)\bpanic\b").unwrap(),
            Regex::new(r"(?i)\bexception\b").unwrap(),
            Regex::new(r"(?i)\btraceback\b").unwrap(),
        ];

        Self { success_patterns: success, fail_patterns: fail }
    }
}

pub fn grade_validation_signal(
    exit_code: i32,
    stdout_tail: &str,
    stderr_tail: &str,
    used_qa_ids_count: usize,
    heur: &SignalHeuristics,
    failing_tools_count: usize,
) -> ValidationSignal {
    let joined = format!("{stdout_tail}\n{stderr_tail}");

    let is_pass = exit_code == 0;
    let hit_success = heur.success_patterns.iter().any(|re| re.is_match(&joined));
    let hit_fail = heur.fail_patterns.iter().any(|re| re.is_match(&joined));

    let result = if is_pass { "pass" } else { "fail" }.to_string();

    let (signal_strength, strong_signal, reason) = if is_pass
        && hit_success
        && used_qa_ids_count > 0
        && failing_tools_count == 0
    {
        (
            "strong".to_string(),
            true,
            "exit_code=0 + success markers + QA used".to_string(),
        )
    } else if is_pass && (hit_success || used_qa_ids_count > 0) {
        (
            "medium".to_string(),
            false,
            "exit_code=0 but not strong-enough markers".to_string(),
        )
    } else if !is_pass && hit_fail {
        (
            "medium".to_string(),
            false,
            "exit_code!=0 with explicit failure markers".to_string(),
        )
    } else {
        (
            "weak".to_string(),
            false,
            "insufficient evidence for strong/medium".to_string(),
        )
    };

    ValidationSignal { result, signal_strength, strong_signal, reason }
}
