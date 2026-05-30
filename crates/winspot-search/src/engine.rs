use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use winspot_core::SearchResult;

use crate::{
    providers::{
        BuiltinCommandProvider, CalculatorProvider, DynamicSearchProvider, FileSystemProvider,
        RefreshableProvider, RunningProcessProvider, SearchProvider, StartMenuAppProvider,
        UnitConversionProvider, WindowsSettingsProvider,
    },
    ranking::rank_results_with_usage,
    usage::UsageSnapshot,
};

const DEFAULT_CACHE_LIMIT: usize = 128;

/// Returns the current Unix time in seconds. Injectable so the daemon can use
/// the real clock while tests drive candidate-freshness deterministically.
pub type NowProvider = Arc<dyn Fn() -> u64 + Send + Sync>;

fn constant_now(now_unix_seconds: u64) -> NowProvider {
    Arc::new(move || now_unix_seconds)
}

/// Source of the static candidate set the engine ranks against.
///
/// `Frozen` candidates are collected once (used by tests and explicit result
/// sets). `Refreshable` candidates are re-collected from their providers when
/// they grow older than `ttl_seconds`, so live signals like running processes
/// and newly installed apps stay current without restarting the daemon.
#[derive(Clone)]
enum CandidateSource {
    Frozen(Arc<Vec<SearchResult>>),
    Refreshable(Arc<RefreshableCandidates>),
}

struct RefreshableCandidates {
    providers: Vec<Arc<dyn RefreshableProvider>>,
    ttl_seconds: u64,
    state: Mutex<RefreshState>,
}

struct RefreshState {
    results: Arc<Vec<SearchResult>>,
    collected_at_unix: u64,
    collected: bool,
}

impl CandidateSource {
    fn collect(providers: &[Arc<dyn RefreshableProvider>]) -> Vec<SearchResult> {
        providers
            .iter()
            .flat_map(|provider| provider.collect_results())
            .collect()
    }

    /// Resolves the current candidate set for `now`. For refreshable sources
    /// this re-collects providers when the cached snapshot is stale and clears
    /// the query cache so callers never see results ranked over stale
    /// candidates. Returns `true` when a refresh happened.
    fn resolve(&self, now_unix_seconds: u64) -> (Arc<Vec<SearchResult>>, bool) {
        match self {
            CandidateSource::Frozen(results) => (Arc::clone(results), false),
            CandidateSource::Refreshable(source) => {
                let mut state = source.state.lock().expect("candidate lock is not poisoned");
                let is_stale = !state.collected
                    || now_unix_seconds.saturating_sub(state.collected_at_unix)
                        >= source.ttl_seconds;
                if is_stale {
                    state.results = Arc::new(Self::collect(&source.providers));
                    state.collected_at_unix = now_unix_seconds;
                    state.collected = true;
                    (Arc::clone(&state.results), true)
                } else {
                    (Arc::clone(&state.results), false)
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueryCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

#[derive(Clone)]
pub struct SearchEngine {
    candidates: CandidateSource,
    dynamic_providers: Vec<Arc<dyn DynamicSearchProvider>>,
    usage: Arc<Mutex<UsageSnapshot>>,
    now_provider: NowProvider,
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
            candidates: CandidateSource::Frozen(Arc::new(candidates)),
            dynamic_providers: Vec::new(),
            usage: Arc::new(Mutex::new(usage)),
            now_provider: constant_now(now_unix_seconds),
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

    /// Builds an engine whose static candidates are re-collected from
    /// `providers` whenever the cached snapshot is older than `ttl_seconds`.
    /// `now_provider` supplies the current Unix time used for both freshness
    /// checks and usage-recency ranking.
    pub fn from_refreshable_providers_with_usage(
        providers: Vec<Arc<dyn RefreshableProvider>>,
        usage: UsageSnapshot,
        ttl_seconds: u64,
        now_provider: NowProvider,
    ) -> Self {
        Self {
            candidates: CandidateSource::Refreshable(Arc::new(RefreshableCandidates {
                providers,
                ttl_seconds,
                state: Mutex::new(RefreshState {
                    results: Arc::new(Vec::new()),
                    collected_at_unix: 0,
                    collected: false,
                }),
            })),
            dynamic_providers: Vec::new(),
            usage: Arc::new(Mutex::new(usage)),
            now_provider,
            cache: Arc::new(Mutex::new(QueryCache::new(DEFAULT_CACHE_LIMIT))),
        }
    }

    pub fn with_dynamic_provider(mut self, provider: Arc<dyn DynamicSearchProvider>) -> Self {
        self.dynamic_providers.push(provider);
        self
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let now_unix_seconds = (self.now_provider)();
        let (candidates, refreshed) = self.candidates.resolve(now_unix_seconds);
        if refreshed {
            // Stale candidates were replaced; drop cached batches ranked over them.
            self.cache
                .lock()
                .expect("query cache lock is not poisoned")
                .clear();
        }

        let key = QueryCacheKey::new(query, limit);
        if let Some(results) = self
            .cache
            .lock()
            .expect("query cache lock is not poisoned")
            .get(&key)
        {
            return results;
        }

        let mut candidates = (*candidates).clone();
        for provider in &self.dynamic_providers {
            candidates.extend(provider.search(query));
        }

        let results = {
            let usage = self.usage.lock().expect("usage lock is not poisoned");
            rank_results_with_usage(query, candidates, limit, &usage, now_unix_seconds)
        };
        self.cache
            .lock()
            .expect("query cache lock is not poisoned")
            .insert(key, results.clone());
        results
    }

    /// Records a usage event in the in-memory ranking snapshot and invalidates
    /// cached query batches so subsequent searches reflect the new signal
    /// without requiring a daemon restart.
    pub fn record_usage(&self, result_id: &str, timestamp_unix_seconds: u64) {
        {
            let mut usage = self.usage.lock().expect("usage lock is not poisoned");
            let signal = usage.entry(result_id.to_string()).or_default();
            signal.launch_count = signal.launch_count.saturating_add(1);
            signal.last_used_unix_seconds =
                signal.last_used_unix_seconds.max(timestamp_unix_seconds);
        }
        self.cache
            .lock()
            .expect("query cache lock is not poisoned")
            .clear();
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
            Box::new(RunningProcessProvider),
            Box::new(StartMenuAppProvider::default()),
            Box::new(FileSystemProvider::default()),
        ])
        .with_dynamic_provider(Arc::new(CalculatorProvider))
        .with_dynamic_provider(Arc::new(UnitConversionProvider))
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

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    fn stats(&self) -> QueryCacheStats {
        QueryCacheStats {
            hits: self.hits,
            misses: self.misses,
            entries: self.entries.len(),
        }
    }
}
