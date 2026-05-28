use winspot_core::{
    ActionKind, IpcEnvelope, IpcPayload, SearchResult, SearchResultKind, SearchStarted,
};

#[test]
fn search_request_round_trips_with_protocol_version() {
    let envelope = IpcEnvelope::request(
        "query-1",
        IpcPayload::SearchStarted(SearchStarted {
            query_id: "query-1".to_string(),
            text: "calc".to_string(),
        }),
    );

    let json = serde_json::to_string(&envelope).expect("serialize envelope");
    assert!(json.contains("\"protocolVersion\":1"));
    assert!(json.contains("\"type\":\"SearchStarted\""));

    let decoded: IpcEnvelope = serde_json::from_str(&json).expect("deserialize envelope");
    assert_eq!(decoded.request_id, "query-1");
    assert_eq!(decoded.protocol_version, 1);
}

#[test]
fn search_result_serializes_action_metadata() {
    let result = SearchResult {
        id: "app:notepad".to_string(),
        title: "Notepad".to_string(),
        subtitle: "Windows text editor".to_string(),
        kind: SearchResultKind::App,
        score: 98.5,
        primary_action: ActionKind::Open,
    };

    let json = serde_json::to_string(&result).expect("serialize result");
    assert!(json.contains("\"kind\":\"App\""));
    assert!(json.contains("\"primaryAction\":\"Open\""));
}
