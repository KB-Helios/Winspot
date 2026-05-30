use std::time::Duration;

use winspot_bench::{BenchmarkConfig, SearchBenchmark, summarize_latencies};
use winspot_search::{engine::SearchEngine, providers::BuiltinCommandProvider};

#[test]
fn summarize_latencies_reports_core_percentiles() {
    let summary = summarize_latencies(&[
        Duration::from_micros(100),
        Duration::from_micros(200),
        Duration::from_micros(300),
        Duration::from_micros(400),
        Duration::from_micros(500),
    ])
    .expect("latency summary exists");

    assert_eq!(summary.min_micros, 100);
    assert_eq!(summary.average_micros, 300);
    assert_eq!(summary.p95_micros, 500);
    assert_eq!(summary.max_micros, 500);
}

#[test]
fn search_benchmark_runs_warm_queries_against_engine() {
    let engine = SearchEngine::from_results(BuiltinCommandProvider.collect_results());
    let benchmark = SearchBenchmark::new(BenchmarkConfig {
        queries: vec!["calc".to_string(), "terminal".to_string()],
        warmup_iterations: 2,
        measured_iterations: 4,
        result_limit: 10,
    });

    let report = benchmark.run(&engine);

    assert_eq!(report.queries.len(), 2);
    assert!(
        report
            .queries
            .iter()
            .all(|query| query.iterations == 4 && query.cache_hits_after > query.cache_hits_before)
    );
}
