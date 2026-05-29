use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use winspot_core::SearchResult;

use crate::{
    providers::{
        BuiltinCommandProvider, CalculatorProvider, DynamicSearchProvider, FileSystemProvider,
        SearchProvider, StartMenuAppProvider, WindowsSettingsProvider,
    },
    ranking::rank_results_with_usage,
    usage::UsageSnapshot,
};

const DEFAULT_CACHE_LIMIT: usize = 128;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueryCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

#[derive(Clone)]
pub struct SearchEngine {
    candidates: Vec<SearchResult>,
    dynamic_providers: Vec<Arc<dyn DynamicSearchProvider>>,
    usage: UsageSnapshot,
    now_unix_seconds: u64,
    cache: Arc<Mutex<QueryCache>>,
}

impl SearchEngine {
    pub fn from_results(candidates: Vec<SearchResult>) -> Self {
        Self::from_results_with_usage(candidates, UsageSnapshot::default(), 0)
    }

    pub fn from_results_with_cache_limit(
        candidates: Vec<SearchResult>,
        cache_limit: usize,
    ) -> Self {
        Self::from_results_with_usage_and_cache_limit(
            candidates,
            UsageSnapshot::default(),
            0,
            cache_limit,
        )
    }

    pub fn from_results_with_usage(
        candidates: Vec<SearchResult>,
        usage: UsageSnapshot,
        now_unix_seconds: u64,
    ) -> Self {
        Self::from_results_with_usage_and_cache_limit(
            candidates,
            usage,
            now_unix_seconds,
            DEFAULT_CACHE_LIMIT,
        )
    }

    pub fn from_results_with_usage_and_cache_limit(
        candidates: Vec<SearchResult>,
        usage: UsageSnapshot,
        now_unix_seconds: u64,
        cache_limit: usize,
    ) -> Self {
        Self {
            candidates,
            dynamic_providers: Vec::new(),
            usage,
            now_unix_seconds,
            cache: Arc::new(Mutex::new(QueryCache::new(cache_limit))),
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

    pub fn with_dynamic_provider(mut self, provider: Arc<dyn DynamicSearchProvider>) -> Self {
        self.dynamic_providers.push(provider);
        self
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let key = QueryCacheKey::new(query, limit);
        if let Some(results) = self
            .cache
            .lock()
            .expect("query cache lock is not poisoned")
            .get(&key)
        {
            return results;
        }

        let mut candidates = self.candidates.clone();
        for provider in &self.dynamic_providers {
            candidates.extend(provider.search(query));
        }

        let results =
            rank_results_with_usage(query, candidates, limit, &self.usage, self.now_unix_seconds);
        self.cache
            .lock()
            .expect("query cache lock is not poisoned")
            .insert(key, results.clone());
        results
    }

    pub fn cache_stats(&self) -> QueryCacheStats {
        self.cache
            .lock()
            .expect("query cache lock is not poisoned")
            .stats()
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::from_providers(vec![
            Box::new(BuiltinCommandProvider),
            Box::new(WindowsSettingsProvider),
            Box::new(StartMenuAppProvider::default()),
            Box::new(FileSystemProvider::default()),
        ])
        .with_dynamic_provider(Arc::new(CalculatorProvider))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct QueryCacheKey {
    normalized_query: String,
    limit: usize,
}

impl QueryCacheKey {
    fn new(query: &str, limit: usize) -> Self {
        Self {
            normalized_query: query.trim().to_lowercase(),
            limit,
        }
    }
}

#[derive(Debug)]
struct QueryCache {
    limit: usize,
    hits: u64,
    misses: u64,
    entries: HashMap<QueryCacheKey, Vec<SearchResult>>,
    order: VecDeque<QueryCacheKey>,
}

impl QueryCache {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            hits: 0,
            misses: 0,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&mut self, key: &QueryCacheKey) -> Option<Vec<SearchResult>> {
        if let Some(results) = self.entries.get(key) {
            self.hits = self.hits.saturating_add(1);
            Some(results.clone())
        } else {
            self.misses = self.misses.saturating_add(1);
            None
        }
    }

    fn insert(&mut self, key: QueryCacheKey, results: Vec<SearchResult>) {
        if self.limit == 0 {
            return;
        }

        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, results);

        while self.entries.len() > self.limit {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    fn stats(&self) -> QueryCacheStats {
        QueryCacheStats {
            hits: self.hits,
            misses: self.misses,
            entries: self.entries.len(),
        }
    }
}
