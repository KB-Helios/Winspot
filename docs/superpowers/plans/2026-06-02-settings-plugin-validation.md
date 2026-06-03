# Settings Plugin Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the Avalonia settings page with plugin validation options and clear validation status backed by the Rust daemon.

**Architecture:** Avalonia remains presentation-only: it requests plugin diagnostics over IPC, renders status/counts/issues, and owns transient UI state. Rust remains the validation source of truth; the daemon should return a fresh validation report for the configured plugin folder when the settings page asks for diagnostics.

**Tech Stack:** Avalonia 12 AXAML, .NET 10, MSTest, Rust daemon IPC, serde JSON plugin validation reports.

---

## Current UI Analysis

The settings page is already a compact tabbed Avalonia window with `General`, `Hotkey`, `Appearance`, `Plugins`, `Diagnostics`, and `About` tabs. `apps/Winspot.App/SettingsWindow.axaml` keeps the surface restrained with 8px panels and shared design tokens, matching the existing visual tests.

The current `Plugins` tab shows static plugin facts: built-ins, user manifest count, plugin folder path, and an open-folder button. It does not show validation state, validation errors, warning-only manifests, disabled manifests, trusted/search-only status, loading state, last checked time, or a manual validation action.

The daemon already has the validation contract: `PluginDiagnosticsRequested` and `PluginDiagnosticsReady` exist in `crates/winspot-core/src/ipc.rs`, and `crates/winspot-daemon/src/server.rs` currently returns `runtime.plugin_validation_report`. That report is built when the daemon runtime starts, so a settings-page "Validate now" action should rescan the configured plugin directory before responding; otherwise newly edited manifests can appear stale.

## Approach Options

**Recommended: IPC-backed fresh validation.** Add typed C# report models, an IPC diagnostics method, ViewModel status properties, and a richer `Plugins` tab. Update the daemon diagnostics handler to rebuild a report from the configured plugin directory on request. This keeps validation ownership in Rust and makes "Validate now" honest without adding plugin hot-reload yet.

**Lower risk: UI-only snapshot status.** Only add the C# IPC method and render the daemon's existing in-memory report. This is smaller, but the status can be stale after users edit manifests, so the button would need to say "Refresh loaded status" instead of "Validate now".

**Bigger scope: full plugin management.** Add enable/disable controls, plugin reload, trust management, and live registry mutation. This is not needed for the current request and would expand the daemon/provider model substantially.

## File Structure

- Create: `apps/Winspot.App/Models/PluginValidationDiagnostics.cs`
  - Owns the C# shape of the daemon validation report plus count/summary helpers.
- Modify: `apps/Winspot.App/Services/WinspotIpcClient.cs`
  - Adds `GetPluginDiagnosticsAsync` to `IWinspotIpcClient` and sends `PluginDiagnosticsRequested`.
- Modify: `apps/Winspot.App/ViewModels/SettingsViewModel.cs`
  - Adds validation state, issue filtering, last checked text, and async validation refresh.
- Modify: `apps/Winspot.App/Strings/SettingsDisplayStrings.cs`
  - Centralizes plugin validation labels/status strings.
- Modify: `apps/Winspot.App/SettingsWindow.axaml`
  - Reworks the `Plugins` tab into status summary, options, issue list, and actions.
- Modify: `apps/Winspot.App/SettingsWindow.axaml.cs`
  - Adds click handler for plugin validation.
- Modify: `apps/Winspot.App.Tests/ViewModels/SettingsViewModelTests.cs`
  - Adds ViewModel tests for success, warnings/errors, loading state, and backend failure.
- Modify: `apps/Winspot.App.Tests/Windowing/LauncherVisualStyleTests.cs`
  - Extends static AXAML tests for the plugin validation UI without relaxing current style constraints.
- Modify: `apps/Winspot.App.Tests/ViewModels/LauncherViewModelTests.cs`
  - Updates fake IPC client for the new interface method.
- Modify: `crates/winspot-daemon/src/server.rs`
  - Makes plugin diagnostics requests build a fresh validation report from `PipeConfig.plugins_dir`.
- Modify: `crates/winspot-daemon/tests/pipe_smoke.rs`
  - Adds regression coverage that diagnostics rescans files changed after runtime creation.

---

### Task 1: Add C# Plugin Validation Models

**Files:**
- Create: `apps/Winspot.App/Models/PluginValidationDiagnostics.cs`
- Test: `apps/Winspot.App.Tests/Models/PluginValidationDiagnosticsTests.cs`

- [ ] **Step 1: Write the failing model tests**

