use std::{
    env, fs,
    path::{Path, PathBuf},
};

use winspot_core::{ActionKind, SearchResult, SearchResultKind};

pub trait SearchProvider {
    fn collect_results(&self) -> Vec<SearchResult>;
}

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
