use winspot_core::SearchResult;

use crate::{
    providers::{BuiltinCommandProvider, SearchProvider, StartMenuAppProvider},
    ranking::rank_results,
};

#[derive(Debug, Clone)]
pub struct SearchEngine {
    candidates: Vec<SearchResult>,
}

impl SearchEngine {
    pub fn from_results(candidates: Vec<SearchResult>) -> Self {
        Self { candidates }
    }

    pub fn from_providers(providers: Vec<Box<dyn SearchProvider>>) -> Self {
        let candidates = providers
            .into_iter()
            .flat_map(|provider| provider.collect_results())
            .collect();
        Self { candidates }
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        rank_results(query, self.candidates.clone(), limit)
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::from_providers(vec![
            Box::new(BuiltinCommandProvider),
            Box::new(StartMenuAppProvider::default()),
        ])
    }
}
