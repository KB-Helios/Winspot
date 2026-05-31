# Winspot Launcher Design

Date: 2026-05-27

## Purpose

Winspot is a native Windows 11 global launcher designed to feel invisible until summoned. It opens from a configurable system-wide hotkey, accepts keyboard-first queries immediately, streams ranked results from a Rust backend, previews selected content asynchronously, and executes actions without blocking the UI thread.

The product targets a native, low-latency desktop experience: Avalonia 12 for presentation, Rust for indexing/search/plugins/actions, local-first storage, telemetry disabled by default, and no Electron or heavy web runtime.

## Scope

This design covers the full product architecture and the first buildable vertical slice. The full architecture includes search, indexing, ranking, previews, plugins, permissions, IPC, packaging, benchmarks, tray integration, developer tooling, and future AI-ready extension points.

The first implementation milestone proves the product spine:

- Launch a native Avalonia 12 app from the local development workflow.
- Register or simulate a configurable global launcher hotkey.
- Show a compact launcher window with focused search input.
- Connect to a Rust backend over async IPC.
- Stream ranked app/file/command results while typing.
- Execute the selected result through the backend action layer.
- Record local usage signals for future ranking.
- Run basic benchmark and verification commands.

## Non-Goals For The First Milestone

- Full plugin marketplace.
- WASM plugin sandbox.
- OCR indexing.
- Semantic or AI command routing.
- Rich PDF or Office rendering beyond metadata.
- Production installer polish.
- Cloud sync or network-backed telemetry.

These remain first-class future requirements, but the first milestone should build the architectural seams that let them arrive later without replacing the product core.

## Architecture

Winspot uses a layered monorepo with a C# Avalonia 12 UI process and a Rust backend process. The UI owns presentation, input, focus, windowing, accessibility, animation, and theme behavior. The backend owns search, indexing, ranking, plugins, previews, action execution, local persistence, and performance measurement.

Proposed repository shape:

```text
Winspot/
  apps/
    Winspot.App/          C# Avalonia 12 launcher UI
    Winspot.Tray/         optional later if tray is split
  crates/
    winspot-core/         shared Rust types, ranking inputs, actions, config
    winspot-ipc/          named pipe RPC protocol and streaming contracts
    winspot-index/        filesystem, app, settings, browser indexing
    winspot-search/       fuzzy matching, ranking, aliases, query cache
    winspot-plugins/      plugin manifests, host lifecycle, permissions
    winspot-preview/      preview provider pipeline
    winspot-daemon/       backend process composition
    winspot-bench/        startup, activation, query, memory benchmarks
  plugins/
    calculator/
    terminal/
    clipboard/
    vscode-workspace-search/
    github/
    spotify/
    system-monitor/
  docs/
    superpowers/specs/
```

Runtime flow:

```text
Global hotkey
  -> Avalonia UI process shows launcher on the active monitor
  -> Search input receives focus immediately
  -> UI streams query changes to Rust over IPC
  -> Rust fans out to search providers
  -> Backend streams partial ranked result batches
  -> UI virtualizes the result list and keeps selection stable
  -> Preview request starts for the selected result
  -> Enter executes a capability-checked action through the backend
  -> Launcher hides after successful action unless the action requests otherwise
```

The default development model is unpackaged Avalonia 12 for repeatable command-line build and launch verification. The product architecture still reserves packaged MSIX and portable modes.

## Avalonia App Design

The launcher is a transient command surface, not a normal document window. It stays hidden until summoned, appears centered on the active monitor, focuses the search box immediately, and dismisses with Escape or light-dismiss behavior.

The Avalonia app owns:

- Hotkey configuration UI and launch activation handoff.
- Launcher window placement, sizing, focus, and monitor awareness.
- Mica base surface for long-lived windows.
- Acrylic for transient flyouts and command overlays.
- Search input and keyboard routing.
- Virtualized result list.
- Inline preview panel.
- Command/action palette UI.
- Settings, plugin management, benchmarks, and diagnostics pages.
- Accessibility, localization readiness, theme support, high contrast, DPI, and multi-monitor behavior.