```csharp
using System;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App.Models;

namespace Winspot_App.Tests.Models;

[TestClass]
public sealed class PluginValidationDiagnosticsTests
{
    [TestMethod]
    public void Summary_WithNoIssues_ReportsReadyCounts()
    {
        var report = new PluginValidationReport(new[]
        {
            new PluginValidationEntry("calculator", "Calculator", null, "BuiltIn", "Accepted", true, Array.Empty<PluginValidationIssue>()),
            new PluginValidationEntry("notes", "Notes", "C:\\Plugins\\notes.json", "User", "Accepted", false, Array.Empty<PluginValidationIssue>()),
            new PluginValidationEntry("off", "Off", "C:\\Plugins\\off.json", "User", "Disabled", false, Array.Empty<PluginValidationIssue>()),
        });

        Assert.AreEqual(2, report.AcceptedCount);
        Assert.AreEqual(1, report.DisabledCount);
        Assert.AreEqual(0, report.RejectedCount);
        Assert.AreEqual(0, report.WarningCount);
        Assert.AreEqual(0, report.ErrorCount);
        Assert.AreEqual("Ready", report.Health);
        Assert.AreEqual("2 accepted, 1 disabled, 0 rejected", report.CountSummary);
    }

    [TestMethod]
    public void Summary_WithIssues_ReportsHighestSeverity()
    {
        var report = new PluginValidationReport(new[]
        {
            new PluginValidationEntry(
                "bad",
                "Bad",
                "C:\\Plugins\\bad.json",
                "User",
                "Rejected",
                false,
                new[]
                {
                    new PluginValidationIssue("Error", "Manifest", "invalid_manifest", "plugin id is invalid"),
                    new PluginValidationIssue("Warning", "Policy", "ignored_user_executable_capability", "capability ignored"),
                }),
        });

        Assert.AreEqual(1, report.RejectedCount);
        Assert.AreEqual(1, report.WarningCount);
        Assert.AreEqual(1, report.ErrorCount);
        Assert.AreEqual("Error", report.Health);
        Assert.AreEqual(2, report.IssueCount);
    }

    [TestMethod]
    public void Entry_WhenFieldsMissing_UsesPathOrUnknownLabel()
    {
        var pathEntry = new PluginValidationEntry(null, null, "C:\\Plugins\\broken.json", "User", "Rejected", false, Array.Empty<PluginValidationIssue>());
        var unknownEntry = new PluginValidationEntry(null, null, null, "User", "Rejected", false, Array.Empty<PluginValidationIssue>());

        Assert.AreEqual("broken.json", pathEntry.DisplayName);
        Assert.AreEqual("<unknown plugin>", unknownEntry.DisplayName);
    }
}
```

- [ ] **Step 2: Run the model tests to verify they fail**

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug --filter PluginValidationDiagnosticsTests -v minimal
```

Expected: fail because `PluginValidationReport`, `PluginValidationEntry`, and `PluginValidationIssue` do not exist.

- [ ] **Step 3: Add the model implementation**

```csharp
namespace Winspot_App.Models;

public sealed record PluginValidationReport(IReadOnlyList<PluginValidationEntry>? Entries)
{
    public static PluginValidationReport Empty { get; } = new(Array.Empty<PluginValidationEntry>());

    public IReadOnlyList<PluginValidationEntry> SafeEntries => Entries ?? Array.Empty<PluginValidationEntry>();

    public int AcceptedCount => SafeEntries.Count(entry => IsStatus(entry, "Accepted"));

    public int DisabledCount => SafeEntries.Count(entry => IsStatus(entry, "Disabled"));

    public int RejectedCount => SafeEntries.Count(entry => IsStatus(entry, "Rejected"));

    public int WarningCount => SafeEntries.Sum(entry => entry.SafeIssues.Count(issue => IsSeverity(issue, "Warning")));

    public int ErrorCount => SafeEntries.Sum(entry => entry.SafeIssues.Count(issue => IsSeverity(issue, "Error")));

    public int IssueCount => WarningCount + ErrorCount;

    public string Health => ErrorCount > 0
        ? "Error"
        : WarningCount > 0
            ? "Warning"
            : "Ready";

    public string CountSummary => $"{AcceptedCount} accepted, {DisabledCount} disabled, {RejectedCount} rejected";

    private static bool IsStatus(PluginValidationEntry entry, string status) =>
        string.Equals(entry.Status, status, StringComparison.OrdinalIgnoreCase);

    private static bool IsSeverity(PluginValidationIssue issue, string severity) =>
        string.Equals(issue.Severity, severity, StringComparison.OrdinalIgnoreCase);
}

