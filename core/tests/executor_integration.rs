//! Integration tests for the executor module
//!
//! These tests verify end-to-end execution of task graphs with dependencies.
use memex_core::api::StdioTask;
use memex_core::executor::TaskGraph;
use memex_core::stdio::{FilesEncoding, FilesMode};

// Mock task result tracker
// #[derive(Debug, Clone)]
// struct MockTaskTracker {
//     executed_tasks: Arc<Mutex<Vec<String>>>,
//     task_delays: HashMap<String, u64>, // task_id -> delay_ms
// }

// impl MockTaskTracker {
//     fn new() -> Self {
//         Self {
//             executed_tasks: Arc::new(Mutex::new(Vec::new())),
//             task_delays: HashMap::new(),
//         }
//     }

//     fn with_delay(mut self, task_id: &str, delay_ms: u64) -> Self {
//         self.task_delays.insert(task_id.to_string(), delay_ms);
//         self
//     }

//     fn record_execution(&self, task_id: &str) {
//         self.executed_tasks
//             .lock()
//             .unwrap()
//             .push(task_id.to_string());
//     }

//     fn get_executed_tasks(&self) -> Vec<String> {
//         self.executed_tasks.lock().unwrap().clone()
//     }
// }

fn task(id: &str, deps: &[&str]) -> StdioTask {
    StdioTask {
        id: id.to_string(),
        backend: "mock".to_string(),
        workdir: ".".to_string(),
        model: None,
        model_provider: None,
        dependencies: deps.iter().map(|s| s.to_string()).collect(),
        stream_format: "text".to_string(),
        timeout: None,
        retry: None,
        files: vec![],
        files_mode: FilesMode::Auto,
        files_encoding: FilesEncoding::Auto,
        content: format!("Task {} content", id),
    }
}

#[tokio::test]
async fn test_single_task_execution() {
    // Single task with no dependencies should execute successfully
    let tasks = vec![task("A", &[])];

    let graph = TaskGraph::from_tasks(tasks.clone()).unwrap();
    graph.validate().unwrap();
    let stages = graph.topological_sort().unwrap();

    assert_eq!(stages.len(), 1);
    assert_eq!(stages[0], vec!["A".to_string()]);
}

#[tokio::test]
async fn test_linear_dependency_chain() {
    // A -> B -> C should execute in 3 stages
    let tasks = vec![task("A", &[]), task("B", &["A"]), task("C", &["B"])];

    let graph = TaskGraph::from_tasks(tasks).unwrap();
    graph.validate().unwrap();
    let stages = graph.topological_sort().unwrap();

    assert_eq!(stages.len(), 3);
    assert_eq!(stages[0], vec!["A".to_string()]);
    assert_eq!(stages[1], vec!["B".to_string()]);
    assert_eq!(stages[2], vec!["C".to_string()]);
}

#[tokio::test]
async fn test_diamond_dag_execution() {
    //     A
    //    / \
    //   B   C
    //    \ /
    //     D
    let tasks = vec![
        task("A", &[]),
        task("B", &["A"]),
        task("C", &["A"]),
        task("D", &["B", "C"]),
    ];

    let graph = TaskGraph::from_tasks(tasks).unwrap();
    graph.validate().unwrap();
    let stages = graph.topological_sort().unwrap();

    assert_eq!(stages.len(), 3);
    assert_eq!(stages[0], vec!["A".to_string()]);

    // Stage 1: B and C can run in parallel (order preserved from input)
    assert_eq!(stages[1].len(), 2);
    assert!(stages[1].contains(&"B".to_string()));
    assert!(stages[1].contains(&"C".to_string()));

    assert_eq!(stages[2], vec!["D".to_string()]);
}