The summoned launcher should use a compact custom window rather than a full `NavigationView`. Settings and developer tools can use a standard Avalonia navigation shell later. The main launcher layout is:

- Search input at the top.
- Result list as the primary content.
- Preview/actions panel on the right when width allows.
- Single-column result-first layout at narrow widths.

Keyboard workflow:

- Hotkey shows launcher and focuses search input.
- Escape hides launcher or closes transient UI.
- Up and Down change selected result.
- Enter runs the primary action.
- Tab or Right moves into actions/preview affordances when present.
- Modifier shortcuts expose alternate actions.

The UI must keep disk, network, plugin, indexing, and preview work off the UI thread. XAML should use theme resources, platform typography, standard controls where practical, a shallow visual tree, and accessible names for meaningful controls.

## Backend Design

The Rust backend is the always-ready engine. During early development it can be launched by the UI or dev scripts. Later it may run as an optional background daemon or service so indexing and caches remain warm while the UI is hidden.

Core crates:

```text
winspot-core
  SearchQuery, SearchResult, Action, PreviewRequest, PreviewPayload,
  PluginManifest, Capability, UsageEvent, Config

winspot-ipc
  Async named pipe protocol, request IDs, cancellation, streaming responses

winspot-index
  App, file, folder, settings, browser-history, and command indexing
  Windows Search adapter plus custom fallback indexer

winspot-search
  Fuzzy matching, scoring, ranking, aliases, recency, frequency, query cache

winspot-plugins
  Plugin manifests, hot reload, capability checks, host lifecycle

winspot-preview
  Async preview provider registry and preview payload normalization

winspot-daemon
  IPC server, plugin host, index scheduler, action runner, persistence
```

Backend operations must be async and cancellation-aware. Provider failure, timeout, plugin crash, or stale query cancellation must not block the UI or prevent faster providers from returning results.

## Search And Indexing

Search combines indexed local data, live providers, plugins, and built-in commands. Providers stream partial results so the first useful result can appear before slower sources finish.

Query flow:

```text
UI text changes
  -> adaptive debounce
  -> IPC SearchStarted(query_id, text)
  -> backend fans out to providers
  -> fast cached providers stream first result batches
  -> slower index/plugin providers append batches
  -> stale query IDs are cancelled or ignored
  -> UI preserves selection when possible
```

The search engine supports:

- Apps, files, folders, commands, browser history, settings, processes, plugins, and future semantic sources.
- Windows Search when available.
- Custom fallback indexing when Windows Search is unavailable or insufficient.
- Filesystem watchers for incremental updates.
- Smart caching for warm queries and common result sets.
- Adaptive debounce and query result caching.
- Streaming result batches.

Ranking combines:

- Exact match.
- Prefix match.
- Fuzzy score.
- Result type priority.
- Alias match.
- Usage frequency.
- Recency.
- Plugin-provided confidence.
- Learned local behavior.

Learned behavior is local-only and can be disabled or reset. External telemetry is off by default.

## Plugin Architecture

Plugins extend commands, search providers, preview providers, actions, and later custom UI components. The SDK is Rust-first, with optional WASM sandbox support in a later phase.

Plugin phases:

```text
Phase 1:
  In-process Rust plugins for SDK shape and example plugins.

Phase 2:
  Out-of-process plugin host for crash isolation.

Phase 3:
  WASM sandbox support for restricted third-party plugins.
```

Every plugin declares a manifest with capabilities. Capability categories include:

- Filesystem read scopes.
- Filesystem write scopes.
- Process execution.
- Clipboard access.
- Network access.
- Shell execution.
- Window/process inspection.
- Preview rendering.
- Custom UI contributions.

The backend enforces capabilities before a plugin can search, preview, or execute actions. Developer mode supports hot reload by watching plugin directories and restarting plugin instances without restarting the launcher.