public sealed record PluginValidationEntry(
    string? Id,
    string? Name,
    string? ManifestPath,
    string Source,
    string Status,
    bool Trusted,
    IReadOnlyList<PluginValidationIssue>? Issues)
{
    public IReadOnlyList<PluginValidationIssue> SafeIssues => Issues ?? Array.Empty<PluginValidationIssue>();

    public bool HasIssues => SafeIssues.Count > 0;

    public string DisplayName
    {
        get
        {
            if (!string.IsNullOrWhiteSpace(Name))
            {
                return Name;
            }

            if (!string.IsNullOrWhiteSpace(Id))
            {
                return Id;
            }

            if (!string.IsNullOrWhiteSpace(ManifestPath))
            {
                return Path.GetFileName(ManifestPath);
            }

            return "<unknown plugin>";
        }
    }

    public string TrustSummary => Trusted ? "Trusted" : "Search-only";
}

public sealed record PluginValidationIssue(
    string Severity,
    string Stage,
    string Code,
    string Message)
{
    public string DisplayText => $"[{Severity} {Stage} {Code}] {Message}";
}
```

- [ ] **Step 4: Run the model tests to verify they pass**

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug --filter PluginValidationDiagnosticsTests -v minimal
```

Expected: pass.

---

### Task 2: Add IPC Diagnostics Method

**Files:**
- Modify: `apps/Winspot.App/Services/WinspotIpcClient.cs`
- Modify: `apps/Winspot.App.Tests/ViewModels/LauncherViewModelTests.cs`
- Modify: `apps/Winspot.App.Tests/ViewModels/SettingsViewModelTests.cs`

- [ ] **Step 1: Add the interface method and verify compile failures**

In `IWinspotIpcClient`, add:

```csharp
Task<PluginValidationReport> GetPluginDiagnosticsAsync(CancellationToken cancellationToken);
```

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug --filter SettingsViewModelTests -v minimal
```

Expected: fail at compile time because `WinspotIpcClient`, `RecordingIpcClient`, and `FakeWinspotIpcClient` do not implement the new interface member.

- [ ] **Step 2: Update fake IPC clients**

Add this method to the `RecordingIpcClient` in `SettingsViewModelTests` and the `FakeWinspotIpcClient` in `LauncherViewModelTests`:

```csharp
public Task<PluginValidationReport> GetPluginDiagnosticsAsync(CancellationToken cancellationToken) =>
    Task.FromResult(PluginValidationReport.Empty);
```

- [ ] **Step 3: Implement daemon diagnostics IPC**

In `WinspotIpcClient`, add:

```csharp
public async Task<PluginValidationReport> GetPluginDiagnosticsAsync(CancellationToken cancellationToken)
{
    await using var pipe = await ConnectAsync(cancellationToken);

    await using var writer = new StreamWriter(pipe, leaveOpen: true) { AutoFlush = true };
    using var reader = new StreamReader(pipe, leaveOpen: true);

    await NegotiateAsync(writer, reader, cancellationToken);

    var requestId = Guid.NewGuid().ToString("N");
    var request = new IpcEnvelope(
        ProtocolVersion,
        requestId,
        new IpcPayload(
            "PluginDiagnosticsRequested",
            JsonSerializer.SerializeToElement(new PluginDiagnosticsRequested(), JsonOptions)));

    await writer.WriteLineAsync(
        JsonSerializer.Serialize(request, JsonOptions).AsMemory(),
        cancellationToken);

    var line = await reader.ReadLineAsync(cancellationToken);
    if (string.IsNullOrWhiteSpace(line))
    {
        throw new InvalidOperationException("Backend returned an empty plugin diagnostics response.");
    }

    var response = JsonSerializer.Deserialize<IpcEnvelope>(line, JsonOptions);
    if (response?.Payload?.Type == "Error")
    {
        var error = response.Payload.Data.Deserialize<BackendError>(JsonOptions);
        throw new InvalidOperationException(error?.Message ?? "Backend returned a plugin diagnostics error.");
    }

    if (response?.Payload?.Type != "PluginDiagnosticsReady")
    {
        throw new InvalidOperationException("Backend returned an unexpected plugin diagnostics response.");
    }

    var ready = response.Payload.Data.Deserialize<PluginDiagnosticsReady>(JsonOptions)
        ?? throw new InvalidOperationException("Backend returned an invalid plugin diagnostics response.");

    return ready.Report ?? PluginValidationReport.Empty;
}
```

Add these private records near the other IPC payload records:

```csharp
private sealed record PluginDiagnosticsRequested();

