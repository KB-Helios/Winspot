using System;
using System.IO;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Winspot_App.Tests.Windowing;

[TestClass]
public sealed class LauncherVisualStyleTests
{
    [TestMethod]
    public void MainWindow_UsesCalmGraphiteVisualTokens()
    {
        var axaml = File.ReadAllText(FindMainWindowAxaml());

        StringAssert.Contains(axaml, "#FF111620");
        StringAssert.Contains(axaml, "#FF1A2230");
        StringAssert.Contains(axaml, "#FFE8EDF7");
        StringAssert.Contains(axaml, "#FFAEB7C6");
        Assert.IsFalse(axaml.Contains("#B8F5D8"), "The Calm Graphite direction should avoid bright teal as a dominant UI accent.");
        Assert.IsFalse(axaml.Contains("#285E55"), "Selected rows should use neutral graphite, not green/teal selection blocks.");
    }

    [TestMethod]
    public void MainWindow_CompactSearchIsSingleSurface()
    {
        var axaml = File.ReadAllText(FindMainWindowAxaml());

        StringAssert.Contains(axaml, "Height=\"76\"");
        StringAssert.Contains(axaml, "MinHeight=\"76\"");
        StringAssert.Contains(axaml, "<Setter Property=\"Padding\" Value=\"18,0\" />");
        Assert.IsFalse(axaml.Contains("<Setter Property=\"Background\" Value=\"#FF151C28\" />"), "Compact search should not render as a second filled pill inside the spotlight surface.");
        Assert.IsFalse(axaml.Contains("<Setter Property=\"BorderBrush\" Value=\"#1FFFFFFF\" />"), "Compact search should not have a second border inside the spotlight surface.");
    }

    private static string FindMainWindowAxaml()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null)
        {
            var path = Path.Combine(directory.FullName, "apps", "Winspot.App", "MainWindow.axaml");
            if (File.Exists(path))
            {
                return path;
            }

            directory = directory.Parent;
        }

        Assert.Fail("Could not locate apps/Winspot.App/MainWindow.axaml from the test output directory.");
        return string.Empty;
    }
}
