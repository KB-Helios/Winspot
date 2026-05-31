use std::{
    fs,
    time::{Duration, SystemTime},
};

use winspot_core::SearchResultKind;
use winspot_index::{
    DebouncedWatcher, FallbackWindowsSearchAdapter, IndexStore, IndexedItem, Indexer,
    WindowsSearchAdapter,
};

#[test]
fn index_store_upserts_searches_and_deletes_items() {
    let store = IndexStore::open_in_memory().expect("open in-memory index");
    store
        .upsert(&IndexedItem {
            id: "file:C:\\Temp\\Roadmap.md".to_string(),
            title: "Roadmap.md".to_string(),
            path: "C:\\Temp\\Roadmap.md".to_string(),
            kind: SearchResultKind::File,
            modified_unix_seconds: 1,
        })
        .expect("upsert item");

    let results = store.search("road", 10).expect("search index");
    assert_eq!(results[0].title, "Roadmap.md");
    assert_eq!(results[0].kind, SearchResultKind::File);

    store
        .delete("file:C:\\Temp\\Roadmap.md")
        .expect("delete item");
    assert!(
        store
            .search("road", 10)
            .expect("search after delete")
            .is_empty()
    );
}

#[test]
fn direct_upsert_does_not_mark_index_refresh_complete() {
    let store = IndexStore::open_in_memory().expect("open in-memory index");
    store
        .upsert(&IndexedItem {
            id: "file:C:\\Temp\\Roadmap.md".to_string(),
            title: "Roadmap.md".to_string(),
            path: "C:\\Temp\\Roadmap.md".to_string(),
            kind: SearchResultKind::File,
            modified_unix_seconds: 1,
        })
        .expect("upsert item");

    assert_eq!(
        store
            .diagnostics()
            .expect("read diagnostics")
            .last_refresh_unix_seconds,
        0
    );
}

#[test]
fn indexer_refreshes_file_and_folder_records() {
    let root = std::env::temp_dir().join(format!("winspot-index-{}", std::process::id()));
    let nested = root.join("Nested");
    fs::create_dir_all(&nested).expect("create nested folder");
    fs::write(nested.join("Note.txt"), "hello").expect("write test file");

    let store = IndexStore::open_in_memory().expect("open index");
    let diagnostics = Indexer::new(vec![root.clone()], 4)
        .refresh(&store)
        .expect("refresh index");

    assert!(diagnostics.item_count >= 2);
    assert!(diagnostics.last_refresh_unix_seconds > 0);
    assert!(
        store
            .search("note", 10)
            .expect("search index")
            .iter()
            .any(|result| result.title == "Note.txt")
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn watcher_debounces_rapid_changes() {
    let start = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let mut watcher = DebouncedWatcher::new(Duration::from_secs(5));

    assert!(watcher.observe_change(start));
    assert!(!watcher.observe_change(start + Duration::from_secs(1)));
    assert!(watcher.observe_change(start + Duration::from_secs(6)));
}

#[test]
fn windows_search_adapter_falls_back_to_local_index() {
    let store = IndexStore::open_in_memory().expect("open index");
    store
        .upsert(&IndexedItem {
            id: "folder:C:\\Users\\kevin\\Documents".to_string(),
            title: "Documents".to_string(),
            path: "C:\\Users\\kevin\\Documents".to_string(),
            kind: SearchResultKind::Folder,
            modified_unix_seconds: 1,
        })
        .expect("upsert folder");

    let adapter = FallbackWindowsSearchAdapter::new(&store);
    assert!(!adapter.is_available());
    assert_eq!(
        adapter.search("doc", 5).expect("fallback search")[0].title,
        "Documents"
    );
}