private sealed record PluginDiagnosticsReady(PluginValidationReport? Report);
```

- [ ] **Step 4: Run the affected C# tests**

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug --filter "SettingsViewModelTests|LauncherViewModelTests" -v minimal
```

Expected: pass after both fake clients implement the new method.

---

### Task 3: Make Daemon Diagnostics Rescan Plugin Manifests

**Files:**
- Modify: `crates/winspot-daemon/src/server.rs`
- Modify: `crates/winspot-daemon/tests/pipe_smoke.rs`

- [ ] **Step 1: Add the failing daemon regression test**

Add a test that builds the runtime before writing an invalid plugin, then asks for diagnostics:

```rust
#[tokio::test]
async fn daemon_plugin_diagnostics_rescans_configured_plugin_dir() {
    let pipe_name = format!(r"\\.\pipe\winspot-plugin-rescan-{}", std::process::id());
    let plugin_root =
        std::env::temp_dir().join(format!("winspot-plugin-rescan-{}", std::process::id()));
    fs::create_dir_all(&plugin_root).expect("create plugin dir");

    let config = PipeConfig {
        pipe_name: pipe_name.clone(),
        usage_log_path: None,
        plugins_dir: Some(plugin_root.clone()),
    };
    let runtime = build_daemon_runtime(&config).expect("build runtime before plugin appears");

    fs::write(
        plugin_root.join("bad-id.json"),
        r#"{"id":"Bad Id","name":"Bad","capabilities":[],"enabled":true}"#,
    )
    .expect("write invalid manifest after runtime build");

    let server_config = config.clone();
    let server = tokio::spawn(async move {
        serve_runtime_pipe_once(server_config, &runtime)
            .await
            .expect("pipe server completes");
    });

    let client = open_pipe_with_retry(&pipe_name).await;
    let mut client = BufReader::new(client);
    let request = IpcEnvelope::request(
        "plugins-rescan",
        IpcPayload::PluginDiagnosticsRequested(PluginDiagnosticsRequested {}),
    );
    let mut request_json = serde_json::to_string(&request).expect("serialize diagnostics");
    request_json.push('\n');
    client
        .get_mut()
        .write_all(request_json.as_bytes())
        .await
        .expect("write diagnostics request");

    let mut line = String::new();
    timeout(Duration::from_secs(2), client.read_line(&mut line))
        .await
        .expect("diagnostics response before timeout")
        .expect("read diagnostics response");
    let response: IpcEnvelope =
        serde_json::from_str(line.trim()).expect("decode diagnostics response");

    match response.payload {
        IpcPayload::PluginDiagnosticsReady(ready) => {
            let entries = ready.report["entries"].as_array().expect("entries array");
            let codes: Vec<&str> = entries
                .iter()
                .flat_map(|entry| entry["issues"].as_array().expect("issues"))
                .map(|issue| issue["code"].as_str().expect("issue code"))
                .collect();
            assert!(codes.contains(&"invalid_manifest"));
        }
        other => panic!("expected PluginDiagnosticsReady, got {other:?}"),
    }

    server.await.expect("server task joins");
    fs::remove_dir_all(plugin_root).expect("cleanup plugin dir");
}
```

- [ ] **Step 2: Run the daemon test to verify it fails**

Run:

```powershell
rtk cargo test -p winspot-daemon daemon_plugin_diagnostics_rescans_configured_plugin_dir
```

Expected: fail because diagnostics return the startup snapshot.

- [ ] **Step 3: Add fresh report helper**

In `server.rs`, add:

```rust
fn current_plugin_validation_report(
    runtime: &DaemonRuntime,
    config: &PipeConfig,
) -> PluginValidationReport {
    let Some(dir) = config.plugins_dir.as_deref() else {
        return runtime.plugin_validation_report.as_ref().clone();
    };

    let (mut registry, mut report) = PluginRegistry::with_built_ins_with_report();
    match registry.load_dir_into_with_report(dir) {
        Ok(user_report) => {
            report.extend(user_report);
            report
        }
        Err(error) => {
            eprintln!(
                "winspot-daemon: failed to rescan plugins directory {}: {error:?}",
                dir.display()
            );
            runtime.plugin_validation_report.as_ref().clone()
        }
    }
}
```

Then change the diagnostics handler to serialize this fresh report:

```rust
IpcPayload::PluginDiagnosticsRequested(_) => {
    let report = current_plugin_validation_report(runtime, config);
    Ok((
        vec![IpcEnvelope::request(
            request_id,
            IpcPayload::PluginDiagnosticsReady(PluginDiagnosticsReady {
                report: serde_json::to_value(&report)
                    .context("serialize plugin validation report")?,
            }),
        )],
        true,
    ))
}
```

