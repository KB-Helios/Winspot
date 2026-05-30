use winspot_search::usage::{UsageEvent, UsageStore};

#[test]
fn usage_store_persists_events_and_loads_snapshot() {
    let path =
        std::env::temp_dir().join(format!("winspot-usage-test-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);

    let store = UsageStore::new(path.clone());
    store
        .record(UsageEvent {
            result_id: "app:notepad".to_string(),
            timestamp_unix_seconds: 10,
        })
        .expect("record first usage event");
    store
        .record(UsageEvent {
            result_id: "app:notepad".to_string(),
            timestamp_unix_seconds: 20,
        })
        .expect("record second usage event");
    store
        .record(UsageEvent {
            result_id: "command:terminal".to_string(),
            timestamp_unix_seconds: 15,
        })
        .expect("record terminal usage event");

    let snapshot = UsageStore::new(path.clone())
        .load_snapshot()
        .expect("load usage snapshot");

    let notepad = snapshot.get("app:notepad").expect("notepad signal exists");
    assert_eq!(notepad.launch_count, 2);
    assert_eq!(notepad.last_used_unix_seconds, 20);

    let terminal = snapshot
        .get("command:terminal")
        .expect("terminal signal exists");
    assert_eq!(terminal.launch_count, 1);
    assert_eq!(terminal.last_used_unix_seconds, 15);

    std::fs::remove_file(path).expect("cleanup usage log");
}
