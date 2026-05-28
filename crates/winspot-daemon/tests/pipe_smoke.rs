#[cfg(windows)]
mod windows_tests {
    use std::{fs, time::Duration};

    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::windows::named_pipe::ClientOptions,
        time::timeout,
    };
    use winspot_core::{
        ActionKind, IpcEnvelope, IpcPayload, SearchResult, SearchResultKind, SearchStarted,
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
}