- [ ] **Step 4: Run daemon diagnostics tests**

Run:

```powershell
rtk cargo test -p winspot-daemon plugin_diagnostics
```

Expected: all daemon plugin diagnostics tests pass.

---

### Task 4: Add Settings ViewModel Validation State

**Files:**
- Modify: `apps/Winspot.App/ViewModels/SettingsViewModel.cs`
- Modify: `apps/Winspot.App/Strings/SettingsDisplayStrings.cs`
- Test: `apps/Winspot.App.Tests/ViewModels/SettingsViewModelTests.cs`

- [ ] **Step 1: Write failing ViewModel tests**

Add tests:

```csharp
[TestMethod]
public async Task RefreshPluginValidationAsync_WhenReportHasIssues_UpdatesStatusAndEntries()
{
    var client = new RecordingIpcClient
    {
        PluginReport = new PluginValidationReport(new[]
        {
            new PluginValidationEntry(
                "bad",
                "Bad",
                "C:\\Plugins\\bad.json",
                "User",
                "Rejected",
                false,
                new[] { new PluginValidationIssue("Error", "Manifest", "invalid_manifest", "plugin id is invalid") }),
        }),
    };
    var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath), "C:\\Plugins", client);

    await viewModel.RefreshPluginValidationAsync(CancellationToken.None);

    Assert.AreEqual("Error", viewModel.PluginValidationHealth);
    StringAssert.Contains(viewModel.PluginValidationSummary, "0 accepted");
    StringAssert.Contains(viewModel.PluginValidationIssueSummary, "1 error");
    Assert.AreEqual(1, viewModel.PluginValidationEntries.Count);
    Assert.IsFalse(viewModel.IsPluginValidationRunning);
}

[TestMethod]
public async Task RefreshPluginValidationAsync_WhenBackendUnavailable_ShowsFailureStatus()
{
    var client = new RecordingIpcClient
    {
        PluginDiagnosticsException = new InvalidOperationException("pipe unavailable"),
    };
    var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath), "C:\\Plugins", client);

    await viewModel.RefreshPluginValidationAsync(CancellationToken.None);

    Assert.AreEqual("Unavailable", viewModel.PluginValidationHealth);
    StringAssert.Contains(viewModel.PluginValidationSummary, "Plugin validation unavailable");
    Assert.IsFalse(viewModel.IsPluginValidationRunning);
}
```

Extend `RecordingIpcClient`:

```csharp
public PluginValidationReport PluginReport { get; init; } = PluginValidationReport.Empty;

public Exception? PluginDiagnosticsException { get; init; }

public Task<PluginValidationReport> GetPluginDiagnosticsAsync(CancellationToken cancellationToken)
{
    if (PluginDiagnosticsException is not null)
    {
        throw PluginDiagnosticsException;
    }

    return Task.FromResult(PluginReport);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug --filter RefreshPluginValidationAsync -v minimal
```

Expected: fail because the ViewModel properties/method do not exist.

- [ ] **Step 3: Add centralized strings**

Add `using Winspot_App.Models;` to `SettingsDisplayStrings.cs`.

Add to `SettingsDisplayStrings`:

```csharp
public const string PluginValidationNotChecked = "Plugin validation not checked.";
public const string PluginValidationChecking = "Checking plugin manifests.";
public const string PluginValidationUnavailable = "Plugin validation unavailable.";
public const string PluginValidationReadyFormat = "{0}. {1}";
public const string PluginValidationIssueSummaryFormat = "{0} warning(s), {1} error(s)";
public const string PluginValidationLastCheckedFormat = "Last checked {0:t}";
```

Add helpers:

```csharp
public static string FormatPluginValidationSummary(PluginValidationReport report) => string.Format(
    CultureInfo.InvariantCulture,
    PluginValidationReadyFormat,
    report.Health,
    report.CountSummary);

public static string FormatPluginValidationIssueSummary(PluginValidationReport report) => string.Format(
    CultureInfo.InvariantCulture,
    PluginValidationIssueSummaryFormat,
    report.WarningCount,
    report.ErrorCount);

public static string FormatPluginValidationLastChecked(DateTimeOffset checkedAt) => string.Format(
    CultureInfo.CurrentCulture,
    PluginValidationLastCheckedFormat,
    checkedAt);
```

Extend `DiagnosticsFormat` to include plugin validation:

