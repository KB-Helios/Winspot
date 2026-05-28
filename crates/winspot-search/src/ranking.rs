use std::cmp::Ordering;

use winspot_core::SearchResult;

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
    let mut scored = results
        .into_iter()
        .map(|mut result| {
            result.score = score_match(query, &result.title);
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
