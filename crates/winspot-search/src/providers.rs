use std::{
    env, fs,
    path::{Path, PathBuf},
};

use winspot_core::{ActionKind, SearchResult, SearchResultKind};

pub trait SearchProvider {
    fn collect_results(&self) -> Vec<SearchResult>;
}

const DEFAULT_FILE_SYSTEM_MAX_DEPTH: usize = 2;
const DEFAULT_FILE_SYSTEM_MAX_ENTRIES: usize = 500;

#[derive(Debug, Default)]
pub struct BuiltinCommandProvider;

impl BuiltinCommandProvider {
    pub fn collect_results(&self) -> Vec<SearchResult> {
        <Self as SearchProvider>::collect_results(self)
    }
}

impl SearchProvider for BuiltinCommandProvider {
    fn collect_results(&self) -> Vec<SearchResult> {
        vec![
            SearchResult {
                id: "command:calculator".to_string(),
                title: "Calculator".to_string(),
                subtitle: "Built-in command".to_string(),
                kind: SearchResultKind::Command,
                score: 0.0,
                primary_action: ActionKind::RunCommand,
            },
            SearchResult {
                id: "command:terminal".to_string(),
                title: "Terminal".to_string(),
                subtitle: "Open a shell command".to_string(),
                kind: SearchResultKind::Command,
                score: 0.0,
                primary_action: ActionKind::RunCommand,
            },
        ]
    }
}

#[derive(Debug, Clone)]
pub struct StartMenuAppProvider {
    roots: Vec<PathBuf>,
}

impl Default for StartMenuAppProvider {
    fn default() -> Self {
        Self::new(default_start_menu_roots())
    }
}

impl StartMenuAppProvider {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }

    pub fn collect_results(&self) -> Vec<SearchResult> {
        <Self as SearchProvider>::collect_results(self)
    }
}

impl SearchProvider for StartMenuAppProvider {
    fn collect_results(&self) -> Vec<SearchResult> {
        let mut results = Vec::new();
        for root in &self.roots {
            collect_shortcuts(root, &mut results);
        }
        results
    }
}

#[derive(Debug, Clone)]
pub struct FileSystemProvider {
    roots: Vec<PathBuf>,
    max_depth: usize,
    max_entries: usize,
}

impl Default for FileSystemProvider {
    fn default() -> Self {
        Self::new(
            default_file_system_roots(),
            DEFAULT_FILE_SYSTEM_MAX_DEPTH,
            DEFAULT_FILE_SYSTEM_MAX_ENTRIES,
        )
    }
}

impl FileSystemProvider {
    pub fn new(roots: Vec<PathBuf>, max_depth: usize, max_entries: usize) -> Self {
        Self {
            roots,
            max_depth,
            max_entries,
        }
    }

    pub fn collect_results(&self) -> Vec<SearchResult> {
        <Self as SearchProvider>::collect_results(self)
    }
}

impl SearchProvider for FileSystemProvider {
    fn collect_results(&self) -> Vec<SearchResult> {
        let mut results = Vec::new();
        for root in &self.roots {
            collect_file_system_entries(root, 0, self.max_depth, self.max_entries, &mut results);
            if results.len() >= self.max_entries {
                break;
            }
        }
        results
    }
}

fn default_start_menu_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(program_data) = env::var("ProgramData") {
        roots.push(PathBuf::from(program_data).join("Microsoft\\Windows\\Start Menu"));
    }
    if let Ok(app_data) = env::var("APPDATA") {
        roots.push(PathBuf::from(app_data).join("Microsoft\\Windows\\Start Menu"));
    }
    roots
}

fn default_file_system_roots() -> Vec<PathBuf> {
    let Ok(user_profile) = env::var("USERPROFILE") else {
        return Vec::new();
    };

    let user_profile = PathBuf::from(user_profile);
    ["Desktop", "Documents", "Downloads"]
        .into_iter()
        .map(|folder| user_profile.join(folder))
        .collect()
}

fn collect_shortcuts(root: &Path, results: &mut Vec<SearchResult>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_shortcuts(&path, results);
            continue;
        }

        if path.extension().and_then(|extension| extension.to_str()) != Some("lnk") {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };

        results.push(SearchResult {
            id: format!("app:{}", path.display()),
            title: stem.to_string(),
            subtitle: path.display().to_string(),
            kind: SearchResultKind::App,
            score: 0.0,
            primary_action: ActionKind::Open,
        });
    }
}

fn collect_file_system_entries(
    root: &Path,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    results: &mut Vec<SearchResult>,
) {
    if depth >= max_depth || results.len() >= max_entries {
        return;
    }

    let Ok(entries) = fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        if results.len() >= max_entries {
            return;
        }

        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        let Some(title) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            continue;
        };

        if file_type.is_dir() {
            results.push(SearchResult {
                id: format!("folder:{}", path.display()),
                title: title.to_string(),
                subtitle: path.display().to_string(),
                kind: SearchResultKind::Folder,
                score: 0.0,
                primary_action: ActionKind::Open,
            });
            collect_file_system_entries(&path, depth + 1, max_depth, max_entries, results);
            continue;
        }

        if file_type.is_file() {
            results.push(SearchResult {
                id: format!("file:{}", path.display()),
                title: title.to_string(),
                subtitle: path.display().to_string(),
                kind: SearchResultKind::File,
                score: 0.0,
                primary_action: ActionKind::Open,
            });
        }
    }
}
