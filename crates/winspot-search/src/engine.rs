use winspot_core::SearchResult;

use crate::{
    providers::{BuiltinCommandProvider, FileSystemProvider, SearchProvider, StartMenuAppProvider},
    ranking::rank_results_with_usage,
    usage::UsageSnapshot,
};

#[derive(Debug, Clone)]
pub struct SearchEngine {
    candidates: Vec<SearchResult>,
    usage: UsageSnapshot,
    now_unix_seconds: u64,
}

impl SearchEngine {
    pub fn from_results(candidates: Vec<SearchResult>) -> Self {
        Self::from_results_with_usage(candidates, UsageSnapshot::default(), 0)
    }

    pub fn from_results_with_usage(
        candidates: Vec<SearchResult>,
        usage: UsageSnapshot,
        now_unix_seconds: u64,
    ) -> Self {
        Self {
            candidates,
            usage,
            now_unix_seconds,
        }
    }

    pub fn from_providers(providers: Vec<Box<dyn SearchProvider>>) -> Self {
        let candidates = providers
            .into_iter()
            .flat_map(|provider| provider.collect_results())
            .collect();
        Self::from_results(candidates)
    }

    pub fn from_providers_with_usage(
        providers: Vec<Box<dyn SearchProvider>>,
        usage: UsageSnapshot,
        now_unix_seconds: u64,
    ) -> Self {
        let candidates = providers
            .into_iter()
            .flat_map(|provider| provider.collect_results())
            .collect();
        Self::from_results_with_usage(candidates, usage, now_unix_seconds)
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        rank_results_with_usage(
            query,
            self.candidates.clone(),
            limit,
            &self.usage,
            self.now_unix_seconds,
        )
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::from_providers(vec![
            Box::new(BuiltinCommandProvider),
            Box::new(StartMenuAppProvider::default()),
            Box::new(FileSystemProvider::default()),
        ])
    }
}
