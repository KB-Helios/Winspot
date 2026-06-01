using System;
using System.IO;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Winspot_App.Tests.Windowing;

[TestClass]
public sealed class LauncherVisualStyleTests
{
    [TestMethod]
    public void Tokens_UseCalmGraphiteVisualTokens()
    {
        // Colors are centralized in the design-token dictionary, so the calm
        // graphite palette is asserted there rather than inline in the view.
        var tokens = File.ReadAllText(FindTokensAxaml());

        StringAssert.Contains(tokens, "#FF111620");
        StringAssert.Contains(tokens, "#FF1A2230");
        StringAssert.Contains(tokens, "#FFE8EDF7");
        StringAssert.Contains(tokens, "#FFAEB7C6");
        Assert.IsFalse(tokens.Contains("#B8F5D8"), "The Calm Graphite direction should avoid bright teal as a dominant UI accent.");
        Assert.IsFalse(tokens.Contains("#285E55"), "Selected rows should use neutral graphite, not green/teal selection blocks.");
    }

    [TestMethod]
    public void MainWindow_ReferencesCentralizedTokens()
    {
        var axaml = File.ReadAllText(FindMainWindowAxaml());

        StringAssert.Contains(axaml, "{DynamicResource TextPrimaryBrush}");
        StringAssert.Contains(axaml, "{DynamicResource SelectionBrush}");
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

    [TestMethod]
    public void Tokens_RemoveFocusedTextBoxGlow()
    {
        var tokens = File.ReadAllText(FindTokensAxaml());

        StringAssert.Contains(tokens, "<SolidColorBrush x:Key=\"TextControlBorderBrushFocused\" Color=\"Transparent\" />");
        Assert.IsFalse(tokens.Contains("TextControlBorderBrushFocused\" Color=\"#40FFFFFF\""), "Focused search text box should not draw a white glow.");
    }

    private static string FindMainWindowAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "MainWindow.axaml"));

    private static string FindTokensAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Themes", "Tokens.axaml"));

    private static string FindRepoFile(string relativePath)
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null)
        {
            var path = Path.Combine(directory.FullName, relativePath);
            if (File.Exists(path))
            {
                return path;
            }

            directory = directory.Parent;
        }

        Assert.Fail($"Could not locate {relativePath} from the test output directory.");
        return string.Empty;
    }
}
