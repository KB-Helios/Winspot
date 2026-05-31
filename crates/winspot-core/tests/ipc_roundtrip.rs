use winspot_core::{
    ActionKind, ActionRequested, CancelRequest, Hello, HelloAccepted, IpcEnvelope, IpcPayload,
    MAX_JSON_LINE_BYTES, MAX_PROTOCOL_VERSION, MIN_PROTOCOL_VERSION, PreviewChunk,
    PreviewRequested, SearchCompleted, SearchResult, SearchResultKind, SearchStarted,
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
fn protocol_hello_round_trips_negotiation_limits() {
    let envelope = IpcEnvelope::request(
        "hello-1",
        IpcPayload::Hello(Hello {
            min_protocol_version: MIN_PROTOCOL_VERSION,
            max_protocol_version: MAX_PROTOCOL_VERSION,
            client_name: "Winspot.App.Tests".to_string(),
        }),
    );

    let json = serde_json::to_string(&envelope).expect("serialize hello");
    assert!(json.contains("\"type\":\"Hello\""));

    let accepted = IpcEnvelope::request(
        "hello-1",
        IpcPayload::HelloAccepted(HelloAccepted {
            protocol_version: MAX_PROTOCOL_VERSION,
            max_json_line_bytes: MAX_JSON_LINE_BYTES,
            server_name: "winspot-daemon".to_string(),
        }),
    );
    let decoded: IpcEnvelope =
        serde_json::from_str(&serde_json::to_string(&accepted).unwrap()).unwrap();
    match decoded.payload {
        IpcPayload::HelloAccepted(hello) => {
            assert_eq!(hello.protocol_version, MAX_PROTOCOL_VERSION);
            assert_eq!(hello.max_json_line_bytes, MAX_JSON_LINE_BYTES);
        }
        other => panic!("expected HelloAccepted, got {other:?}"),
    }
}

#[test]
fn cancellation_and_stream_completion_round_trip() {
    let cancel = IpcEnvelope::request(
        "cancel-1",
        IpcPayload::CancelRequest(CancelRequest {
            request_to_cancel: "query-1".to_string(),
        }),
    );
    let completed = IpcEnvelope::request(
        "query-1",
        IpcPayload::SearchCompleted(SearchCompleted {
            query_id: "query-1".to_string(),
            cancelled: true,
        }),
    );

    let cancel_json = serde_json::to_string(&cancel).expect("serialize cancel");
    let completed_json = serde_json::to_string(&completed).expect("serialize completed");

    assert!(cancel_json.contains("\"type\":\"CancelRequest\""));
    assert!(completed_json.contains("\"cancelled\":true"));
}

#[test]
fn preview_chunk_round_trips_incremental_payload() {
    let envelope = IpcEnvelope::request(
        "preview-1",
        IpcPayload::PreviewChunk(PreviewChunk {
            preview_id: "preview-1".to_string(),
            title: "Roadmap.md".to_string(),
            body: "Loading text preview".to_string(),
            is_final: false,
        }),
    );

    let json = serde_json::to_string(&envelope).expect("serialize preview chunk");
    let decoded: IpcEnvelope = serde_json::from_str(&json).expect("deserialize preview chunk");

    match decoded.payload {
        IpcPayload::PreviewChunk(chunk) => {
            assert_eq!(chunk.preview_id, "preview-1");
            assert!(!chunk.is_final);
        }
        other => panic!("expected PreviewChunk, got {other:?}"),
    }
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
        ..SearchResult::default()
    };

    let json = serde_json::to_string(&result).expect("serialize result");
    assert!(json.contains("\"kind\":\"App\""));
    assert!(json.contains("\"primaryAction\":\"Open\""));
}

#[test]
fn action_request_round_trips_selected_result_metadata() {
    let envelope = IpcEnvelope::request(
        "action-1",
        IpcPayload::ActionRequested(ActionRequested {
            action_id: "action-1".to_string(),
            result_id: "command:calculator".to_string(),
            title: "Calculator".to_string(),
            primary_action: ActionKind::RunCommand,
        }),
    );

    let json = serde_json::to_string(&envelope).expect("serialize action envelope");
    assert!(json.contains("\"type\":\"ActionRequested\""));
    assert!(json.contains("\"resultId\":\"command:calculator\""));

    let decoded: IpcEnvelope = serde_json::from_str(&json).expect("deserialize action envelope");
    match decoded.payload {
        IpcPayload::ActionRequested(action) => {
            assert_eq!(action.action_id, "action-1");
            assert_eq!(action.primary_action, ActionKind::RunCommand);
        }
        other => panic!("expected ActionRequested, got {other:?}"),
    }
}

#[test]
fn preview_request_round_trips_selected_result_metadata() {
    let result = SearchResult {
        id: "file:C:\\Users\\kevin\\Desktop\\Roadmap.md".to_string(),
        title: "Roadmap.md".to_string(),
        subtitle: "C:\\Users\\kevin\\Desktop\\Roadmap.md".to_string(),
        kind: SearchResultKind::File,
        score: 42.0,
        primary_action: ActionKind::Open,
        ..SearchResult::default()
    };
    let envelope = IpcEnvelope::request(
        "preview-1",
        IpcPayload::PreviewRequested(PreviewRequested {
            preview_id: "preview-1".to_string(),
            result,
        }),
    );

    let json = serde_json::to_string(&envelope).expect("serialize preview envelope");
    assert!(json.contains("\"type\":\"PreviewRequested\""));
    assert!(json.contains("\"previewId\":\"preview-1\""));

    let decoded: IpcEnvelope = serde_json::from_str(&json).expect("deserialize preview envelope");
    match decoded.payload {
        IpcPayload::PreviewRequested(preview) => {
            assert_eq!(preview.preview_id, "preview-1");
            assert_eq!(preview.result.title, "Roadmap.md");
        }
        other => panic!("expected PreviewRequested, got {other:?}"),
    }
}
