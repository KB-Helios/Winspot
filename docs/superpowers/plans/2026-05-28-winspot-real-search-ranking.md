# Winspot Real Search Ranking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace mocked daemon results with a tested local search engine that ranks built-in commands and discovered Start Menu applications.

**Architecture:** Add a focused Rust `winspot-search` crate for fuzzy scoring, result ranking, and local providers. Keep IPC unchanged: the daemon receives `SearchStarted` and returns one final `ResultBatch`, but the batch comes from `SearchEngine` instead of hard-coded mock results.

**Tech Stack:** Rust 1.95, serde models from `winspot-core`, Windows filesystem Start Menu scanning, pure-Rust fuzzy scoring, Cargo unit tests.

---

## Scope Check

This plan implements the next search slice only. It does not add filesystem watchers, Windows Search integration, SQLite usage learning, previews, plugins, clipboard history, window switching, tray integration, or hotkey registration. Those remain follow-on slices from the design spec.

## File Structure

- Modify `Cargo.toml`: add `crates/winspot-search` to the workspace and add `winspot-search` as a workspace dependency.
- Create `crates/winspot-search/Cargo.toml`: search crate manifest.
- Create `crates/winspot-search/src/lib.rs`: exports search engine, ranking, and providers.
- Create `crates/winspot-search/src/ranking.rs`: fuzzy scoring and ranked sorting.
- Create `crates/winspot-search/src/providers.rs`: built-in command provider and Start Menu app discovery provider.
- Create `crates/winspot-search/src/engine.rs`: provider fanout and result limiting.
- Create `crates/winspot-search/tests/ranking.rs`: behavior tests for fuzzy ranking.
- Create `crates/winspot-search/tests/providers.rs`: behavior tests for command/app providers.
- Modify `crates/winspot-daemon/Cargo.toml`: depend on `winspot-search`.
- Modify `crates/winspot-daemon/src/server.rs`: use `SearchEngine` instead of mocked results.
- Modify `crates/winspot-daemon/tests/pipe_smoke.rs`: verify daemon returns ranked local search results.

## Task 1: Ranking Crate Skeleton

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/winspot-search/Cargo.toml`
- Create: `crates/winspot-search/src/lib.rs`
- Create: `crates/winspot-search/src/ranking.rs`
- Create: `crates/winspot-search/tests/ranking.rs`

- [ ] **Step 1: Write failing ranking tests**

Create `crates/winspot-search/tests/ranking.rs`:

```rust
use winspot_core::{ActionKind, SearchResult, SearchResultKind};
use winspot_search::ranking::{rank_results, score_match};

#[test]
fn score_match_rewards_exact_and_prefix_matches() {
    let exact = score_match("notepad", "notepad");
    let prefix = score_match("note", "notepad");
    let fuzzy = score_match("npd", "notepad");
    let miss = score_match("zz", "notepad");

    assert!(exact > prefix);
    assert!(prefix > fuzzy);
    assert!(fuzzy > miss);
    assert_eq!(miss, 0.0);
}

#[test]
fn rank_results_orders_by_score_then_title() {
    let results = vec![
        SearchResult {
            id: "command:calculator".to_string(),
            title: "Calculator".to_string(),
            subtitle: "Command".to_string(),
            kind: SearchResultKind::Command,
            score: 0.0,
            primary_action: ActionKind::RunCommand,
        },
        SearchResult {
            id: "app:notepad".to_string(),
            title: "Notepad".to_string(),
            subtitle: "App".to_string(),
            kind: SearchResultKind::App,
            score: 0.0,
            primary_action: ActionKind::Open,
        },
    ];

    let ranked = rank_results("note", results, 10);

    assert_eq!(ranked[0].title, "Notepad");
    assert!(ranked[0].score > ranked[1].score);
}
```

- [ ] **Step 2: Run the failing test**

Run:

```powershell
rtk cargo test -p winspot-search --test ranking
```

Expected:

```text
error: package ID specification `winspot-search` did not match any packages
```

- [ ] **Step 3: Add the workspace member and manifest**

Modify root `Cargo.toml`:

```toml
[workspace]
members = [
    "crates/winspot-core",
    "crates/winspot-daemon",
    "crates/winspot-search"
]
resolver = "3"

[workspace.package]
edition = "2024"
license = "MIT"

