using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
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
        });

        var viewModel = new SettingsViewModel(store);

        Assert.IsTrue(viewModel.UseControl);
        Assert.IsTrue(viewModel.UseShift);
        Assert.IsFalse(viewModel.UseAlt);
        Assert.AreEqual("K", viewModel.Key);
        Assert.IsTrue(viewModel.LaunchOnStartup);
        Assert.IsFalse(viewModel.ShowTrayIcon);
        Assert.IsTrue(viewModel.ReduceMotion);
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
        };

        var saved = viewModel.TrySave();

        Assert.IsTrue(saved);

        var reloaded = new LauncherSettingsStore(_settingsPath).Load();
        Assert.AreEqual("J", reloaded.Hotkey.Key);
        CollectionAssert.AreEquivalent(new List<string> { "Control", "Shift" }, reloaded.Hotkey.Modifiers);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.IsTrue(reloaded.ReduceMotion);
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
}
