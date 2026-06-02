# Winspot Current Architecture

Winspot is an Avalonia 12 desktop launcher backed by a Rust daemon. The UI owns
windowing, focus, tray integration, hotkey registration, settings, keyboard
workflow, and rendering. The daemon owns search, indexing, ranking, previews,
actions, plugins, local persistence, and benchmark data.

## Runtime Flow

1. The Avalonia app opens as a compact, topmost spotlight window.
2. The configured global hotkey toggles visibility and focuses the search box.
3. The UI negotiates IPC protocol support with the Rust daemon over a local
   named pipe.
4. Query updates stream request-scoped result batches until `SearchCompleted`.
5. Selection changes request cancellable previews.
6. Enter executes the selected action through the daemon; successful actions
   hide the launcher.

## IPC

IPC uses newline-delimited JSON envelopes with camelCase fields. Protocol v2
adds `Hello`, `HelloAccepted`, `CancelRequest`, streaming `ResultBatch`,
`SearchCompleted`, `PreviewChunk`, `PreviewReady`, `ActionCompleted`, and
structured `Error` payloads. The daemon enforces a maximum JSON line size and
returns typed backend errors for unsupported or oversized messages.

## Search And Index

Search combines static providers, dynamic providers, local usage signals, and a
persistent SQLite index. Static providers cover built-in commands, Windows
settings, running processes, Start Menu apps, and bounded file-system roots.
Dynamic providers cover calculator, unit conversion, indexed search, browser
history adapters, and internal plugin results. The index crate owns SQLite
records, refresh diagnostics, debounced change observation, and a Windows Search
fallback adapter.

## Previews

The preview crate normalizes metadata, text/code, image, document, app, plugin,
and error previews. The daemon sends a quick `PreviewChunk` loading response,
then a final `PreviewReady` payload. Avalonia renders the payload as simple text
or metadata today, with richer controls isolated behind the same contract later.

## Actions And Plugins

Search results keep backward-compatible `primaryAction` data and can also carry
action descriptors. The action crate checks capabilities before clipboard,
shell, process, filesystem, or plugin work. The plugin crate supports phase-1
internal Rust plugin manifests and built-in calculator, terminal, clipboard, and
unit-conversion plugin identities. WASM sandboxing and a marketplace are outside
the current phase. Plugin loading emits a typed validation report, user manifests
are search-only by default, and plugin command execution is authorized against
the daemon's trusted registry before the action executor runs.

## Plugin Validation

The validation stage accepts warning-only manifests, rejects malformed or
policy-breaking manifests, and is available through `winspot-pluginctl validate`
for CI and local checks. The daemon keeps the same report in memory and exposes
it over IPC through `PluginDiagnosticsRequested` / `PluginDiagnosticsReady`.

## Packaging And Diagnostics

Portable builds are produced by `scripts/publish-portable.ps1` for `win-x64`
and `win-arm64`. A `Winspot.portable` marker beside the executable stores app
data under the portable folder; otherwise `%LOCALAPPDATA%\Winspot` is used.
Benchmarks run through `scripts/bench-search.ps1` and report cold-start-shaped,
IPC, preview, cached search, memory, and budget gate data.
