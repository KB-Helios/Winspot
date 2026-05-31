use winspot_core::{ActionKind, SearchResult, SearchResultKind};
use winspot_search::{
    ranking::{rank_results, rank_results_with_usage, score_match},
    usage::{UsageSignal, UsageSnapshot},
};

#[test]
fn score_match_rewards_exact_and_prefix_matches() {
    let exact = score_match("notepad", "notepad");
    let prefix = score_match("note", "notepad");
    let fuzzy = score_match("npd", "notepad");
    let miss = score_match("zz", "notepad");

    assert!(exact > prefix);
    assert!(prefix > fuzzy);
    assert!(fuzzy > miss);
    assert_eq!(miss, 0.0);
}

#[test]
fn rank_results_orders_by_score_then_title() {
    let results = vec![
        SearchResult {
            id: "command:calculator".to_string(),
            title: "Calculator".to_string(),
            subtitle: "Command".to_string(),
            kind: SearchResultKind::Command,
            score: 0.0,
            primary_action: ActionKind::RunCommand,
            ..SearchResult::default()
        },
        SearchResult {
            id: "app:notepad".to_string(),
            title: "Notepad".to_string(),
            subtitle: "App".to_string(),
            kind: SearchResultKind::App,
            score: 0.0,
            primary_action: ActionKind::Open,
            ..SearchResult::default()
        },
    ];

    let ranked = rank_results("t", results, 10);

    assert_eq!(ranked[0].title, "Notepad");
    assert!(ranked[0].score > ranked[1].score);
}

#[test]
fn rank_results_with_usage_boosts_frequent_recent_results() {
    let results = vec![
        SearchResult {
            id: "app:notes".to_string(),
            title: "Notes".to_string(),
            subtitle: "Less-used exact-ish match".to_string(),
            kind: SearchResultKind::App,
            score: 0.0,
            primary_action: ActionKind::Open,
            ..SearchResult::default()
        },
        SearchResult {
            id: "app:notepad".to_string(),
            title: "Notepad".to_string(),
            subtitle: "Frequently used editor".to_string(),
            kind: SearchResultKind::App,
            score: 0.0,
            primary_action: ActionKind::Open,
            ..SearchResult::default()
        },
    ];
    let mut usage = UsageSnapshot::default();
    usage.insert(
        "app:notepad".to_string(),
        UsageSignal {
            launch_count: 8,
            last_used_unix_seconds: 2_000,
        },
    );

    let ranked = rank_results_with_usage("note", results, 10, &usage, 2_100);

    assert_eq!(ranked[0].title, "Notepad");
    assert!(ranked[0].score > ranked[1].score);
}

#[test]
fn rank_results_with_usage_does_not_include_text_misses() {
    let results = vec![SearchResult {
        id: "app:notepad".to_string(),
        title: "Notepad".to_string(),
        subtitle: "Frequently used editor".to_string(),
        kind: SearchResultKind::App,
        score: 0.0,
        primary_action: ActionKind::Open,
        ..SearchResult::default()
    }];
    let mut usage = UsageSnapshot::default();
    usage.insert(
        "app:notepad".to_string(),
        UsageSignal {
            launch_count: 20,
            last_used_unix_seconds: 2_000,
        },
    );

    let ranked = rank_results_with_usage("zzzz", results, 10, &usage, 2_100);

    assert!(ranked.is_empty());
}
