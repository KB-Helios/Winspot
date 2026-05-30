using System;
using System.Reflection;

using Avalonia;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Winspot_App.Tests.Windowing;

[TestClass]
public sealed class LauncherWindowLayoutTests
{
    [TestMethod]
    public void CalculateBounds_WhenExpandedOnLargeScreen_UsesMaximumSpotlightSizeAboveCenter()
    {
        var bounds = CalculateBounds(new Rect(0, 0, 1200, 900), isExpanded: true);

        Assert.AreEqual(760, bounds.Width);
        Assert.AreEqual(560, bounds.Height);
        Assert.AreEqual(220, bounds.X);
        Assert.AreEqual(116, bounds.Y);
    }

    [TestMethod]
    public void CalculateBounds_WhenCompactOnNarrowScreen_LeavesHorizontalBreathingRoom()
    {
        var bounds = CalculateBounds(new Rect(0, 0, 500, 400), isExpanded: false);

        Assert.AreEqual(452, bounds.Width);
        Assert.AreEqual(76, bounds.Height);
        Assert.AreEqual(24, bounds.X);
        Assert.AreEqual(138, bounds.Y);
    }

    private static Rect CalculateBounds(Rect workingArea, bool isExpanded)
    {
        var type = Type.GetType("Winspot_App.LauncherWindowLayout, Winspot.App", throwOnError: false);
        Assert.IsNotNull(type, "Expected an internal LauncherWindowLayout helper in Winspot.App.");

        var method = type!.GetMethod("CalculateBounds", BindingFlags.Static | BindingFlags.Public | BindingFlags.NonPublic);
        Assert.IsNotNull(method, "Expected LauncherWindowLayout.CalculateBounds(Rect, bool).");

        var result = method!.Invoke(null, new object[] { workingArea, isExpanded });
        Assert.IsInstanceOfType<Rect>(result);
        return (Rect)result!;
    }
}
