use winspot_bench::{BenchmarkConfig, SearchBenchmark};
use winspot_search::engine::SearchEngine;

fn main() -> anyhow::Result<()> {
    let engine = SearchEngine::default();
    let report = SearchBenchmark::new(BenchmarkConfig::default()).run(&engine);
    println!("{}", report.to_human_readable());
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
