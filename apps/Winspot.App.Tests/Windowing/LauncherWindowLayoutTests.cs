using Avalonia;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App;

namespace Winspot_App.Tests.Windowing;

[TestClass]
public sealed class LauncherWindowLayoutTests
{
    [TestMethod]
    public void CalculateBounds_WhenExpandedOnLargeScreen_IncludesShadowMarginAroundMaximumSurface()
    {
        var bounds = LauncherWindowLayout.CalculateBounds(new Rect(0, 0, 1200, 900), isExpanded: true);

        Assert.AreEqual(888, bounds.Width);
        Assert.AreEqual(688, bounds.Height);
        Assert.AreEqual(156, bounds.X);
        Assert.AreEqual(52, bounds.Y);
        Assert.AreEqual(760, bounds.Width - (LauncherWindowLayout.ShadowMargin * 2));
        Assert.AreEqual(560, bounds.Height - (LauncherWindowLayout.ShadowMargin * 2));
    }

    [TestMethod]
    public void CalculateBounds_WhenCompactOnNarrowScreen_FitsWindowAndPreservesShadowMargin()
    {
        var bounds = LauncherWindowLayout.CalculateBounds(new Rect(0, 0, 500, 400), isExpanded: false);

        Assert.AreEqual(452, bounds.Width);
        Assert.AreEqual(188, bounds.Height);
        Assert.AreEqual(24, bounds.X);
        Assert.AreEqual(82, bounds.Y);
        Assert.AreEqual(324, bounds.Width - (LauncherWindowLayout.ShadowMargin * 2));
        Assert.AreEqual(60, bounds.Height - (LauncherWindowLayout.ShadowMargin * 2));
    }
}
