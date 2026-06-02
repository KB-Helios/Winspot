using System.Collections.Generic;
using System.IO;
using System;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App.Models;
using Winspot_App.Services;

namespace Winspot_App.Tests.Services;

[TestClass]
public sealed class LauncherSettingsStoreTests
{
    private string _settingsPath = string.Empty;

    [TestInitialize]
    public void TestInitialize()
    {
        _settingsPath = Path.Combine(Path.GetTempPath(), $"winspot-store-{Path.GetRandomFileName()}.json");
    }

    [TestCleanup]
    public void TestCleanup()
    {
        if (File.Exists(_settingsPath))
        {
            File.SetAttributes(_settingsPath, FileAttributes.Normal);
            File.Delete(_settingsPath);
        }
    }

    [TestMethod]
    public void Load_WhenFileMissing_ReturnsDefaultsAndCreatesFile()
    {
        var store = new LauncherSettingsStore(_settingsPath);

        var settings = store.Load();

        Assert.IsTrue(settings.ShowTrayIcon);
        Assert.IsFalse(settings.LaunchOnStartup);
        Assert.AreEqual(ThemeMode.Dark, settings.ThemeMode);
        Assert.AreEqual(MotionProfile.Snappy240, settings.MotionProfile);
        Assert.IsTrue(File.Exists(_settingsPath));
    }

    [TestMethod]
    public void Save_ThenLoad_RoundTripsAllFields()
    {
        var store = new LauncherSettingsStore(_settingsPath);
        store.Save(new LauncherSettings
        {
            Hotkey = new HotkeyBinding { Key = "P", Modifiers = new List<string> { "Win" } },
            LaunchOnStartup = true,
            ShowTrayIcon = false,
            ReduceMotion = true,
            ThemeMode = ThemeMode.Light,
            MotionProfile = MotionProfile.Reduced,
        });

        var reloaded = store.Load();

        Assert.AreEqual("P", reloaded.Hotkey.Key);
        CollectionAssert.AreEquivalent(new List<string> { "Win" }, reloaded.Hotkey.Modifiers);
        Assert.IsTrue(reloaded.LaunchOnStartup);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.IsTrue(reloaded.ReduceMotion);
        Assert.AreEqual(ThemeMode.Light, reloaded.ThemeMode);
        Assert.AreEqual(MotionProfile.Reduced, reloaded.MotionProfile);
    }

    [TestMethod]
    public void Load_WithReservedWindowsHotkey_ReplacesHotkeyWithDefaultAndPreservesOtherSettings()
    {
        var store = new LauncherSettingsStore(_settingsPath);
        store.Save(new LauncherSettings
        {
            Hotkey = new HotkeyBinding { Key = "Space", Modifiers = new List<string> { "Control", "Win" } },
            LaunchOnStartup = true,
            ShowTrayIcon = false,
            ReduceMotion = true,
            ThemeMode = ThemeMode.System,
            MotionProfile = MotionProfile.Reduced,
        });

        var reloaded = store.Load();
        var persisted = new LauncherSettingsStore(_settingsPath).Load();

        Assert.AreEqual("Space", reloaded.Hotkey.Key);
        CollectionAssert.AreEqual(new List<string> { "Control", "Alt" }, reloaded.Hotkey.Modifiers);
        Assert.IsTrue(reloaded.LaunchOnStartup);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.IsTrue(reloaded.ReduceMotion);
        Assert.AreEqual(ThemeMode.System, reloaded.ThemeMode);
        Assert.AreEqual(MotionProfile.Reduced, reloaded.MotionProfile);
        CollectionAssert.AreEqual(new List<string> { "Control", "Alt" }, persisted.Hotkey.Modifiers);
    }