Example plugins:

- Calculator and unit conversion.
- Terminal command runner.
- Clipboard history.
- VSCode workspace search.
- GitHub integration.
- Spotify controls.
- System monitor.

## Preview System

Previews are asynchronous, cancellable, and disposable. The UI requests a preview for the selected result and renders a lightweight loading state immediately. If selection changes, stale preview responses are ignored by preview ID.

Preview payloads are normalized so the UI does not execute arbitrary plugin UI code in the first version.

Preview types:

```text
TextPreview       markdown, code snippets, metadata, terminal output
ImagePreview      decoded thumbnail plus file metadata
DocumentPreview   PDF/Office metadata first, richer rendering later
AppPreview        app identity, install path, usage, available actions
PluginPreview     constrained plugin-defined payload
```

Preview rendering must be lazy-loaded and virtualization-friendly. Heavy document rendering, image decoding, or plugin preview work runs outside the UI thread.

## Actions

Actions are backend commands guarded by capability checks. The UI displays actions and sends action requests, but it does not perform shell execution directly.

Common actions:

- Open.
- Open containing folder.
- Run command.
- Copy path or text.
- Pin or favorite.
- Kill process.
- Switch window.
- Run plugin command.
- Open in terminal.

Actions return a structured result so the UI can decide whether to hide the launcher, show inline feedback, or display an error.

## IPC

The initial IPC transport should be Windows named pipes with async request/response and streaming events. The protocol must include request IDs, cancellation, version negotiation, structured errors, and payload size limits.

Named pipes are the first choice because they are native, lightweight, local-only, and suitable for UI-to-backend communication. The design should keep the contract transport-neutral enough to support high-performance RPC later if benchmarks prove it necessary.

## Storage And Privacy

Winspot is local-first and works offline. Local storage includes:

- User settings.
- Hotkey configuration.
- Index cache.
- Usage frequency and recency signals.
- Plugin manifests and trust state.
- Clipboard history if enabled.
- Benchmark and diagnostic history.

Telemetry is disabled by default. Diagnostics pages may show local performance data, logs, and privacy controls without sending data externally.

Portable mode stores state beside the executable or in a configured portable data directory. Packaged mode uses appropriate application data locations.

## Packaging And Runtime Modes

Early development:

- Unpackaged Avalonia 12 app.
- Rust daemon launched by dev script or UI.
- Direct command-line build, launch, and verification.

Product delivery:

- MSIX packaged install path.
- Portable mode for direct folder execution.
- Optional background indexing service.
- Tray icon for settings, quit, startup launch, and indexing status.
- Startup launch option.
- Native x64 and ARM64 builds.

The packaging choice must remain explicit because some Windows APIs behave differently with and without package identity.

## Performance Requirements

Performance targets are product requirements and must be measured:

```text
Cold startup target:       under 150ms, measured separately for UI and backend
Warm activation target:    under 30ms from hotkey to focused search box
Idle RAM target:           under 60MB combined steady-state target
First result target:       under 50ms for warm cached app/command queries
UI thread blocking:        no known synchronous disk/network/plugin work
Index updates:             incremental, debounced, non-blocking
```

The benchmark crate should ship early. It measures startup, activation, IPC round-trip latency, query latency, indexing throughput, plugin load time, preview latency, and memory usage. Avalonia settings should later expose a diagnostics page using the same measurements.

## Error Handling

Failure handling must preserve responsiveness:

- Backend unavailable: UI shows a local error state and can attempt restart.
- Provider timeout: partial results continue from other providers.
- Plugin crash: plugin is disabled or restarted without crashing launcher.
- Index corruption: rebuild affected index segment.
- Preview failure: show fallback metadata and keep result list usable.
- Action failure: show structured error and keep launcher open.
- IPC version mismatch: fail with a clear compatibility error.

## Accessibility And Localization