```csharp
public const string DiagnosticsFormat =
    "Settings: {0}\nPlugins: {1}\nPortable: {2}\nTheme: {3}\nMotion: {4}\nTray: {5}\nHotkey: {6}\nVersion: {7}\nPlugin validation: {8}";
```

Then add a `pluginValidationSummary` parameter to `FormatDiagnostics` and pass it as the final format argument.

- [ ] **Step 4: Add ViewModel state and refresh method**

Update `DiagnosticsText` so the Diagnostics tab mirrors the plugin validation state:

```csharp
public string DiagnosticsText => SettingsDisplayStrings.FormatDiagnostics(
    SettingsPath,
    PluginsPath,
    IsPortable,
    ThemeMode.ToString(),
    MotionProfile.ToString(),
    ShowTrayIcon,
    HotkeyPreview,
    AppVersion,
    PluginValidationSummary);
```

Add fields:

```csharp
private bool _isPluginValidationRunning;
private PluginValidationReport _pluginValidationReport = PluginValidationReport.Empty;
private IReadOnlyList<PluginValidationEntry> _pluginValidationEntries = Array.Empty<PluginValidationEntry>();
private string _pluginValidationSummary = SettingsDisplayStrings.PluginValidationNotChecked;
private string _pluginValidationIssueSummary = SettingsDisplayStrings.FormatPluginValidationIssueSummary(PluginValidationReport.Empty);
private string _pluginValidationHealth = "Unchecked";
private string _pluginValidationLastCheckedText = string.Empty;
private bool _showOnlyPluginValidationIssues = true;
```

Add properties:

```csharp
public bool IsPluginValidationRunning
{
    get => _isPluginValidationRunning;
    private set => SetField(ref _isPluginValidationRunning, value);
}

public string PluginValidationSummary
{
    get => _pluginValidationSummary;
    private set => SetField(ref _pluginValidationSummary, value);
}

public string PluginValidationIssueSummary
{
    get => _pluginValidationIssueSummary;
    private set => SetField(ref _pluginValidationIssueSummary, value);
}

public string PluginValidationHealth
{
    get => _pluginValidationHealth;
    private set => SetField(ref _pluginValidationHealth, value);
}

public string PluginValidationLastCheckedText
{
    get => _pluginValidationLastCheckedText;
    private set => SetField(ref _pluginValidationLastCheckedText, value);
}

public bool ShowOnlyPluginValidationIssues
{
    get => _showOnlyPluginValidationIssues;
    set
    {
        if (SetField(ref _showOnlyPluginValidationIssues, value))
        {
            RefreshPluginValidationEntries();
        }
    }
}

public IReadOnlyList<PluginValidationEntry> PluginValidationEntries
{
    get => _pluginValidationEntries;
    private set => SetField(ref _pluginValidationEntries, value);
}
```

Add methods:

```csharp
public async Task RefreshPluginValidationAsync(CancellationToken cancellationToken)
{
    IsPluginValidationRunning = true;
    PluginValidationSummary = SettingsDisplayStrings.PluginValidationChecking;
    PluginValidationHealth = "Checking";

    try
    {
        _pluginValidationReport = await _ipcClient.GetPluginDiagnosticsAsync(cancellationToken).ConfigureAwait(true);
        PluginValidationHealth = _pluginValidationReport.Health;
        PluginValidationSummary = SettingsDisplayStrings.FormatPluginValidationSummary(_pluginValidationReport);
        PluginValidationIssueSummary = SettingsDisplayStrings.FormatPluginValidationIssueSummary(_pluginValidationReport);
        PluginValidationLastCheckedText = SettingsDisplayStrings.FormatPluginValidationLastChecked(DateTimeOffset.Now);
        RefreshPluginValidationEntries();
        OnPropertyChanged(nameof(DiagnosticsText));
    }
    catch (OperationCanceledException)
    {
    }
    catch
    {
        _pluginValidationReport = PluginValidationReport.Empty;
        PluginValidationEntries = Array.Empty<PluginValidationEntry>();
        PluginValidationHealth = "Unavailable";
        PluginValidationSummary = SettingsDisplayStrings.PluginValidationUnavailable;
        PluginValidationIssueSummary = SettingsDisplayStrings.FormatPluginValidationIssueSummary(PluginValidationReport.Empty);
        PluginValidationLastCheckedText = string.Empty;
    }
    finally
    {
        IsPluginValidationRunning = false;
    }
}

private void RefreshPluginValidationEntries()
{
    PluginValidationEntries = ShowOnlyPluginValidationIssues
        ? _pluginValidationReport.SafeEntries.Where(entry => entry.HasIssues).ToArray()
        : _pluginValidationReport.SafeEntries.ToArray();
}
```

