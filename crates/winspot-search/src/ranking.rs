use std::cmp::Ordering;

use winspot_core::SearchResult;

use crate::usage::UsageSnapshot;

const FREQUENCY_WEIGHT: f32 = 22.0;
const RECENCY_WEIGHT: f32 = 35.0;
const RECENCY_WINDOW_SECONDS: u64 = 60 * 60 * 24 * 14;

pub fn score_match(query: &str, candidate: &str) -> f32 {
    let query = query.trim().to_lowercase();
    let candidate = candidate.trim().to_lowercase();

    if query.is_empty() || candidate.is_empty() {
        return 0.0;
    }

    if query == candidate {
        return 1000.0;
    }

    if candidate.starts_with(&query) {
        return 850.0 - candidate.len().saturating_sub(query.len()) as f32;
    }

    if let Some(offset) = candidate.find(&query) {
        return 650.0 - offset as f32;
    }

    let mut score = 0.0;
    let mut search_from = 0;
    for character in query.chars() {
        let remaining = &candidate[search_from..];
        if let Some(offset) = remaining.find(character) {
            score += 100.0 / (1.0 + offset as f32);
            search_from += offset + character.len_utf8();
        } else {
            return 0.0;
        }
    }

    score
}

pub fn rank_results(query: &str, results: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    rank_results_with_usage(query, results, limit, &UsageSnapshot::default(), 0)
}

pub fn rank_results_with_usage(
    query: &str,
    results: Vec<SearchResult>,
    limit: usize,
    usage: &UsageSnapshot,
    now_unix_seconds: u64,
) -> Vec<SearchResult> {
    let mut scored = results
        .into_iter()
        .map(|mut result| {
            let text_score = score_match(query, &result.title);
            result.score = if text_score > 0.0 {
                text_score + usage_boost(&result.id, usage, now_unix_seconds)
            } else {
                0.0
            };
            result
        })
        .filter(|result| result.score > 0.0)
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.title.cmp(&right.title))
    });
    scored.truncate(limit);
    scored
}

fn usage_boost(result_id: &str, usage: &UsageSnapshot, now_unix_seconds: u64) -> f32 {
    let Some(signal) = usage.get(result_id) else {
        return 0.0;
    };

    let frequency = signal.launch_count.min(20) as f32 * FREQUENCY_WEIGHT;
    let recency = if now_unix_seconds == 0 || signal.last_used_unix_seconds == 0 {
        0.0
    } else {
        let age = now_unix_seconds.saturating_sub(signal.last_used_unix_seconds);
        let remaining = RECENCY_WINDOW_SECONDS.saturating_sub(age) as f32;
        (remaining / RECENCY_WINDOW_SECONDS as f32) * RECENCY_WEIGHT
    };

    frequency + recency
}
