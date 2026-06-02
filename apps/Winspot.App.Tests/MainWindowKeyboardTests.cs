using Avalonia.Input;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App;

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
}