#[tokio::test]
async fn test_complex_dag_execution() {
    // Complex DAG with multiple parallel stages:
    //       A
    //      /|\
    //     B C D
    //      \|/
    //       E
    let tasks = vec![
        task("A", &[]),
        task("B", &["A"]),
        task("C", &["A"]),
        task("D", &["A"]),
        task("E", &["B", "C", "D"]),
    ];

    let graph = TaskGraph::from_tasks(tasks).unwrap();
    graph.validate().unwrap();
    let stages = graph.topological_sort().unwrap();

    assert_eq!(stages.len(), 3);
    assert_eq!(stages[0], vec!["A".to_string()]);

    // Stage 1: B, C, D can run in parallel
    assert_eq!(stages[1].len(), 3);
    assert!(stages[1].contains(&"B".to_string()));
    assert!(stages[1].contains(&"C".to_string()));
    assert!(stages[1].contains(&"D".to_string()));

    assert_eq!(stages[2], vec!["E".to_string()]);
}

#[tokio::test]
async fn test_circular_dependency_detection() {
    // A -> B -> C -> A (cycle)
    let tasks = vec![task("A", &["C"]), task("B", &["A"]), task("C", &["B"])];

    let graph = TaskGraph::from_tasks(tasks).unwrap();
    let result = graph.validate();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Circular dependency"));
}

#[tokio::test]
async fn test_missing_dependency_detection() {
    // A depends on B, but B doesn't exist
    let tasks = vec![task("A", &["B"])];

    let graph = TaskGraph::from_tasks(tasks).unwrap();
    let result = graph.validate();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Dependency not found"));
    assert!(err.to_string().contains("'B'")); // Check for the missing dependency name
}

#[tokio::test]
async fn test_duplicate_task_id_detection() {
    let tasks = vec![task("A", &[]), task("A", &[])];

    let result = TaskGraph::from_tasks(tasks);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Duplicate task ID"));
}

#[tokio::test]
async fn test_parallel_execution_order_preservation() {
    // All tasks independent, should preserve input order
    let tasks = vec![
        task("C", &[]),
        task("A", &[]),
        task("B", &[]),
        task("D", &[]),
    ];

    let graph = TaskGraph::from_tasks(tasks).unwrap();
    let stages = graph.topological_sort().unwrap();

    assert_eq!(stages.len(), 1);
    // Should preserve original input order: C, A, B, D
    assert_eq!(
        stages[0],
        vec![
            "C".to_string(),
            "A".to_string(),
            "B".to_string(),
            "D".to_string()
        ]
    );
}

#[tokio::test]
async fn test_wide_parallel_fan_out() {
    // One task spawns many parallel tasks:
    //        A
    //   / | | | | \
    //  B  C D E F  G
    let tasks = vec![
        task("A", &[]),
        task("B", &["A"]),
        task("C", &["A"]),
        task("D", &["A"]),
        task("E", &["A"]),
        task("F", &["A"]),
        task("G", &["A"]),
    ];

    let graph = TaskGraph::from_tasks(tasks).unwrap();
    graph.validate().unwrap();
    let stages = graph.topological_sort().unwrap();

    assert_eq!(stages.len(), 2);
    assert_eq!(stages[0], vec!["A".to_string()]);
    assert_eq!(stages[1].len(), 6); // B, C, D, E, F, G
}

#[tokio::test]
async fn test_deep_linear_chain() {
    // Deep chain: A -> B -> C -> D -> E -> F -> G -> H
    let tasks = vec![
        task("A", &[]),
        task("B", &["A"]),
        task("C", &["B"]),
        task("D", &["C"]),
        task("E", &["D"]),
        task("F", &["E"]),
        task("G", &["F"]),
        task("H", &["G"]),
    ];

    let graph = TaskGraph::from_tasks(tasks).unwrap();
    graph.validate().unwrap();
    let stages = graph.topological_sort().unwrap();

    assert_eq!(stages.len(), 8); // One task per stage
    for (i, stage) in stages.iter().enumerate() {
        assert_eq!(stage.len(), 1);
        // Verify correct order
        let expected = format!("{}", (b'A' + i as u8) as char);
        assert_eq!(stage[0], expected);
    }
}
