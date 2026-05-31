use std::fs;

use winspot_core::{ActionKind, SearchResult, SearchResultKind};
use winspot_preview::{DefaultPreviewProvider, PreviewPayload, PreviewProvider};

#[test]
fn text_file_preview_reads_small_utf8_files() {
    let path = std::env::temp_dir().join(format!("winspot-preview-{}.md", std::process::id()));
    fs::write(&path, "# Roadmap\nShip previews").expect("write preview file");
    let result = SearchResult {
        id: format!("file:{}", path.display()),
        title: "Roadmap.md".to_string(),
        subtitle: path.display().to_string(),
        kind: SearchResultKind::File,
        score: 1.0,
        primary_action: ActionKind::Open,
        ..SearchResult::default()
    };

    let payload = DefaultPreviewProvider.preview(&result);

    match payload {
        PreviewPayload::Text {
            title,
            body,
            language,
        } => {
            assert_eq!(title, "Roadmap.md");
            assert_eq!(language.as_deref(), Some("markdown"));
            assert!(body.contains("Ship previews"));
        }
        other => panic!("expected text preview, got {other:?}"),
    }

    fs::remove_file(path).expect("cleanup");
}

#[test]
fn image_preview_returns_path_payload_without_decoding_in_backend() {
    let result = SearchResult {
        id: "file:C:\\Temp\\photo.png".to_string(),
        title: "photo.png".to_string(),
        subtitle: "C:\\Temp\\photo.png".to_string(),
        kind: SearchResultKind::File,
        score: 1.0,
        primary_action: ActionKind::Open,
        ..SearchResult::default()
    };

    let payload = DefaultPreviewProvider.preview(&result);

    assert!(matches!(payload, PreviewPayload::Image { .. }));
}