- [ ] **Step 5: Run the ViewModel tests**

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug --filter SettingsViewModelTests -v minimal
```

Expected: pass.

---

### Task 5: Rework the Avalonia Plugins Tab

**Files:**
- Modify: `apps/Winspot.App/SettingsWindow.axaml`
- Modify: `apps/Winspot.App/SettingsWindow.axaml.cs`
- Modify: `apps/Winspot.App.Tests/Windowing/LauncherVisualStyleTests.cs`

- [ ] **Step 1: Add failing visual assertions**

Extend `SettingsWindow_WhenRendered_ContainsCompactTabbedSections`:

```csharp
StringAssert.Contains(axaml, "PluginValidationSummary");
StringAssert.Contains(axaml, "PluginValidationEntries");
StringAssert.Contains(axaml, "OnValidatePluginsClick");
StringAssert.Contains(axaml, "ShowOnlyPluginValidationIssues");
StringAssert.Contains(axaml, "ScrollViewer");
```

- [ ] **Step 2: Run the visual-style test**

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug --filter SettingsWindow_WhenRendered_ContainsCompactTabbedSections -v minimal
```

Expected: fail until the AXAML is updated.

- [ ] **Step 3: Update the Plugins tab layout**

Add the model namespace to the root `Window` element:

```xml
xmlns:models="using:Winspot_App.Models"
```

Replace the current `Plugins` tab content with this structure, preserving the existing panel style and 8px radius:

```xml
<TabItem Header="Plugins">
    <ScrollViewer Margin="0,10,0,0" VerticalScrollBarVisibility="Auto">
        <StackPanel Spacing="12">
            <Border Classes="panel">
                <StackPanel Spacing="12">
                    <Grid ColumnDefinitions="*,Auto,Auto" ColumnSpacing="10">
                        <StackPanel Spacing="3">
                            <TextBlock Classes="section-title" Text="Plugin validation" />
                            <TextBlock Classes="hint" Text="{Binding PluginValidationSummary}" TextWrapping="Wrap" />
                            <TextBlock Classes="hint" Text="{Binding PluginValidationIssueSummary}" />
                            <TextBlock Classes="hint" Text="{Binding PluginValidationLastCheckedText}" />
                        </StackPanel>
                        <Button
                            Grid.Column="1"
                            Click="OnValidatePluginsClick"
                            Content="Validate"
                            IsEnabled="{Binding !IsPluginValidationRunning}"
                            MinWidth="92" />
                        <Button
                            Grid.Column="2"
                            Classes="icon-button"
                            Click="OnOpenPluginsFolderClick"
                            ToolTip.Tip="Open folder">
                            <TextBlock
                                FontFamily="Segoe Fluent Icons, Segoe MDL2 Assets"
                                FontSize="15"
                                Text="&#xE838;" />
                        </Button>
                    </Grid>

                    <CheckBox
                        Content="Show only manifests with issues"
                        IsChecked="{Binding ShowOnlyPluginValidationIssues}" />
                </StackPanel>
            </Border>

            <Border Classes="panel">
                <StackPanel Spacing="12">
                    <TextBlock Classes="section-title" Text="Plugin folder" />
                    <Grid Classes="setting-row" ColumnDefinitions="210,*" ColumnSpacing="18">
                        <TextBlock Classes="label" VerticalAlignment="Center" Text="Built-in" />
                        <TextBlock
                            Grid.Column="1"
                            Classes="hint"
                            VerticalAlignment="Center"
                            Text="{Binding BuiltInPluginsText}"
                            TextWrapping="Wrap" />
                    </Grid>
                    <Grid Classes="setting-row" ColumnDefinitions="210,*" ColumnSpacing="18">
                        <TextBlock Classes="label" VerticalAlignment="Center" Text="User manifests" />
                        <TextBlock
                            Grid.Column="1"
                            Classes="hint"
                            VerticalAlignment="Center"
                            Text="{Binding UserPluginManifestCountText}" />
                    </Grid>
                    <Grid ColumnDefinitions="210,*" ColumnSpacing="18">
                        <TextBlock Classes="label" VerticalAlignment="Center" Text="Folder" />
                        <TextBlock
                            Grid.Column="1"
                            Classes="hint"
                            VerticalAlignment="Center"
                            Text="{Binding PluginsPath}"
                            TextTrimming="CharacterEllipsis" />
                    </Grid>
                </StackPanel>
            </Border>

            <ItemsControl ItemsSource="{Binding PluginValidationEntries}">
                <ItemsControl.ItemTemplate>
                    <DataTemplate DataType="models:PluginValidationEntry" x:DataType="models:PluginValidationEntry">
                        <Border Classes="panel" Margin="0,0,0,8">
                            <StackPanel Spacing="6">
                                <Grid ColumnDefinitions="*,Auto,Auto" ColumnSpacing="10">
                                    <TextBlock Classes="label" Text="{Binding DisplayName}" TextTrimming="CharacterEllipsis" />
                                    <TextBlock Grid.Column="1" Classes="hint" Text="{Binding Status}" />
                                    <TextBlock Grid.Column="2" Classes="hint" Text="{Binding TrustSummary}" />
                                </Grid>
                                <TextBlock Classes="hint" Text="{Binding ManifestPath}" TextTrimming="CharacterEllipsis" />
                                <ItemsControl ItemsSource="{Binding SafeIssues}">
                                    <ItemsControl.ItemTemplate>
                                        <DataTemplate DataType="models:PluginValidationIssue" x:DataType="models:PluginValidationIssue">
                                            <TextBlock
                                                Classes="hint"
                                                Text="{Binding DisplayText}"
                                                TextWrapping="Wrap" />
                                        </DataTemplate>
                                    </ItemsControl.ItemTemplate>
                                </ItemsControl>
                            </StackPanel>
                        </Border>
                    </DataTemplate>
                </ItemsControl.ItemTemplate>
            </ItemsControl>
        </StackPanel>
    </ScrollViewer>
</TabItem>
```

