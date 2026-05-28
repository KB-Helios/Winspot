use winspot_core::{ActionKind, SearchResult, SearchResultKind};
use winspot_search::ranking::{rank_results, score_match};

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
        },
        SearchResult {
            id: "app:notepad".to_string(),
            title: "Notepad".to_string(),
            subtitle: "App".to_string(),
            kind: SearchResultKind::App,
            score: 0.0,
            primary_action: ActionKind::Open,
        },
    ];

    let ranked = rank_results("t", results, 10);

    assert_eq!(ranked[0].title, "Notepad");
    assert!(ranked[0].score > ranked[1].score);
}
