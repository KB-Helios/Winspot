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
        });

        var reloaded = store.Load();

        Assert.AreEqual("P", reloaded.Hotkey.Key);
        CollectionAssert.AreEquivalent(new List<string> { "Win" }, reloaded.Hotkey.Modifiers);
        Assert.IsTrue(reloaded.LaunchOnStartup);
        Assert.IsFalse(reloaded.ShowTrayIcon);
        Assert.IsTrue(reloaded.ReduceMotion);
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
}
