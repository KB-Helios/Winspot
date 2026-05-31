use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, params};
use winspot_core::{ActionKind, SearchResult, SearchResultKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedItem {
    pub id: String,
    pub title: String,
    pub path: String,
    pub kind: SearchResultKind,
    pub modified_unix_seconds: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexDiagnostics {
    pub item_count: u64,
    pub last_refresh_unix_seconds: u64,
}

#[derive(Debug)]
pub struct IndexStore {
    connection: Connection,
}

impl IndexStore {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }

        let connection = Connection::open(path)?;
        let store = Self { connection };
        store.initialize()?;
        Ok(store)
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let store = Self {
            connection: Connection::open_in_memory()?,
        };
        store.initialize()?;
        Ok(store)
    }

    pub fn upsert(&self, item: &IndexedItem) -> anyhow::Result<()> {
        self.connection.execute(
            "insert into indexed_items (id, title, path, kind, modified_unix_seconds)
             values (?1, ?2, ?3, ?4, ?5)
             on conflict(id) do update set
                title = excluded.title,
                path = excluded.path,
                kind = excluded.kind,
                modified_unix_seconds = excluded.modified_unix_seconds",
            params![
                item.id,
                item.title,
                item.path,
                kind_to_str(&item.kind),
                item.modified_unix_seconds
            ],
        )?;
        self.touch_refresh_timestamp()?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        self.connection
            .execute("delete from indexed_items where id = ?1", params![id])?;
        self.touch_refresh_timestamp()?;
        Ok(())
    }

    pub fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
        let pattern = format!("%{}%", query.trim().replace('%', "\\%"));
        let mut statement = self.connection.prepare(
            "select id, title, path, kind from indexed_items
             where title like ?1 escape '\\' or path like ?1 escape '\\'
             order by title collate nocase asc
             limit ?2",
        )?;
        let rows = statement.query_map(params![pattern, limit as u32], |row| {
            let id: String = row.get(0)?;
            let title: String = row.get(1)?;
            let path: String = row.get(2)?;
            let kind = str_to_kind(&row.get::<_, String>(3)?);
            Ok(SearchResult {
                id,
                title,
                subtitle: path,
                primary_action: ActionKind::Open,
                kind,
                score: 0.0,
                actions: Vec::new(),
                source: Some("index".to_string()),
                icon_hint: None,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn diagnostics(&self) -> anyhow::Result<IndexDiagnostics> {
        let item_count =
            self.connection
                .query_row("select count(*) from indexed_items", [], |row| {
                    row.get::<_, u64>(0)
                })?;
        let last_refresh_unix_seconds = self
            .connection
            .query_row(
                "select value from metadata where key = 'last_refresh'",
                [],
                |row| row.get::<_, u64>(0),
            )
            .unwrap_or_default();

        Ok(IndexDiagnostics {
            item_count,
            last_refresh_unix_seconds,
        })
    }

    fn initialize(&self) -> anyhow::Result<()> {
        self.connection.execute_batch(
            "create table if not exists indexed_items (
                id text primary key not null,
                title text not null,
                path text not null,
                kind text not null,
                modified_unix_seconds integer not null
             );
             create table if not exists metadata (
                key text primary key not null,
                value integer not null
             );",
        )?;
        Ok(())
    }

    fn touch_refresh_timestamp(&self) -> anyhow::Result<()> {
        self.connection.execute(
            "insert into metadata (key, value) values ('last_refresh', ?1)
             on conflict(key) do update set value = excluded.value",
            params![current_unix_seconds()],
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Indexer {
    roots: Vec<PathBuf>,
    max_depth: usize,
}

impl Indexer {
    pub fn new(roots: Vec<PathBuf>, max_depth: usize) -> Self {
        Self { roots, max_depth }
    }

    pub fn refresh(&self, store: &IndexStore) -> anyhow::Result<IndexDiagnostics> {
        for root in &self.roots {
            collect_root(root, 0, self.max_depth, store)?;
        }
        store.diagnostics()
    }
}

#[derive(Debug, Clone)]
pub struct DebouncedWatcher {
    debounce: Duration,
    last_event: Option<SystemTime>,
}

impl DebouncedWatcher {
    pub fn new(debounce: Duration) -> Self {
        Self {
            debounce,
            last_event: None,
        }
    }

    pub fn observe_change(&mut self, now: SystemTime) -> bool {
        let should_refresh = self
            .last_event
            .and_then(|last| now.duration_since(last).ok())
            .map(|elapsed| elapsed >= self.debounce)
            .unwrap_or(true);
        self.last_event = Some(now);
        should_refresh
    }
}

pub trait WindowsSearchAdapter {
    fn is_available(&self) -> bool;
    fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>>;
}

pub struct FallbackWindowsSearchAdapter<'a> {
    store: &'a IndexStore,
}

impl<'a> FallbackWindowsSearchAdapter<'a> {
    pub fn new(store: &'a IndexStore) -> Self {
        Self { store }
    }
}

impl WindowsSearchAdapter for FallbackWindowsSearchAdapter<'_> {
    fn is_available(&self) -> bool {
        false
    }

    fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<SearchResult>> {
        self.store.search(query, limit)
    }
}

fn collect_root(
    root: &Path,
    depth: usize,
    max_depth: usize,
    store: &IndexStore,
) -> anyhow::Result<()> {
    if depth > max_depth {
        return Ok(());
    }

    let Ok(entries) = fs::read_dir(root) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let Some(title) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };

        let kind = if file_type.is_dir() {
            SearchResultKind::Folder
        } else if file_type.is_file() {
            SearchResultKind::File
        } else {
            continue;
        };

        let id = format!("{}:{}", kind_to_id_prefix(&kind), path.display());
        store.upsert(&IndexedItem {
            id,
            title: title.to_string(),
            path: path.display().to_string(),
            kind,
            modified_unix_seconds: modified_unix_seconds(&path),
        })?;

        if file_type.is_dir() {
            collect_root(&path, depth + 1, max_depth, store)?;
        }
    }

    Ok(())
}

fn kind_to_str(kind: &SearchResultKind) -> &'static str {
    match kind {
        SearchResultKind::App => "app",
        SearchResultKind::File => "file",
        SearchResultKind::Folder => "folder",
        SearchResultKind::Command => "command",
        SearchResultKind::Setting => "setting",
        SearchResultKind::Plugin => "plugin",
        SearchResultKind::Process => "process",
        SearchResultKind::BrowserHistory => "browser-history",
    }
}

fn kind_to_id_prefix(kind: &SearchResultKind) -> &'static str {
    match kind {
        SearchResultKind::Folder => "folder",
        SearchResultKind::File => "file",
        _ => kind_to_str(kind),
    }
}

fn str_to_kind(value: &str) -> SearchResultKind {
    match value {
        "app" => SearchResultKind::App,
        "file" => SearchResultKind::File,
        "folder" => SearchResultKind::Folder,
        "setting" => SearchResultKind::Setting,
        "plugin" => SearchResultKind::Plugin,
        "process" => SearchResultKind::Process,
        "browser-history" => SearchResultKind::BrowserHistory,
        _ => SearchResultKind::Command,
    }
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn modified_unix_seconds(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
