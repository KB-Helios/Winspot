# Winspot

A fast, keyboard-first desktop launcher for Windows. Winspot pairs an Avalonia
(.NET) UI with a Rust backend: the UI owns the window, focus, global hotkey, tray
icon, and settings, while the Rust daemon owns search, previews, and action
execution. They communicate over a local named pipe using a newline-delimited JSON
protocol.

## Architecture

```
            named pipe (JSON, v2)
  ┌────────────────┐   <────────>   ┌────────────────────────────┐
  │  Avalonia UI   │                │        Rust daemon         │
  │ apps/Winspot.* │                │   crates/winspot-daemon    │
  │  - window      │                │   - search engine          │
  │  - hotkey      │                │   - providers + ranking    │
  │  - tray icon   │                │   - previews               │
  │  - settings    │                │   - capability-gated actions│
  └────────────────┘                └────────────────────────────┘
```

| Crate / project        | Responsibility |
| ---------------------- | -------------- |
| `apps/Winspot.App`     | Avalonia UI: launcher window, tray icon, settings, hotkey. |
| `crates/winspot-core`  | IPC envelopes, `SearchResult`, capabilities. |
| `crates/winspot-daemon`| Named-pipe server; builds the search engine. |
| `crates/winspot-search`| Search providers, ranking, usage signals. |
| `crates/winspot-index` | SQLite-backed file index. |
| `crates/winspot-preview`| Result previews. |
| `crates/winspot-actions`| Capability-gated action executor. |
| `crates/winspot-plugins`| Plugin manifests + registry (see its [README](crates/winspot-plugins/README.md)). |

A deeper write-up lives in [`docs/architecture-current.md`](docs/architecture-current.md).

## Build & test

Requires the .NET SDK pinned in `global.json` and a stable Rust toolchain (MSVC).

```bash
# Rust
cargo build --workspace
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings

# App
dotnet build apps\Winspot.App\Winspot.App.csproj -c Debug -p:Platform=x64
dotnet test  apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug -p:Platform=x64
```

## Run

```bash
cargo run -p winspot-daemon                                   # backend
dotnet run --project apps\Winspot.App\Winspot.App.csproj -c Debug -p:Platform=x64
```

The app starts the daemon when needed. Press the activation hotkey (default
`Ctrl Alt Space`) to summon the launcher.

## Settings

Open settings from the **tray icon → Settings…**, or type `settings` in the launcher
and pick **Winspot Settings**. The settings window configures:

- the activation hotkey chord (modifiers + key),
- launch on sign-in,
- system-tray icon visibility,
- reduced motion.

## Plugins

Winspot ships built-in plugins (Calculator, Terminal, Clipboard, Unit Conversion)
and loads user-authored JSON manifests from `%LOCALAPPDATA%\Winspot\plugins`. See
the [plugin README](crates/winspot-plugins/README.md) for the manifest format,
capabilities, and validation rules.
