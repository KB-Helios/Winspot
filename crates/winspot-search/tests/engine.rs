use winspot_search::engine::SearchEngine;

#[test]
fn search_returns_ranked_builtin_command_results() {
    let results = SearchEngine::default().search("calc", 5);

    assert_eq!(results[0].title, "Calculator");
    assert!(results[0].score > 0.0);
}
