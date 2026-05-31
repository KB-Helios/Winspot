#[cfg(windows)]
mod windows_tests {
    use std::{fs, time::Duration};

    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::windows::named_pipe::ClientOptions,
        time::timeout,
    };
    use winspot_core::{
        ActionKind, ActionRequested, IpcEnvelope, IpcPayload, MAX_JSON_LINE_BYTES,
        PreviewRequested, SearchResult, SearchResultKind, SearchStarted,
    };
    use winspot_daemon::server::{PipeConfig, build_search_engine, serve_pipe_once};
    use winspot_search::{
        engine::SearchEngine,
        usage::{UsageEvent, UsageStore},
    };

    #[tokio::test]
    async fn daemon_streams_prebuilt_search_results() {
        let pipe_name = format!(r"\\.\pipe\winspot-test-{}", std::process::id());
        let server_name = pipe_name.clone();
        let server = tokio::spawn(async move {
            let engine = SearchEngine::from_results(vec![SearchResult {
                id: "app:sample".to_string(),
                title: "Sample App".to_string(),
                subtitle: "Prebuilt candidate".to_string(),
                kind: SearchResultKind::App,
                score: 0.0,
                primary_action: ActionKind::Open,
                ..SearchResult::default()
            }]);
            serve_pipe_once(
                PipeConfig {
                    pipe_name: server_name,
                    usage_log_path: None,
                },
                &engine,
            )
            .await
            .expect("pipe server completes");
        });

        tokio::time::sleep(Duration::from_millis(25)).await;

        let client = ClientOptions::new()
            .open(&pipe_name)
            .expect("connect to test pipe");
        let mut client = BufReader::new(client);

        let request = IpcEnvelope::request(
            "query-1",
            IpcPayload::SearchStarted(SearchStarted {
                query_id: "query-1".to_string(),
                text: "sample".to_string(),
            }),
        );
        let mut request_json = serde_json::to_string(&request).expect("serialize request");
        request_json.push('\n');
        client
            .get_mut()
            .write_all(request_json.as_bytes())
            .await
            .expect("write request");

        let mut line = String::new();
        timeout(Duration::from_secs(2), client.read_line(&mut line))
            .await
            .expect("response before timeout")
            .expect("read response");

        let response: IpcEnvelope = serde_json::from_str(line.trim()).expect("decode response");
        match response.payload {
            IpcPayload::ResultBatch(batch) => {
                assert_eq!(batch.query_id, "query-1");
                assert!(batch.is_final);
                assert_eq!(batch.batch_index, 0);
                assert!(
                    batch
                        .results
                        .iter()
                        .any(|result| result.title == "Sample App")
                );
            }
            other => panic!("expected ResultBatch, got {other:?}"),
        }

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn daemon_returns_dynamic_calculator_results() {
        let pipe_name = format!(r"\\.\pipe\winspot-calculator-test-{}", std::process::id());
        let server_name = pipe_name.clone();
        let server = tokio::spawn(async move {
            let engine = build_search_engine(&PipeConfig {
                pipe_name: server_name.clone(),
                usage_log_path: None,
            })
            .expect("build search engine");
            serve_pipe_once(
                PipeConfig {
                    pipe_name: server_name,
                    usage_log_path: None,
                },
                &engine,
            )
            .await
            .expect("pipe server completes");
        });

        tokio::time::sleep(Duration::from_millis(25)).await;

        let client = ClientOptions::new()
            .open(&pipe_name)
            .expect("connect to calculator test pipe");
        let mut client = BufReader::new(client);

        let request = IpcEnvelope::request(
            "query-calculator",
            IpcPayload::SearchStarted(SearchStarted {
                query_id: "query-calculator".to_string(),
                text: "2 + 2".to_string(),
            }),
        );
        let mut request_json = serde_json::to_string(&request).expect("serialize request");
        request_json.push('\n');
        client
            .get_mut()
            .write_all(request_json.as_bytes())
            .await
            .expect("write request");

        let mut line = String::new();
        timeout(Duration::from_secs(2), client.read_line(&mut line))
            .await
            .expect("response before timeout")
            .expect("read response");

        let response: IpcEnvelope = serde_json::from_str(line.trim()).expect("decode response");
        match response.payload {
            IpcPayload::ResultBatch(batch) => {
                assert_eq!(batch.query_id, "query-calculator");
                assert!(batch.is_final);
                assert_eq!(batch.batch_index, 0);
                assert_eq!(batch.results[0].title, "2 + 2 = 4");
                assert_eq!(batch.results[0].primary_action, ActionKind::Copy);
            }
            other => panic!("expected ResultBatch, got {other:?}"),
        }

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn daemon_records_usage_after_successful_action() {
        let pipe_name = format!(r"\\.\pipe\winspot-action-test-{}", std::process::id());
        let usage_log_path =
            std::env::temp_dir().join(format!("winspot-action-usage-{}.jsonl", std::process::id()));
        let _ = fs::remove_file(&usage_log_path);
        let server_name = pipe_name.clone();
        let server_usage_log_path = usage_log_path.clone();
        let server = tokio::spawn(async move {
            let engine = SearchEngine::from_results(Vec::new());
            serve_pipe_once(
                PipeConfig {
                    pipe_name: server_name,
                    usage_log_path: Some(server_usage_log_path),
                },
                &engine,
            )
            .await
            .expect("pipe server completes");
        });

        tokio::time::sleep(Duration::from_millis(25)).await;

        let client = ClientOptions::new()
            .open(&pipe_name)
            .expect("connect to action test pipe");
        let mut client = BufReader::new(client);

        let request = IpcEnvelope::request(
            "action-1",
            IpcPayload::ActionRequested(ActionRequested {
                action_id: "action-1".to_string(),
                result_id: "test:copy".to_string(),
                title: "Copy Test".to_string(),
                primary_action: ActionKind::Copy,
            }),
        );
        let mut request_json = serde_json::to_string(&request).expect("serialize action request");
        request_json.push('\n');
        client
            .get_mut()
            .write_all(request_json.as_bytes())
            .await
            .expect("write action request");

        let mut line = String::new();
        timeout(Duration::from_secs(2), client.read_line(&mut line))
            .await
            .expect("action response before timeout")
            .expect("read action response");

        let response: IpcEnvelope =
            serde_json::from_str(line.trim()).expect("decode action response");
        match response.payload {
            IpcPayload::ActionCompleted(completed) => {
                assert_eq!(completed.action_id, "action-1");
                assert!(completed.succeeded);
            }
            other => panic!("expected ActionCompleted, got {other:?}"),
        }

        server.await.expect("server task joins");

        let snapshot = UsageStore::new(usage_log_path.clone())
            .load_snapshot()
            .expect("load usage snapshot");
        let signal = snapshot.get("test:copy").expect("usage signal exists");
        assert_eq!(signal.launch_count, 1);

        fs::remove_file(usage_log_path).expect("cleanup usage log");
    }

    #[tokio::test]
    async fn daemon_returns_metadata_preview_for_selected_result() {
        let pipe_name = format!(r"\\.\pipe\winspot-preview-test-{}", std::process::id());
        let server_name = pipe_name.clone();
        let server = tokio::spawn(async move {
            let engine = SearchEngine::from_results(Vec::new());
            serve_pipe_once(
                PipeConfig {
                    pipe_name: server_name,
                    usage_log_path: None,
                },
                &engine,
            )
            .await
            .expect("pipe server completes");
        });

        tokio::time::sleep(Duration::from_millis(25)).await;

        let client = ClientOptions::new()
            .open(&pipe_name)
            .expect("connect to preview test pipe");
        let mut client = BufReader::new(client);

        let request = IpcEnvelope::request(
            "preview-1",
            IpcPayload::PreviewRequested(PreviewRequested {
                preview_id: "preview-1".to_string(),
                result: SearchResult {
                    id: "file:C:\\Users\\kevin\\Desktop\\Roadmap.md".to_string(),
                    title: "Roadmap.md".to_string(),
                    subtitle: "C:\\Users\\kevin\\Desktop\\Roadmap.md".to_string(),
                    kind: SearchResultKind::File,
                    score: 123.0,
                    primary_action: ActionKind::Open,
                    ..SearchResult::default()
                },
            }),
        );
        let mut request_json = serde_json::to_string(&request).expect("serialize preview request");
        request_json.push('\n');
        client
            .get_mut()
            .write_all(request_json.as_bytes())
            .await
            .expect("write preview request");

        let mut line = String::new();
        timeout(Duration::from_secs(2), client.read_line(&mut line))
            .await
            .expect("preview response before timeout")
            .expect("read preview response");

        let chunk: IpcEnvelope = serde_json::from_str(line.trim()).expect("decode preview chunk");
        match chunk.payload {
            IpcPayload::PreviewChunk(preview) => {
                assert_eq!(preview.preview_id, "preview-1");
                assert_eq!(preview.title, "Roadmap.md");
                assert!(!preview.is_final);
            }
            other => panic!("expected PreviewChunk, got {other:?}"),
        }

        line.clear();
        timeout(Duration::from_secs(2), client.read_line(&mut line))
            .await
            .expect("preview ready before timeout")
            .expect("read preview ready");
        let response: IpcEnvelope =
            serde_json::from_str(line.trim()).expect("decode preview ready");
        match response.payload {
            IpcPayload::PreviewReady(preview) => {
                assert_eq!(preview.preview_id, "preview-1");
                assert_eq!(preview.title, "Roadmap.md");
                assert!(preview.body.contains("Roadmap.md") || preview.body.contains("Kind: File"));
            }
            other => panic!("expected PreviewReady, got {other:?}"),
        }

        server.await.expect("server task joins");
    }

    #[tokio::test]
    async fn daemon_rejects_oversized_ipc_payloads() {
        let pipe_name = format!(r"\\.\pipe\winspot-payload-test-{}", std::process::id());
        let server_name = pipe_name.clone();
        let server = tokio::spawn(async move {
            let engine = SearchEngine::from_results(Vec::new());
            serve_pipe_once(
                PipeConfig {
                    pipe_name: server_name,
                    usage_log_path: None,
                },
                &engine,
            )
            .await
            .expect("pipe server completes");
        });

        tokio::time::sleep(Duration::from_millis(25)).await;

        let client = ClientOptions::new()
            .open(&pipe_name)
            .expect("connect to payload test pipe");
        let mut client = BufReader::new(client);
        let oversized = format!("{}\n", "x".repeat(MAX_JSON_LINE_BYTES + 1));
        client
            .get_mut()
            .write_all(oversized.as_bytes())
            .await
            .expect("write oversized request");

        let mut line = String::new();
        timeout(Duration::from_secs(2), client.read_line(&mut line))
            .await
            .expect("payload response before timeout")
            .expect("read payload response");

        let response: IpcEnvelope =
            serde_json::from_str(line.trim()).expect("decode payload response");
        match response.payload {
            IpcPayload::Error(error) => assert_eq!(error.code, "payload_too_large"),
            other => panic!("expected Error, got {other:?}"),
        }

        server.await.expect("server task joins");
    }

    #[test]
    fn daemon_builds_search_engine_with_usage_log() {
        let path =
            std::env::temp_dir().join(format!("winspot-daemon-usage-{}.jsonl", std::process::id()));
        let _ = fs::remove_file(&path);
        let store = UsageStore::new(path.clone());
        store
            .record(UsageEvent {
                result_id: "command:terminal".to_string(),
                timestamp_unix_seconds: 2_000,
            })
            .expect("record terminal usage");

        let engine = build_search_engine(&PipeConfig {
            pipe_name: r"\\.\pipe\winspot-unused".to_string(),
            usage_log_path: Some(path.clone()),
        })
        .expect("build engine with usage");

        let results = engine.search("term", 5);

        assert_eq!(results[0].title, "Terminal");

        fs::remove_file(path).expect("cleanup usage log");
    }

    #[test]
    fn daemon_builds_search_engine_with_windows_settings() {
        let engine = build_search_engine(&PipeConfig {
            pipe_name: r"\\.\pipe\winspot-unused".to_string(),
            usage_log_path: None,
        })
        .expect("build engine with settings");

        let results = engine.search("windows update", 5);

        assert!(results.iter().any(|result| {
            result.title == "Windows Update" && result.kind == SearchResultKind::Setting
        }));
    }
}