- [ ] **Step 4: Add the click handler**

In `SettingsWindow.axaml.cs`:

```csharp
private async void OnValidatePluginsClick(object? sender, RoutedEventArgs e)
{
    await ViewModel.RefreshPluginValidationAsync(CancellationToken.None);
}
```

- [ ] **Step 5: Run the visual-style test**

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug --filter SettingsWindow_WhenRendered_ContainsCompactTabbedSections -v minimal
```

Expected: pass, with existing corner-radius assertions still intact.

---

### Task 6: Wire Initial Validation Refresh

**Files:**
- Modify: `apps/Winspot.App/SettingsWindow.axaml.cs`
- Test: `apps/Winspot.App.Tests/ViewModels/SettingsViewModelTests.cs`

- [ ] **Step 1: Add a ViewModel test for initial unchecked state**

```csharp
[TestMethod]
public void Constructor_PluginValidationStartsUnchecked()
{
    var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath));

    Assert.AreEqual("Unchecked", viewModel.PluginValidationHealth);
    Assert.AreEqual(SettingsDisplayStrings.PluginValidationNotChecked, viewModel.PluginValidationSummary);
}
```

- [ ] **Step 2: Trigger validation once the settings window opens**

In the `SettingsWindow` constructor after `InitializeComponent()`:

```csharp
Opened += OnOpened;
```

Add:

```csharp
private async void OnOpened(object? sender, EventArgs e)
{
    await ViewModel.RefreshPluginValidationAsync(CancellationToken.None);
}
```

- [ ] **Step 3: Keep manual validation available**

Do not remove the `Validate` button; initial refresh gives the page a useful first status, and manual validation handles edits while the page stays open.

- [ ] **Step 4: Run C# tests**

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug -v minimal
```

Expected: pass.

---

### Task 7: Full Verification

**Files:**
- All touched C#, AXAML, and Rust files.

- [ ] **Step 1: Run C# unit tests**

Run:

```powershell
rtk proxy dotnet test apps\Winspot.App.Tests\Winspot.App.Tests.csproj -c Debug -v minimal
```

Expected: all tests pass.

- [ ] **Step 2: Run Rust plugin and daemon tests**

Run:

```powershell
rtk cargo test -p winspot-plugins
rtk cargo test -p winspot-daemon plugin_diagnostics
```

Expected: all selected Rust tests pass.

- [ ] **Step 3: Build the Avalonia app**

Run:

```powershell
rtk dotnet build apps\Winspot.App\Winspot.App.csproj -c Debug -p:Platform=x64
```

Expected: build succeeds.

- [ ] **Step 4: Manual smoke check**

Run the daemon and app:

```powershell
rtk cargo run -p winspot-daemon
rtk dotnet run --project apps\Winspot.App\Winspot.App.csproj -c Debug -p:Platform=x64
```

Expected:
- Settings opens from tray/menu or launcher command.
- Plugins tab shows validation status after opening.
- Validate button updates status without freezing the UI.
- Invalid manifest JSON appears as a rejected entry with issue code/message.
- Warning-only manifest appears as accepted with warning state.
- Open folder button still dispatches through IPC.
