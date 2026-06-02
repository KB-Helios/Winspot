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
    public void Tokens_WhenFocusedTextBoxSelected_RemovesWhiteGlow()
    {
        var tokens = File.ReadAllText(FindTokensAxaml());

        StringAssert.Contains(tokens, "<SolidColorBrush x:Key=\"TextControlBorderBrushFocused\" Color=\"Transparent\" />");
        Assert.IsFalse(tokens.Contains("TextControlBorderBrushFocused\" Color=\"#40FFFFFF\""), "Focused search text box should not draw a white glow.");
    }

    [TestMethod]
    public void App_UsesSimpleThemeBaseForAvalonia12Performance()
    {
        var axaml = File.ReadAllText(FindAppAxaml());

        StringAssert.Contains(axaml, "<SimpleTheme />");
        Assert.IsFalse(axaml.Contains("<FluentTheme />"), "Winspot should use the lightweight Avalonia SimpleTheme base.");
    }

    [TestMethod]
    public void SettingsWindow_ContainsCompactTabbedSections()
    {
        var axaml = File.ReadAllText(FindSettingsWindowAxaml());

        StringAssert.Contains(axaml, "TabControl");
        foreach (var section in new[] { "General", "Hotkey", "Appearance", "Plugins", "Diagnostics", "About" })
        {
            StringAssert.Contains(axaml, $"Header=\"{section}\"");
        }

        Assert.IsFalse(axaml.Contains("CornerRadius=\"12\""), "Settings surfaces should stay at 8px radius or lower.");
        Assert.IsFalse(axaml.Contains("<Setter Property=\"CornerRadius\" Value=\"12\""), "Settings card style should not use a 12px radius.");
    }

    [TestMethod]
    public void MainWindow_DoesNotThrottleResizeAnimationToSixtyHertz()
    {
        var code = File.ReadAllText(FindMainWindowCodeBehind());

        Assert.IsFalse(code.Contains("Task.Delay(16"), "Snappy motion must not be capped by a hardcoded 16ms timer.");
    }

    [TestMethod]
    public void App_LoadsDedicatedTrayIcon()
    {
        var code = File.ReadAllText(FindAppCodeBehind());
        var project = File.ReadAllText(FindAppProject());

        StringAssert.Contains(code, "WinspotTrayIcon.ico");
        StringAssert.Contains(project, "Assets\\WinspotTrayIcon.ico");
    }

    private static string FindMainWindowAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "MainWindow.axaml"));

    private static string FindMainWindowCodeBehind() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "MainWindow.axaml.cs"));

    private static string FindSettingsWindowAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "SettingsWindow.axaml"));

    private static string FindAppAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "App.axaml"));

    private static string FindAppCodeBehind() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "App.axaml.cs"));

    private static string FindAppProject() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Winspot.App.csproj"));

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
