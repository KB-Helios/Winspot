using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App.Models;
using Winspot_App.Services;
using Winspot_App.Strings;
using Winspot_App.ViewModels;

namespace Winspot_App.Tests.ViewModels;

[TestClass]
public sealed class SettingsViewModelTests
{
    private string _settingsPath = string.Empty;

    [TestInitialize]
    public void TestInitialize()
    {
        _settingsPath = Path.Combine(Path.GetTempPath(), $"winspot-settings-{Path.GetRandomFileName()}.json");
    }

    [TestCleanup]
    public void TestCleanup()
    {
        if (File.Exists(_settingsPath))
        {
            File.Delete(_settingsPath);
        }
    }

    [TestMethod]
    public void Constructor_WithDefaultSettings_LoadsCtrlAltSpaceChord()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath));

        Assert.IsTrue(viewModel.UseControl);
        Assert.IsTrue(viewModel.UseAlt);
        Assert.IsFalse(viewModel.UseShift);
        Assert.IsFalse(viewModel.UseWin);
        Assert.AreEqual("Space", viewModel.Key);
        Assert.IsTrue(viewModel.ShowTrayIcon);
        Assert.IsFalse(viewModel.LaunchOnStartup);
        Assert.AreEqual(ThemeMode.Dark, viewModel.ThemeMode);
        Assert.AreEqual(MotionProfile.Snappy240, viewModel.MotionProfile);
        Assert.AreEqual("Ctrl Alt Space", viewModel.HotkeyPreview);
    }

    [TestMethod]
    public void Constructor_LoadsExistingSettingsFromStore()
    {
        var store = new LauncherSettingsStore(_settingsPath);
        store.Save(new LauncherSettings
        {
            Hotkey = new HotkeyBinding { Key = "K", Modifiers = new List<string> { "Control", "Shift" } },
            LaunchOnStartup = true,
            ShowTrayIcon = false,
            ReduceMotion = true,
            ThemeMode = ThemeMode.Light,
            MotionProfile = MotionProfile.Reduced,
        });

        var viewModel = new SettingsViewModel(store);

        Assert.IsTrue(viewModel.UseControl);
        Assert.IsTrue(viewModel.UseShift);
        Assert.IsFalse(viewModel.UseAlt);
        Assert.AreEqual("K", viewModel.Key);
        Assert.IsTrue(viewModel.LaunchOnStartup);
        Assert.IsFalse(viewModel.ShowTrayIcon);
        Assert.IsTrue(viewModel.ReduceMotion);
        Assert.AreEqual(ThemeMode.Light, viewModel.ThemeMode);
        Assert.AreEqual(MotionProfile.Reduced, viewModel.MotionProfile);
    }

    [TestMethod]
    public void HotkeyPreview_UpdatesWhenChordChanges()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath));
        var previewChanged = false;
        viewModel.PropertyChanged += (_, args) =>
        {
            if (args.PropertyName == nameof(SettingsViewModel.HotkeyPreview))
            {
                previewChanged = true;
            }
        };

        viewModel.UseShift = true;

        Assert.IsTrue(previewChanged);
        Assert.AreEqual("Ctrl Alt Shift Space", viewModel.HotkeyPreview);
    }

    [TestMethod]
    public void TrySave_WithNoModifiers_FailsAndDoesNotPersist()
    {
        var store = new LauncherSettingsStore(_settingsPath);
        var viewModel = new SettingsViewModel(store)
        {
            UseControl = false,
            UseAlt = false,
            UseShift = false,
            UseWin = false,
        };

        var saved = viewModel.TrySave();

        Assert.IsFalse(saved);
        Assert.IsFalse(viewModel.IsValid);
        Assert.IsTrue(store.Load().Hotkey.Modifiers.Count > 0, "Existing settings must be left untouched.");
    }

    [TestMethod]
    public void TrySave_WithInvalidKey_Fails()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath))
        {
            Key = "NotARealKey",
        };

        Assert.IsFalse(viewModel.TrySave());
    }

    [TestMethod]
    public void TrySave_WithReservedWindowsHotkey_FailsAndDoesNotPersist()
    {
        var store = new LauncherSettingsStore(_settingsPath);
        var viewModel = new SettingsViewModel(store)
        {
            UseControl = true,
            UseAlt = false,
            UseWin = true,
            Key = "Space",
        };

        Assert.IsFalse(viewModel.TrySave());
        Assert.AreEqual(SettingsStatusMessages.ReservedWindowsHotkey, viewModel.StatusMessage);
        CollectionAssert.AreEqual(new List<string> { "Control", "Alt" }, store.Load().Hotkey.Modifiers);
    }

    [TestMethod]
    public void TrySave_WhenKeyClearedToNullOrEmpty_FailsWithoutThrowing()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath));

        viewModel.Key = null!;
        Assert.IsFalse(viewModel.IsValid);
        Assert.IsFalse(viewModel.TrySave());

        viewModel.Key = "   ";
        Assert.IsFalse(viewModel.IsValid);
        Assert.IsFalse(viewModel.TrySave());
    }

    [TestMethod]
    public void TrySave_WithValidChord_PersistsAndRoundTrips()
    {
        var store = new LauncherSettingsStore(_settingsPath);
        var viewModel = new SettingsViewModel(store)
        {
            UseControl = true,
            UseAlt = false,
            UseShift = true,
            UseWin = false,
            Key = "J",
            ShowTrayIcon = false,
            LaunchOnStartup = false,
            ReduceMotion = true,
            ThemeMode = ThemeMode.System,
            MotionProfile = MotionProfile.Reduced,
        };

        var saved = viewModel.TrySave();

        Assert.IsTrue(saved);

        var reloaded = new LauncherSettingsStore(_settingsPath).Load();
        Assert.AreEqual("J", reloaded.Hotkey.Key);
        CollectionAssert.AreEquivalent(new List<string> { "Control", "Shift" }, reloaded.Hotkey.Modifiers);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.IsTrue(reloaded.ReduceMotion);
        Assert.AreEqual(ThemeMode.System, reloaded.ThemeMode);
        Assert.AreEqual(MotionProfile.Reduced, reloaded.MotionProfile);
    }

    [TestMethod]
    public void TrySave_WithFastFlowLmDisabledAndInvalidNumbers_PersistsDisabledSettings()
    {
        var store = new LauncherSettingsStore(_settingsPath);
        var viewModel = new SettingsViewModel(store)
        {
            FastFlowLmEnabled = false,
            FastFlowLmPort = string.Empty,
            FastFlowLmIdleTimeoutSeconds = "not-a-number",
            FastFlowLmMaxContextFiles = "0",
            FastFlowLmMaxFileBytes = "-1",
            FastFlowLmMaxContextBytes = " ",
        };

        Assert.IsTrue(viewModel.TrySave());

        var reloaded = store.Load();
        Assert.IsFalse(reloaded.FastFlowLm.Enabled);
    }

    [TestMethod]
    public void BuildSettings_WithFastFlowLmPortAboveTcpRange_UsesDefaultPort()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath))
        {
            FastFlowLmPort = "99999",
        };

        Assert.AreEqual(52625, viewModel.BuildSettings().FastFlowLm.Port);
    }

    [TestMethod]
    public void TrySave_WithCaptureEnabledAndInvalidDurations_FailsWithoutPersisting()
    {
        var store = new LauncherSettingsStore(_settingsPath);
        var viewModel = new SettingsViewModel(store)
        {
            CaptureEnabled = true,
            CaptureDefaultRecordSeconds = "0",
            CaptureMaxRecordSeconds = "not-a-number",
            CapturePreCaptureDelayMs = "-1",
        };

        var saved = viewModel.TrySave();

        Assert.IsFalse(saved);
        Assert.IsFalse(viewModel.IsValid);
        StringAssert.Contains(viewModel.StatusMessage, "Capture numeric settings");
        Assert.AreEqual(8, store.Load().Capture.DefaultRecordSeconds);
    }

    [TestMethod]
    public void BuildSettings_WithCaptureFields_TrimsAndParsesValues()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath))
        {
            CaptureEnabled = false,
            CaptureOutputDirectory = @" C:\Captures ",
            CaptureDefaultRecordSeconds = "5",
            CaptureMaxRecordSeconds = "20",
            CaptureIncludeCursor = false,
            CapturePreCaptureDelayMs = "750",
        };

        var settings = viewModel.BuildSettings().Capture;

        Assert.IsFalse(settings.Enabled);
        Assert.AreEqual(@"C:\Captures", settings.OutputDirectory);
        Assert.AreEqual(5, settings.DefaultRecordSeconds);
        Assert.AreEqual(20, settings.MaxRecordSeconds);
        Assert.IsFalse(settings.IncludeCursor);
        Assert.AreEqual(750, settings.PreCaptureDelayMs);
    }

    [TestMethod]
    public void TrySave_RaisesSavedEventWithSnapshot()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath))
        {
            UseControl = true,
            Key = "M",
        };

        LauncherSettings? captured = null;
        viewModel.Saved += (_, settings) => captured = settings;

        Assert.IsTrue(viewModel.TrySave());
        Assert.IsNotNull(captured);
        Assert.AreEqual("M", captured!.Hotkey.Key);
    }

    [TestMethod]
    public void SetHotkeyRegistrationStatus_WhenUnavailable_KeepsConflictVisible()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath));

        viewModel.SetHotkeyRegistrationStatus(false);

        Assert.IsTrue(viewModel.StatusMessage.Contains("previous hotkey"));
    }

    [TestMethod]
    public void Diagnostics_WhenConstructed_ExposeLocalSettingsAndPluginFacts()
    {
        var pluginsRoot = Path.Combine(Path.GetTempPath(), $"winspot-plugins-{Guid.NewGuid():N}");
        Directory.CreateDirectory(pluginsRoot);
        File.WriteAllText(Path.Combine(pluginsRoot, "notes.json"), "{}");
        File.WriteAllText(Path.Combine(pluginsRoot, "readme.txt"), "ignored");
        try
        {
            var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath), pluginsRoot);

            Assert.AreEqual(_settingsPath, viewModel.SettingsPath);
            Assert.AreEqual(pluginsRoot, viewModel.PluginsPath);
            Assert.AreEqual(1, viewModel.UserPluginManifestCount);
            StringAssert.Contains(viewModel.BuiltInPluginsText, "Calculator");
            StringAssert.Contains(viewModel.BuiltInPluginsText, "Unit Conversion");
            StringAssert.Contains(viewModel.DiagnosticsText, "Theme: Dark");
            StringAssert.Contains(viewModel.DiagnosticsText, "Motion: Snappy240");
        }
        finally
        {
            if (Directory.Exists(pluginsRoot))
            {
                Directory.Delete(pluginsRoot, recursive: true);
            }
        }
    }

    [TestMethod]
    public void MotionProfile_WhenSetToReduced_KeepsReduceMotionCompatibilityTrue()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath))
        {
            MotionProfile = MotionProfile.Reduced,
        };

        Assert.IsTrue(viewModel.ReduceMotion);
        Assert.AreEqual(MotionProfile.Reduced, viewModel.BuildSettings().MotionProfile);
    }

    [TestMethod]
    public void Constructor_PluginValidationStartsUnchecked()
    {
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath));

        Assert.AreEqual("Unchecked", viewModel.PluginValidationHealth);
        Assert.AreEqual("Plugin validation not checked.", viewModel.PluginValidationSummary);
        Assert.AreEqual(string.Empty, viewModel.PluginValidationIssueSummary);
    }

    [TestMethod]
    public async Task OpenPluginsFolderAsync_WhenInvoked_DispatchesFolderOpenThroughIpc()
    {
        var pluginsRoot = Path.Combine(Path.GetTempPath(), $"winspot-plugins-open-{Guid.NewGuid():N}");
        var client = new RecordingIpcClient();
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath), pluginsRoot, client);

        await viewModel.OpenPluginsFolderAsync(CancellationToken.None);

        Assert.IsNotNull(client.LastResult);
        Assert.AreEqual($"folder:{pluginsRoot}", client.LastResult!.Id);
        Assert.AreEqual("Open", client.LastAction?.Kind);
        Assert.AreEqual("Opened plugins folder.", viewModel.StatusMessage);
    }

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
        Assert.AreEqual("Bad", viewModel.PluginValidationEntries[0].DisplayName);
        Assert.AreEqual("Search-only", viewModel.PluginValidationEntries[0].TrustSummary);
        Assert.AreEqual("[Error Manifest invalid_manifest] plugin id is invalid", viewModel.PluginValidationEntries[0].Issues[0].DisplayText);
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
        Assert.AreEqual(string.Empty, viewModel.PluginValidationIssueSummary);
        Assert.IsFalse(viewModel.IsPluginValidationRunning);
    }

    [TestMethod]
    public async Task RefreshPluginValidationAsync_WhenAlreadyRunning_DoesNotStartSecondRequest()
    {
        var completion = new TaskCompletionSource<PluginValidationReport>(TaskCreationOptions.RunContinuationsAsynchronously);
        var client = new RecordingIpcClient
        {
            PluginReportTask = completion.Task,
        };
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath), "C:\\Plugins", client);

        var firstRefresh = viewModel.RefreshPluginValidationAsync(CancellationToken.None);
        await WaitUntilAsync(() => viewModel.IsPluginValidationRunning);

        await viewModel.RefreshPluginValidationAsync(CancellationToken.None);

        Assert.AreEqual(1, client.PluginDiagnosticsCallCount);
        completion.SetResult(PluginValidationReport.Empty);
        await firstRefresh;
    }

    [TestMethod]
    public async Task RefreshPluginValidationAsync_WhenCanceledBeforeFirstReport_RestoresUncheckedState()
    {
        var client = new RecordingIpcClient
        {
            PluginDiagnosticsException = new OperationCanceledException(),
        };
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath), "C:\\Plugins", client);

        await viewModel.RefreshPluginValidationAsync(CancellationToken.None);

        Assert.AreEqual("Unchecked", viewModel.PluginValidationHealth);
        Assert.AreEqual("Plugin validation not checked.", viewModel.PluginValidationSummary);
        Assert.AreEqual(string.Empty, viewModel.PluginValidationIssueSummary);
        Assert.AreEqual(string.Empty, viewModel.PluginValidationLastCheckedText);
        Assert.IsFalse(viewModel.IsPluginValidationRunning);
    }

    [TestMethod]
    public async Task RefreshPluginValidationAsync_WhenCanceledAfterReport_RestoresPreviousState()
    {
        var client = new RecordingIpcClient
        {
            PluginReport = new PluginValidationReport(new[]
            {
                new PluginValidationEntry(
                    "warn",
                    "Warn",
                    "C:\\Plugins\\warn.json",
                    "User",
                    "Accepted",
                    true,
                    new[] { new PluginValidationIssue("Warning", "Policy", "ignored", "capability ignored") }),
            }),
        };
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath), "C:\\Plugins", client);
        await viewModel.RefreshPluginValidationAsync(CancellationToken.None);
        var previousSummary = viewModel.PluginValidationSummary;
        var previousIssueSummary = viewModel.PluginValidationIssueSummary;
        var previousLastChecked = viewModel.PluginValidationLastCheckedText;

        client.PluginDiagnosticsException = new OperationCanceledException();

        await viewModel.RefreshPluginValidationAsync(CancellationToken.None);

        Assert.AreEqual("Warning", viewModel.PluginValidationHealth);
        Assert.AreEqual(previousSummary, viewModel.PluginValidationSummary);
        Assert.AreEqual(previousIssueSummary, viewModel.PluginValidationIssueSummary);
        Assert.AreEqual(previousLastChecked, viewModel.PluginValidationLastCheckedText);
        Assert.AreEqual(1, viewModel.PluginValidationEntries.Count);
    }

    [TestMethod]
    public async Task RefreshPluginValidationAsync_WhenManifestPathIsMalformed_UsesRawPathAsDisplayName()
    {
        var malformedPath = "C:\\Plugins\\\0bad.json";
        var client = new RecordingIpcClient
        {
            PluginReport = new PluginValidationReport(new[]
            {
                new PluginValidationEntry(
                    null,
                    null,
                    malformedPath,
                    "User",
                    "Rejected",
                    false,
                    new[] { new PluginValidationIssue("Error", "Manifest", "invalid_manifest", "plugin id is invalid") }),
            }),
        };
        var viewModel = new SettingsViewModel(new LauncherSettingsStore(_settingsPath), "C:\\Plugins", client);

        await viewModel.RefreshPluginValidationAsync(CancellationToken.None);

        Assert.AreEqual(Path.GetFileName(malformedPath), viewModel.PluginValidationEntries[0].DisplayName);
    }

    private sealed class RecordingIpcClient : IWinspotIpcClient
    {
        public SearchResultItem? LastResult { get; private set; }

        public ActionItem? LastAction { get; private set; }

        public PluginValidationReport PluginReport { get; init; } = PluginValidationReport.Empty;

        public Task<PluginValidationReport>? PluginReportTask { get; init; }

        public Exception? PluginDiagnosticsException { get; set; }

        public int PluginDiagnosticsCallCount { get; private set; }

        public Task<IReadOnlyList<SearchResultItem>> SearchAsync(string query, CancellationToken cancellationToken) =>
            Task.FromResult<IReadOnlyList<SearchResultItem>>(Array.Empty<SearchResultItem>());

        public Task<string> ExecuteAsync(SearchResultItem result, ActionItem action, CancellationToken cancellationToken)
        {
            LastResult = result;
            LastAction = action;
            return Task.FromResult("Opened plugins folder.");
        }

        public Task<PreviewItem?> GetPreviewAsync(SearchResultItem result, CancellationToken cancellationToken) =>
            Task.FromResult<PreviewItem?>(null);

        public Task<PluginValidationReport> GetPluginDiagnosticsAsync(CancellationToken cancellationToken)
        {
            PluginDiagnosticsCallCount++;
            if (PluginDiagnosticsException is not null)
            {
                return Task.FromException<PluginValidationReport>(PluginDiagnosticsException);
            }

            return PluginReportTask is not null
                ? PluginReportTask.WaitAsync(cancellationToken)
                : Task.FromResult(PluginReport);
        }
    }

    private static async Task WaitUntilAsync(Func<bool> condition)
    {
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        while (!condition())
        {
            await Task.Delay(10, timeout.Token);
        }
    }
}