[workspace.dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["io-util", "macros", "net", "rt-multi-thread", "sync", "time"] }
uuid = { version = "1", features = ["serde", "v4"] }
winspot-core = { path = "crates/winspot-core" }
winspot-search = { path = "crates/winspot-search" }
```

Create `crates/winspot-search/Cargo.toml`:

```toml
[package]
name = "winspot-search"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
winspot-core.workspace = true
```

- [ ] **Step 4: Add ranking exports**

Create `crates/winspot-search/src/lib.rs`:

```rust
pub mod engine;
pub mod providers;
pub mod ranking;
```

- [ ] **Step 5: Implement ranking**

Create `crates/winspot-search/src/ranking.rs`:

```rust
use std::cmp::Ordering;

use winspot_core::SearchResult;

pub fn score_match(query: &str, candidate: &str) -> f32 {
    let query = query.trim().to_lowercase();
    let candidate = candidate.trim().to_lowercase();

    if query.is_empty() || candidate.is_empty() {
        return 0.0;
    }

    if query == candidate {
        return 1000.0;
    }

    if candidate.starts_with(&query) {
        return 850.0 - (candidate.len().saturating_sub(query.len()) as f32);
    }

    if candidate.contains(&query) {
        return 650.0 - candidate.find(&query).unwrap_or(0) as f32;
    }

    let mut score = 0.0;
    let mut search_from = 0;
    for character in query.chars() {
        let remaining = &candidate[search_from..];
        if let Some(offset) = remaining.find(character) {
            score += 100.0 / (1.0 + offset as f32);
            search_from += offset + character.len_utf8();
        } else {
            return 0.0;
        }
    }

    score
}

pub fn rank_results(query: &str, results: Vec<SearchResult>, limit: usize) -> Vec<SearchResult> {
    let mut scored = results
        .into_iter()
        .map(|mut result| {
            result.score = score_match(query, &result.title);
            result
        })
        .filter(|result| result.score > 0.0)
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.title.cmp(&right.title))
    });
    scored.truncate(limit);
    scored
}
```

- [ ] **Step 6: Run ranking tests**

Run:

```powershell
rtk cargo test -p winspot-search --test ranking
```

Expected:

```text
test result: ok. 2 passed
```

- [ ] **Step 7: Commit ranking crate skeleton**

Run:

```powershell
rtk git add Cargo.toml crates/winspot-search
rtk git commit -m "feat: add search ranking crate"
```

Expected:

```text
Commit output includes "feat: add search ranking crate".
```

## Task 2: Local Providers

**Files:**
- Modify: `crates/winspot-search/Cargo.toml`
- Modify: `crates/winspot-search/src/providers.rs`
- Create: `crates/winspot-search/tests/providers.rs`

- [ ] **Step 1: Write provider tests**

Create `crates/winspot-search/tests/providers.rs`:

```rust
use std::fs;

use winspot_search::providers::{BuiltinCommandProvider, StartMenuAppProvider};

#[test]
fn builtin_command_provider_exposes_calculator_and_terminal() {
    let results = BuiltinCommandProvider::default().collect_results();

    assert!(results.iter().any(|result| result.title == "Calculator"));
    assert!(results.iter().any(|result| result.title == "Terminal"));
}

#[test]
fn start_menu_provider_discovers_shortcuts_from_configured_roots() {
    let root = std::env::temp_dir().join(format!(
        "winspot-provider-test-{}",
        std::process::id()
    ));
    let programs = root.join("Programs");
    fs::create_dir_all(&programs).expect("create test programs directory");
    fs::write(programs.join("Sample App.lnk"), b"shortcut").expect("write shortcut");

    let provider = StartMenuAppProvider::new(vec![root.clone()]);
    let results = provider.collect_results();

    assert!(results.iter().any(|result| result.title == "Sample App"));

    fs::remove_dir_all(root).expect("cleanup test directory");
}
```

- [ ] **Step 2: Run failing provider tests**

Run:

```powershell
rtk cargo test -p winspot-search --test providers
```

Expected:

```text
error[E0432]: unresolved import `winspot_search::providers`
```

- [ ] **Step 3: Implement providers**

Create `crates/winspot-search/src/providers.rs`:

```rust
use std::{
    env,
    fs,
    path::{Path, PathBuf},
};

use winspot_core::{ActionKind, SearchResult, SearchResultKind};

#[derive(Debug, Default)]
pub struct BuiltinCommandProvider;

