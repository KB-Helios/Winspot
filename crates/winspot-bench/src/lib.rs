use std::time::{Duration, Instant};

use serde::Serialize;
use winspot_search::engine::SearchEngine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkConfig {
    pub queries: Vec<String>,
    pub warmup_iterations: usize,
    pub measured_iterations: usize,
    pub result_limit: usize,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            queries: vec![
                "calc".to_string(),
                "terminal".to_string(),
                "notepad".to_string(),
            ],
            warmup_iterations: 3,
            measured_iterations: 25,
            result_limit: 20,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencySummary {
    pub min_micros: u128,
    pub average_micros: u128,
    pub p95_micros: u128,
    pub max_micros: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryBenchmarkReport {
    pub query: String,
    pub iterations: usize,
    pub result_count: usize,
    pub latency: LatencySummary,
    pub cache_hits_before: u64,
    pub cache_hits_after: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchBenchmarkReport {
    pub queries: Vec<QueryBenchmarkReport>,
}

#[derive(Debug, Clone)]
pub struct SearchBenchmark {
    config: BenchmarkConfig,
}

impl SearchBenchmark {
    pub fn new(config: BenchmarkConfig) -> Self {
        Self { config }
    }

    pub fn run(&self, engine: &SearchEngine) -> SearchBenchmarkReport {
        let queries = self
            .config
            .queries
            .iter()
            .map(|query| self.run_query(engine, query))
            .collect();
        SearchBenchmarkReport { queries }
    }

    fn run_query(&self, engine: &SearchEngine, query: &str) -> QueryBenchmarkReport {
        for _ in 0..self.config.warmup_iterations {
            let _ = engine.search(query, self.config.result_limit);
        }

        let cache_hits_before = engine.cache_stats().hits;
        let mut latencies = Vec::with_capacity(self.config.measured_iterations);
        let mut result_count = 0;
        for _ in 0..self.config.measured_iterations {
            let started = Instant::now();
            let results = engine.search(query, self.config.result_limit);
            latencies.push(started.elapsed());
            result_count = results.len();
        }

        QueryBenchmarkReport {
            query: query.to_string(),
            iterations: self.config.measured_iterations,
            result_count,
            latency: summarize_latencies(&latencies).unwrap_or_default(),
            cache_hits_before,
            cache_hits_after: engine.cache_stats().hits,
        }
    }
}

pub fn summarize_latencies(latencies: &[Duration]) -> Option<LatencySummary> {
    if latencies.is_empty() {
        return None;
    }

    let mut micros = latencies
        .iter()
        .map(Duration::as_micros)
        .collect::<Vec<_>>();
    micros.sort_unstable();

    let total = micros.iter().sum::<u128>();
    let p95_index = ((micros.len() * 95).div_ceil(100)).saturating_sub(1);

    Some(LatencySummary {
        min_micros: micros[0],
        average_micros: total / micros.len() as u128,
        p95_micros: micros[p95_index],
        max_micros: micros[micros.len() - 1],
    })
}

impl Default for LatencySummary {
    fn default() -> Self {
        Self {
            min_micros: 0,
            average_micros: 0,
            p95_micros: 0,
            max_micros: 0,
        }
    }
}
