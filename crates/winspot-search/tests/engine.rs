use std::{cell::Cell, rc::Rc};

use winspot_core::{ActionKind, SearchResult, SearchResultKind};
use winspot_search::{
    engine::SearchEngine,
    providers::{BuiltinCommandProvider, SearchProvider},
};

#[test]
fn search_returns_ranked_builtin_command_results() {
    let results = SearchEngine::default().search("calc", 5);

    assert_eq!(results[0].title, "Calculator");
    assert!(results[0].score > 0.0);
}

#[test]
fn search_engine_collects_provider_candidates_once() {
    let collection_count = Rc::new(Cell::new(0));
    let provider = CountingProvider {
        collection_count: Rc::clone(&collection_count),
    };

    let engine = SearchEngine::from_providers(vec![Box::new(provider)]);

    assert_eq!(collection_count.get(), 1);

    let first = engine.search("sample", 10);
    let second = engine.search("app", 10);

    assert_eq!(first[0].title, "Sample App");
    assert_eq!(second[0].title, "Sample App");
    assert_eq!(collection_count.get(), 1);
}

#[test]
fn search_engine_can_rank_explicit_candidates() {
    let engine = SearchEngine::from_results(BuiltinCommandProvider::default().collect_results());

    let results = engine.search("term", 5);

    assert_eq!(results[0].title, "Terminal");
}

struct CountingProvider {
    collection_count: Rc<Cell<u32>>,
}

impl SearchProvider for CountingProvider {
    fn collect_results(&self) -> Vec<SearchResult> {
        self.collection_count.set(self.collection_count.get() + 1);
        vec![SearchResult {
            id: "app:sample".to_string(),
            title: "Sample App".to_string(),
            subtitle: "Test app".to_string(),
            kind: SearchResultKind::App,
            score: 0.0,
            primary_action: ActionKind::Open,
        }]
    }
}