The launcher is keyboard-first but must remain mouse and touch usable. Required accessibility behavior:

- Search input has an accessible name and receives focus on activation.
- Result rows expose title, subtitle, type, and primary action.
- Preview panel has a meaningful landmark/name.
- Icon-only commands have accessible labels.
- Focus order is logical and visible.
- High contrast is supported.
- Text uses localizable resources and layouts tolerate string growth.

## Testing And Verification

Rust verification:

- Unit tests for scoring, indexing, IPC contracts, capability checks, and config.
- Integration tests for provider fanout, cancellation, plugin load/unload, and action execution.
- Benchmarks for query latency, memory, startup, IPC, indexing, and preview latency.

Avalonia verification:

- Build and launch verification after UI changes.
- Objective top-level window evidence after launch.
- Hotkey activation check.
- Keyboard workflow check.
- Light, dark, and high-contrast checks.
- DPI and multi-monitor placement checks.
- UI thread responsiveness checks for query and preview workflows.

End-to-end verification:

```text
hotkey
  -> focused launcher
  -> query typed
  -> streamed results visible
  -> preview loads without blocking selection
  -> selected action executes
  -> launcher hides or reports action status
```

## Implementation Milestones

1. Repository and toolchain foundation.
   - Initialize monorepo.
   - Verify Avalonia toolchain.
   - Scaffold unpackaged Avalonia app.
   - Scaffold Rust workspace and core crates.

2. UI/backend spine.
   - Launch Avalonia app.
   - Start/connect Rust backend.
   - Define IPC messages.
   - Send query and receive mocked streaming results.

3. Launcher interaction.
   - Compact launcher window.
   - Focused search input.
   - Keyboard navigation.
   - Hide/show behavior.
   - First styling pass using Mica/theme resources.

4. Real search.
   - App and command provider.
   - File/folder provider.
   - Fuzzy scoring and ranking.
   - Query cancellation and result streaming.

5. Actions and usage learning.
   - Open/run/copy actions.
   - Backend action execution.
   - Local usage event storage.
   - Ranking updates from usage.

6. Previews.
   - Metadata preview.
   - Text/code preview.
   - Image thumbnail preview.
   - Preview cancellation.

7. Indexing.
   - Filesystem watcher.
   - Windows Search adapter.
   - Fallback indexer.
   - Smart cache.

8. Plugins.
   - Manifest format.
   - Calculator plugin.
   - Terminal plugin.
   - Clipboard plugin.
   - Capability enforcement.
   - Developer hot reload.

9. Product shell.
   - Settings pages.
   - Plugin management.
   - Tray icon.
   - Startup launch options.
   - Diagnostics and benchmark page.

10. Packaging and hardening.
    - MSIX path.
    - Portable mode.
    - x64 and ARM64 builds.
    - Crash isolation.
    - Performance budget gates.

## Implementation Defaults

These defaults remove ambiguity for the implementation plan while preserving room for benchmark-driven changes:

- The Avalonia app process owns the global hotkey, tray icon, and launcher window in early builds. The Rust backend owns indexing, search, actions, previews, plugins, and benchmark collection. A separate tray process is added only if measurements show the UI host cannot meet idle memory or activation targets.
- IPC starts with versioned JSON messages over async named pipes for debuggability. The protocol remains transport-neutral, and hot paths can move to a compact binary encoding only if IPC benchmarks threaten the latency budget.
- Local persistence starts with SQLite for usage events, plugin trust state, index metadata, and diagnostics history. Portable mode stores the database under the portable data directory. Simple user-editable settings can use structured JSON or TOML files.
- Phase 1 plugins are internal Rust plugins used to prove the SDK. Out-of-process plugin hosting must land before third-party plugin support is considered stable. WASM support follows after capability enforcement and crash isolation are working.
- PDF support starts with file metadata and shell-provided thumbnails when available. A custom PDF renderer is deferred until dependency cost, memory impact, and preview latency are measured against the product budgets.
