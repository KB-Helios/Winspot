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
        Assert.IsTrue(settings.FastFlowLm.Enabled);
        Assert.AreEqual("gemma4-it:e2b", settings.FastFlowLm.ModelTag);
        Assert.AreEqual("flm", settings.FastFlowLm.ExecutablePath);
        Assert.AreEqual(52625, settings.FastFlowLm.Port);
        Assert.AreEqual(120, settings.FastFlowLm.IdleTimeoutSeconds);
        Assert.AreEqual(5, settings.FastFlowLm.MaxContextFiles);
        Assert.AreEqual(1024 * 1024, settings.FastFlowLm.MaxFileBytes);
        Assert.AreEqual(4 * 1024 * 1024, settings.FastFlowLm.MaxContextBytes);
        Assert.IsTrue(settings.Capture.Enabled);
        Assert.AreEqual(string.Empty, settings.Capture.OutputDirectory);
        Assert.AreEqual(8, settings.Capture.DefaultRecordSeconds);
        Assert.AreEqual(60, settings.Capture.MaxRecordSeconds);
        Assert.IsTrue(settings.Capture.IncludeCursor);
        Assert.AreEqual(250, settings.Capture.PreCaptureDelayMs);
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
            FastFlowLm = new FastFlowLmSettings
            {
                Enabled = false,
                ModelTag = "qwen3:4b",
                ExecutablePath = "custom-flm",
                Port = 52626,
                IdleTimeoutSeconds = 30,
                MaxContextFiles = 3,
                MaxFileBytes = 123,
                MaxContextBytes = 456,
            },
            Capture = new CaptureSettings
            {
                Enabled = false,
                OutputDirectory = @"C:\Captures",
                DefaultRecordSeconds = 5,
                MaxRecordSeconds = 20,
                IncludeCursor = false,
                PreCaptureDelayMs = 750,
            },
        });

        var reloaded = store.Load();

        Assert.AreEqual("P", reloaded.Hotkey.Key);
        CollectionAssert.AreEquivalent(new List<string> { "Win" }, reloaded.Hotkey.Modifiers);
        Assert.IsTrue(reloaded.LaunchOnStartup);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.IsTrue(reloaded.ReduceMotion);
        Assert.AreEqual(ThemeMode.Light, reloaded.ThemeMode);
        Assert.AreEqual(MotionProfile.Reduced, reloaded.MotionProfile);
        Assert.IsFalse(reloaded.FastFlowLm.Enabled);
        Assert.AreEqual("qwen3:4b", reloaded.FastFlowLm.ModelTag);
        Assert.AreEqual("custom-flm", reloaded.FastFlowLm.ExecutablePath);
        Assert.AreEqual(52626, reloaded.FastFlowLm.Port);
        Assert.AreEqual(30, reloaded.FastFlowLm.IdleTimeoutSeconds);
        Assert.AreEqual(3, reloaded.FastFlowLm.MaxContextFiles);
        Assert.AreEqual(123, reloaded.FastFlowLm.MaxFileBytes);
        Assert.AreEqual(456, reloaded.FastFlowLm.MaxContextBytes);
        Assert.IsFalse(reloaded.Capture.Enabled);
        Assert.AreEqual(@"C:\Captures", reloaded.Capture.OutputDirectory);
        Assert.AreEqual(5, reloaded.Capture.DefaultRecordSeconds);
        Assert.AreEqual(20, reloaded.Capture.MaxRecordSeconds);
        Assert.IsFalse(reloaded.Capture.IncludeCursor);
        Assert.AreEqual(750, reloaded.Capture.PreCaptureDelayMs);
    }

    [TestMethod]
    public void Load_WithMalformedCaptureSettings_NormalizesToSafeDefaults()
    {
        File.WriteAllText(
            _settingsPath,
            """
            {
              "hotkey": {
                "key": "K",
                "modifiers": ["Control", "Shift"]
              },
              "capture": {
                "enabled": true,
                "outputDirectory": " C:\\Captures ",
                "defaultRecordSeconds": 120,
                "maxRecordSeconds": 0,
                "includeCursor": false,
                "preCaptureDelayMs": 10000
              }
            }
            """);
        var store = new LauncherSettingsStore(_settingsPath);

        var reloaded = store.Load();

        Assert.IsTrue(reloaded.Capture.Enabled);
        Assert.AreEqual(@"C:\Captures", reloaded.Capture.OutputDirectory);
        Assert.AreEqual(60, reloaded.Capture.DefaultRecordSeconds);
        Assert.AreEqual(60, reloaded.Capture.MaxRecordSeconds);
        Assert.IsFalse(reloaded.Capture.IncludeCursor);
        Assert.AreEqual(5000, reloaded.Capture.PreCaptureDelayMs);
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
    public void ResolveSettingsPath_WhenLocalAppDataMissing_UsesSpecialFolder()
    {
        var originalUserProfile = Environment.GetEnvironmentVariable("USERPROFILE");
        Environment.SetEnvironmentVariable("USERPROFILE", @"X:\RedirectedElsewhere");
        try
        {
            var path = LauncherSettingsStore.ResolveSettingsPath("C:\\NotPortable", null);

            Assert.AreEqual(
                Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Winspot", "settings.json"),
                path);
        }
        finally
        {
            Environment.SetEnvironmentVariable("USERPROFILE", originalUserProfile);
        }
    }

    [TestMethod]
    public void Load_WithUnknownEnumStrings_PreservesValidFieldsAndPersistsRepairedEnums()
    {
        File.WriteAllText(
            _settingsPath,
            """
            {
              "hotkey": {
                "key": "K",
                "modifiers": ["Control", "Shift"]
              },
              "launchOnStartup": true,
              "showTrayIcon": false,
              "reduceMotion": false,
              "themeMode": "Neon",
              "motionProfile": "WarpSpeed"
            }
            """);
        var store = new LauncherSettingsStore(_settingsPath);

        var reloaded = store.Load();
        var persistedJson = File.ReadAllText(_settingsPath);

        Assert.AreEqual("K", reloaded.Hotkey.Key);
        CollectionAssert.AreEquivalent(new List<string> { "Control", "Shift" }, reloaded.Hotkey.Modifiers);
        Assert.IsTrue(reloaded.LaunchOnStartup);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.AreEqual(ThemeMode.Dark, reloaded.ThemeMode);
        Assert.AreEqual(MotionProfile.Snappy240, reloaded.MotionProfile);
        StringAssert.Contains(persistedJson, "\"themeMode\": \"Dark\"");
        StringAssert.Contains(persistedJson, "\"motionProfile\": \"Snappy240\"");
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

    [TestMethod]
    public void ResolveLogPath_WhenPortableMarkerExists_UsesExecutableDataLogsFolder()
    {
        var root = Path.Combine(Path.GetTempPath(), $"winspot-portable-logs-{Guid.NewGuid():N}");
        Directory.CreateDirectory(root);
        try
        {
            File.WriteAllText(Path.Combine(root, "Winspot.portable"), string.Empty);

            var path = AppPaths.ResolveLogPath(root, "C:\\Ignored");

            Assert.AreEqual(Path.Combine(root, "data", "logs", "app.log"), path);
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
    public void ResolveLogPath_WhenNotPortable_UsesLocalAppDataLogsFolder()
    {
        var path = AppPaths.ResolveLogPath("C:\\NotPortable", "C:\\Local");

        Assert.AreEqual(Path.Combine("C:\\Local", "Winspot", "logs", "app.log"), path);
    }
}
