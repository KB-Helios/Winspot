#[cfg(windows)]
mod windows_tests {
    use std::time::Duration;

    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::windows::named_pipe::ClientOptions,
        time::timeout,
    };
    use winspot_core::{IpcEnvelope, IpcPayload, SearchStarted};
    use winspot_daemon::server::{serve_pipe_once, PipeConfig};

    #[tokio::test]
    async fn daemon_streams_mock_results_for_search() {
        let pipe_name = format!(r"\\.\pipe\winspot-test-{}", std::process::id());
        let server_name = pipe_name.clone();
        let server = tokio::spawn(async move {
            serve_pipe_once(PipeConfig {
                pipe_name: server_name,
            })
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
                text: "calc".to_string(),
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
                assert!(batch.results.iter().any(|result| result.title == "Calculator"));
            }
            other => panic!("expected ResultBatch, got {other:?}"),
        }

        server.await.expect("server task joins");
    }
}
