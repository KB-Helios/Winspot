use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use winspot_core::{SearchResult, SearchResultKind};

const TEXT_PREVIEW_MAX_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PreviewPayload {
    Metadata {
        title: String,
        body: String,
    },
    Text {
        title: String,
        language: Option<String>,
        body: String,
    },
    Image {
        title: String,
        path: String,
        body: String,
    },
    Document {
        title: String,
        body: String,
    },
    App {
        title: String,
        body: String,
    },
    Plugin {
        title: String,
        body: String,
    },
    Error {
        title: String,
        body: String,
    },
}

impl PreviewPayload {
    pub fn title(&self) -> &str {
        match self {
            PreviewPayload::Metadata { title, .. }
            | PreviewPayload::Text { title, .. }
            | PreviewPayload::Image { title, .. }
            | PreviewPayload::Document { title, .. }
            | PreviewPayload::App { title, .. }
            | PreviewPayload::Plugin { title, .. }
            | PreviewPayload::Error { title, .. } => title,
        }
    }

    pub fn body(&self) -> &str {
        match self {
            PreviewPayload::Metadata { body, .. }
            | PreviewPayload::Text { body, .. }
            | PreviewPayload::Image { body, .. }
            | PreviewPayload::Document { body, .. }
            | PreviewPayload::App { body, .. }
            | PreviewPayload::Plugin { body, .. }
            | PreviewPayload::Error { body, .. } => body,
        }
    }
}

pub trait PreviewProvider {
    fn preview(&self, result: &SearchResult) -> PreviewPayload;
}

#[derive(Debug, Default)]
pub struct DefaultPreviewProvider;

impl PreviewProvider for DefaultPreviewProvider {
    fn preview(&self, result: &SearchResult) -> PreviewPayload {
        match result.kind {
            SearchResultKind::File => preview_file(result),
            SearchResultKind::Folder => PreviewPayload::Metadata {
                title: result.title.clone(),
                body: format!("Folder\n{}", result.subtitle),
            },
            SearchResultKind::App => PreviewPayload::App {
                title: result.title.clone(),
                body: format!("Application\n{}", result.subtitle),
            },
            SearchResultKind::Plugin => PreviewPayload::Plugin {
                title: result.title.clone(),
                body: format!("Plugin result\n{}", result.subtitle),
            },
            _ => PreviewPayload::Metadata {
                title: result.title.clone(),
                body: metadata_body(result),
            },
        }
    }
}

fn preview_file(result: &SearchResult) -> PreviewPayload {
    let path = result_path(result);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();

    if matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp"
    ) {
        return PreviewPayload::Image {
            title: result.title.clone(),
            path: path.display().to_string(),
            body: format!("Image preview\n{}", path.display()),
        };
    }

    if matches!(
        extension.as_str(),
        "pdf" | "doc" | "docx" | "ppt" | "pptx" | "xls" | "xlsx"
    ) {
        return PreviewPayload::Document {
            title: result.title.clone(),
            body: format!("Document metadata\n{}", path.display()),
        };
    }

    if fs::metadata(&path)
        .map(|metadata| metadata.len() <= TEXT_PREVIEW_MAX_BYTES as u64)
        .unwrap_or(false)
    {
        if let Ok(bytes) = fs::read(&path) {
            if let Ok(text) = String::from_utf8(bytes) {
                return PreviewPayload::Text {
                    title: result.title.clone(),
                    language: extension_to_language(&extension),
                    body: text.lines().take(80).collect::<Vec<_>>().join("\n"),
                };
            }
        }
    }

    PreviewPayload::Metadata {
        title: result.title.clone(),
        body: metadata_body(result),
    }
}

fn result_path(result: &SearchResult) -> &Path {
    let raw = result
        .id
        .split_once(':')
        .map(|(_, value)| value)
        .unwrap_or(&result.subtitle);
    Path::new(raw)
}

fn extension_to_language(extension: &str) -> Option<String> {
    match extension {
        "rs" => Some("rust".to_string()),
        "cs" => Some("csharp".to_string()),
        "md" => Some("markdown".to_string()),
        "json" => Some("json".to_string()),
        "ps1" => Some("powershell".to_string()),
        _ => None,
    }
}

fn metadata_body(result: &SearchResult) -> String {
    format!(
        "Kind: {:?}\nPrimary action: {:?}\nLocation: {}\nScore: {:.1}",
        result.kind, result.primary_action, result.subtitle, result.score
    )
}
