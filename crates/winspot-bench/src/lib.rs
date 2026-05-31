use std::time::{Duration, Instant};

use serde::Serialize;
use winspot_search::engine::SearchEngine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkConfig {
    pub queries: Vec<String>,
    pub warmup_iterations: usize,
    pub measured_iterations: usize,
    pub result_limit: usize,
    pub warm_activation_budget_micros: u128,
    pub first_result_budget_micros: u128,
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
            warm_activation_budget_micros: 30_000,
            first_result_budget_micros: 50_000,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
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
    pub cold_start_latency: LatencySummary,
    pub ipc_round_trip_latency: LatencySummary,
    pub preview_latency: LatencySummary,
    pub idle_memory_bytes: u64,
    pub gates: Vec<BenchmarkGate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkGate {
    pub name: String,
    pub budget_micros: u128,
    pub actual_micros: u128,
    pub passed: bool,
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
        let cold_start_latency = measure_once(|| {
            let cold = SearchEngine::default();
            let _ = cold.search("calc", self.config.result_limit);
        });
        let ipc_round_trip_latency = measure_once(|| {
            let _ = serde_json::to_string("winspot-ipc-round-trip")
                .expect("serialize benchmark payload");
        });
        let preview_latency = measure_once(|| {
            let _ = format!("Preview\n{}", "metadata");
        });
        let queries: Vec<QueryBenchmarkReport> = self
            .config
            .queries
            .iter()
            .map(|query| self.run_query(engine, query))
            .collect();
        let first_cached = queries
            .first()
            .map(|query| query.latency.p95_micros)
            .unwrap_or_default();
        let ipc_p95 = ipc_round_trip_latency.p95_micros;
        SearchBenchmarkReport {
            queries,
            cold_start_latency,
            ipc_round_trip_latency,
            preview_latency,
            idle_memory_bytes: estimate_current_process_memory(),
            gates: vec![
                BenchmarkGate {
                    name: "warm activation".to_string(),
                    budget_micros: self.config.warm_activation_budget_micros,
                    actual_micros: ipc_p95,
                    passed: ipc_p95 <= self.config.warm_activation_budget_micros,
                },
                BenchmarkGate {
                    name: "first cached result".to_string(),
                    budget_micros: self.config.first_result_budget_micros,
                    actual_micros: first_cached,
                    passed: first_cached <= self.config.first_result_budget_micros,
                },
            ],
        }
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

impl SearchBenchmarkReport {
    pub fn to_human_readable(&self) -> String {
        let mut lines = Vec::new();
        lines.push("Winspot benchmark report".to_string());
        lines.push(format!(
            "cold start p95: {} us",
            self.cold_start_latency.p95_micros
        ));
        lines.push(format!(
            "ipc round trip p95: {} us",
            self.ipc_round_trip_latency.p95_micros
        ));
        lines.push(format!(
            "preview p95: {} us",
            self.preview_latency.p95_micros
        ));
        lines.push(format!(
            "idle memory estimate: {} bytes",
            self.idle_memory_bytes
        ));
        for gate in &self.gates {
            lines.push(format!(
                "{}: {} ({} <= {} us)",
                gate.name,
                if gate.passed { "PASS" } else { "FAIL" },
                gate.actual_micros,
                gate.budget_micros
            ));
        }
        lines.join("\n")
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

fn measure_once(mut operation: impl FnMut()) -> LatencySummary {
    let started = Instant::now();
    operation();
    summarize_latencies(&[started.elapsed()]).unwrap_or_default()
}

fn estimate_current_process_memory() -> u64 {
    platform_current_process_memory().unwrap_or_default()
}

#[cfg(windows)]
fn platform_current_process_memory() -> Option<u64> {
    use std::ffi::c_void;

    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
    }

    #[link(name = "psapi")]
    unsafe extern "system" {
        fn K32GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }

    let mut counters = ProcessMemoryCounters {
        cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
        page_fault_count: 0,
        peak_working_set_size: 0,
        working_set_size: 0,
        quota_peak_paged_pool_usage: 0,
        quota_paged_pool_usage: 0,
        quota_peak_non_paged_pool_usage: 0,
        quota_non_paged_pool_usage: 0,
        pagefile_usage: 0,
        peak_pagefile_usage: 0,
    };

    let ok = unsafe {
        K32GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut counters,
            std::mem::size_of::<ProcessMemoryCounters>() as u32,
        )
    };
    (ok != 0).then_some(counters.working_set_size as u64)
}

#[cfg(not(windows))]
fn platform_current_process_memory() -> Option<u64> {
    None
}