    [TestMethod]
    public void Load_WithNullHotkey_ReplacesHotkeyWithDefaultAndPreservesOtherSettings()
    {
        File.WriteAllText(
            _settingsPath,
            """
            {
              "hotkey": null,
              "launchOnStartup": true,
              "showTrayIcon": false,
              "reduceMotion": true,
              "themeMode": "Light",
              "motionProfile": "Reduced"
            }
            """);
        var store = new LauncherSettingsStore(_settingsPath);

        var reloaded = store.Load();

        Assert.AreEqual("Space", reloaded.Hotkey.Key);
        CollectionAssert.AreEqual(new List<string> { "Control", "Alt" }, reloaded.Hotkey.Modifiers);
        Assert.IsTrue(reloaded.LaunchOnStartup);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.IsTrue(reloaded.ReduceMotion);
        Assert.AreEqual(ThemeMode.Light, reloaded.ThemeMode);
        Assert.AreEqual(MotionProfile.Reduced, reloaded.MotionProfile);
    }

    [TestMethod]
    public void Load_WithMalformedHotkey_ReplacesHotkeyWithDefaultAndPreservesOtherSettings()
    {
        File.WriteAllText(
            _settingsPath,
            """
            {
              "hotkey": {
                "key": "Space",
                "modifiers": null
              },
              "launchOnStartup": true,
              "showTrayIcon": false,
              "reduceMotion": true,
              "themeMode": "System",
              "motionProfile": "Reduced"
            }
            """);
        var store = new LauncherSettingsStore(_settingsPath);

        var reloaded = store.Load();

        Assert.AreEqual("Space", reloaded.Hotkey.Key);
        CollectionAssert.AreEqual(new List<string> { "Control", "Alt" }, reloaded.Hotkey.Modifiers);
        Assert.IsTrue(reloaded.LaunchOnStartup);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.IsTrue(reloaded.ReduceMotion);
        Assert.AreEqual(ThemeMode.System, reloaded.ThemeMode);
        Assert.AreEqual(MotionProfile.Reduced, reloaded.MotionProfile);
    }

    [TestMethod]
    public void Load_WhenHotkeyRepairCannotBePersisted_ReturnsRepairedSettings()
    {
        var store = new LauncherSettingsStore(_settingsPath);
        store.Save(new LauncherSettings
        {
            Hotkey = new HotkeyBinding { Key = "Space", Modifiers = new List<string> { "Win" } },
            LaunchOnStartup = true,
            ShowTrayIcon = false,
            ReduceMotion = true,
            ThemeMode = ThemeMode.Light,
            MotionProfile = MotionProfile.Reduced,
        });
        File.SetAttributes(_settingsPath, FileAttributes.ReadOnly);

        var reloaded = store.Load();

        Assert.AreEqual("Space", reloaded.Hotkey.Key);
        CollectionAssert.AreEqual(new List<string> { "Control", "Alt" }, reloaded.Hotkey.Modifiers);
        Assert.IsTrue(reloaded.LaunchOnStartup);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.IsTrue(reloaded.ReduceMotion);
        Assert.AreEqual(ThemeMode.Light, reloaded.ThemeMode);
        Assert.AreEqual(MotionProfile.Reduced, reloaded.MotionProfile);
    }

    [TestMethod]
    public void ResolveSettingsPath_WhenPortableMarkerExists_UsesLocalDataFolder()
    {
        var root = Path.Combine(Path.GetTempPath(), $"winspot-portable-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            File.WriteAllText(Path.Combine(root, "Winspot.portable"), string.Empty);

            var path = LauncherSettingsStore.ResolveSettingsPath(root, "C:\\Ignored");

            Assert.AreEqual(Path.Combine(root, "data", "settings.json"), path);
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }

    [TestMethod]
    public void ResolvePluginsPath_WhenPortableMarkerExists_UsesExecutablePluginsFolder()
    {
        var root = Path.Combine(Path.GetTempPath(), $"winspot-portable-plugins-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            File.WriteAllText(Path.Combine(root, "Winspot.portable"), string.Empty);

            var path = AppPaths.ResolvePluginsPath(root, "C:\\Ignored");

            Assert.AreEqual(Path.Combine(root, "plugins"), path);
            Assert.IsTrue(AppPaths.IsPortable(root));
        }
        finally
        {
            if (Directory.Exists(root))
            {
                Directory.Delete(root, recursive: true);
            }
        }
    }
}
