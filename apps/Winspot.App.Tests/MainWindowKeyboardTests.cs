using Avalonia.Input;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App;
using Winspot_App.Models;

namespace Winspot_App.Tests;

[TestClass]
public sealed class MainWindowKeyboardTests
{
    [TestMethod]
    public void ShouldHandleActionNavigationKey_WhenActionStripNotFocused_DoesNotHandleArrowKeys()
    {
        Assert.IsFalse(MainWindow.ShouldHandleActionNavigationKey(Key.Left, -1));
        Assert.IsFalse(MainWindow.ShouldHandleActionNavigationKey(Key.Right, -1));
    }

    [TestMethod]
    public void ShouldHandleActionNavigationKey_WhenActionStripFocused_HandlesArrowKeys()
    {
        Assert.IsTrue(MainWindow.ShouldHandleActionNavigationKey(Key.Left, 0));
        Assert.IsTrue(MainWindow.ShouldHandleActionNavigationKey(Key.Right, 0));
    }

    [TestMethod]
    public void ShouldHideForHotkey_WhenVisibleButNotActive_StillClosesLauncher()
    {
        Assert.IsTrue(MainWindow.ShouldHideForHotkey(isVisible: true, isActive: false));
    }

    [TestMethod]
    public void ShouldHideBeforeExecutingResult_OnlyPreHidesCaptureActions()
    {
        var capture = new SearchResultItem(
            "plugin:windows-capture:screenshot:monitor:primary",
            "Screenshot primary monitor",
            "Save a PNG capture",
            "Plugin",
            1,
            "PluginCommand",
            Source: "windows-capture");
        var captureIdOnly = new SearchResultItem(
            "plugin:windows-capture:record:window:foreground:8",
            "Record foreground window",
            "Save a video capture",
            "Plugin",
            1,
            "PluginCommand",
            Source: "other-source");
        var captureSourceOnly = new SearchResultItem(
            "plugin:different-prefix:action",
            "Different action",
            "Different description",
            "Plugin",
            1,
            "PluginCommand",
            Source: "windows-capture");
        var fastFlowLm = new SearchResultItem(
            "plugin:fastflowlm",
            "Ask FastFlowLM",
            "Answer in preview",
            "Plugin",
            1,
            "PluginCommand",
            Source: "fastflowlm");

        Assert.IsTrue(MainWindow.ShouldHideBeforeExecutingResult(capture));
        Assert.IsTrue(MainWindow.ShouldHideBeforeExecutingResult(captureIdOnly));
        Assert.IsTrue(MainWindow.ShouldHideBeforeExecutingResult(captureSourceOnly));
        Assert.IsFalse(MainWindow.ShouldHideBeforeExecutingResult(fastFlowLm));
        Assert.IsFalse(MainWindow.ShouldHideBeforeExecutingResult(null));
    }
}
