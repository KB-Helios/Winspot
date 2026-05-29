# Agent Instructions -- Winspot Avalonia Desktop App

## Project Overview

`Winspot.App` is a native desktop launcher built with Avalonia 12 on .NET 10. The UI is a compact, keyboard-first spotlight surface that talks to the Rust `winspot-daemon` over local named-pipe IPC.

## Source Of Truth

Always read `Winspot.App.csproj` before changing framework, package, TFM, platform, namespace, or build details. At the time this file was written:

- UI framework: Avalonia 12
- App model: classic desktop lifetime
- Target framework: read from the project file
- Root namespace: read from the project file
- Backend: Rust daemon in `crates/winspot-daemon`

## Working Rules

- Prefer Avalonia-native AXAML and controls. Do not add WinUI, Windows App SDK, or MSIX-specific dependencies unless the user explicitly asks for that path.
- Keep the launcher transient and keyboard-first: focus the search box on open, hide on Escape, execute the selected result on Enter.
- Preserve the IPC boundary: UI owns presentation, windowing, focus, hotkey handling, and animation; Rust owns search, previews, and actions.
- Keep the first appearance slim. The window should initially present as a focused text box and expand only after the user starts a query.
- Use restrained, fast animation. Favor opacity, transform, and small height changes over heavy effects or long timelines.
- Keep user-facing strings short and ready for later localization.
- Avoid UI-thread blocking. Pipe I/O, backend launch, preview loading, and action execution stay async.
- Build and test before claiming completion.

## Common Commands

Run from the repository root:

```powershell
rtk dotnet build apps\Winspot.App\Winspot.App.csproj -c Debug -p:Platform=x64
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug -v minimal
rtk cargo test
```

For a local manual run:

```powershell
rtk cargo run -p winspot-daemon
rtk dotnet run --project apps\Winspot.App\Winspot.App.csproj -c Debug -p:Platform=x64
```