impl BuiltinCommandProvider {
    pub fn collect_results(&self) -> Vec<SearchResult> {
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
```

- [ ] **Step 4: Run provider tests**

Run:

```powershell
rtk cargo test -p winspot-search --test providers
```

Expected:

```text
test result: ok. 2 passed
```

- [ ] **Step 5: Commit providers**

Run:

```powershell
rtk git add crates/winspot-search
rtk git commit -m "feat: add local search providers"
```

Expected:

```text
Commit output includes "feat: add local search providers".
```

## Task 3: Search Engine And Daemon Wiring

**Files:**
- Create: `crates/winspot-search/src/engine.rs`
- Modify: `crates/winspot-daemon/Cargo.toml`
- Modify: `crates/winspot-daemon/src/server.rs`
- Modify: `crates/winspot-daemon/tests/pipe_smoke.rs`

- [ ] **Step 1: Add engine implementation**

Create `crates/winspot-search/src/engine.rs`:

```rust
use winspot_core::SearchResult;

use crate::{
    providers::{BuiltinCommandProvider, StartMenuAppProvider},
    ranking::rank_results,
};

#[derive(Debug, Default)]
pub struct SearchEngine {
    command_provider: BuiltinCommandProvider,
    app_provider: StartMenuAppProvider,
}

impl SearchEngine {
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let mut results = self.command_provider.collect_results();
        results.extend(self.app_provider.collect_results());
        rank_results(query, results, limit)
    }
}
```

- [ ] **Step 2: Depend on `winspot-search` from daemon**

Modify `crates/winspot-daemon/Cargo.toml`:

```toml
[package]
name = "winspot-daemon"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
anyhow.workspace = true
serde_json.workspace = true
tokio.workspace = true
winspot-core.workspace = true
winspot-search.workspace = true
```

- [ ] **Step 3: Wire daemon to the engine**

Modify `crates/winspot-daemon/src/server.rs` so `handle_line` uses `SearchEngine::default().search(&search.text, 20)` for `ResultBatch.results`.

- [ ] **Step 4: Update pipe smoke expectation**

Modify `crates/winspot-daemon/tests/pipe_smoke.rs` so the request text is `"calc"` and the assertion expects `"Calculator"`.

- [ ] **Step 5: Run daemon and workspace tests**

Run:

```powershell
rtk cargo test -p winspot-daemon
rtk cargo test
```

Expected:

```text
daemon tests pass
workspace tests pass
```

- [ ] **Step 6: Commit daemon wiring**

Run:

```powershell
rtk git add Cargo.toml Cargo.lock crates/winspot-search crates/winspot-daemon
rtk git commit -m "feat: wire daemon to ranked local search"
```

Expected:

```text
Commit output includes "feat: wire daemon to ranked local search".
```

## Task 4: Verification

**Files:**
- Read: `scripts/verify-spine.ps1`

- [ ] **Step 1: Stop old Winspot processes before rebuilding**

Run:

```powershell
rtk proxy powershell -NoProfile -Command "Get-Process Winspot.App,winspot-daemon -ErrorAction SilentlyContinue | Stop-Process -Force"
```

Expected:

```text
Command exits successfully even when no matching process is running.
```

- [ ] **Step 2: Run verification script**

Run:

```powershell
rtk powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\verify-spine.ps1
```

Expected:

```text
Rust tests pass.
Avalonia build succeeds with 0 errors.
```

- [ ] **Step 3: Launch daemon and app**

Run daemon:

```powershell
rtk cargo run -p winspot-daemon
```

Run app:

```powershell
rtk dotnet run --project apps/Winspot.App/Winspot.App.csproj -c Debug -p:Platform=x64
```

Expected:

```text
Winspot.App.exe has a responsive top-level window.
winspot-daemon.exe is running.
Typing "calc" returns Calculator.
Typing "terminal" returns Terminal.
Typing a Start Menu app name returns that app when a matching .lnk exists.
```

## Spec Coverage Notes

This plan advances these design requirements:

- Instant fuzzy search across apps and commands.
- Extensible search scoring and ranking engine.
- Streaming-shaped result contract remains intact.
- Local-first search source without web/runtime dependency.
- Rust backend owns search and ranking.

Still left for later:

- Files, folders, browser history, settings, processes, and plugins.
- Windows Search adapter and fallback persistent indexer.
- Usage learning and SQLite persistence.
- Preview providers and action execution.
- Hotkey, tray, startup, packaging hardening, and benchmarks.
