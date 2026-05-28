use winspot_core::SearchResult;

use crate::{
    providers::{BuiltinCommandProvider, StartMenuAppProvider},
    ranking::rank_results,
};

#[derive(Debug, Default)]
pub struct SearchEngine {
    command_provider: BuiltinCommandProvider,
    app_provider: StartMenuAppProvider,
}

impl SearchEngine {
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let mut results = self.command_provider.collect_results();
        results.extend(self.app_provider.collect_results());
        rank_results(query, results, limit)
    }
}
