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
    public void AppAxaml_WhenConfigured_UsesSimpleThemeBase()
    {
        var axaml = File.ReadAllText(FindAppAxaml());

        StringAssert.Contains(axaml, "<SimpleTheme />");
        Assert.IsFalse(axaml.Contains("<FluentTheme />"), "Winspot should use the lightweight Avalonia SimpleTheme base.");
    }

    [TestMethod]
    public void SettingsWindow_WhenRendered_ContainsCompactTabbedSections()
    {
        var axaml = File.ReadAllText(FindSettingsWindowAxaml());

        StringAssert.Contains(axaml, "TabControl");
        foreach (var section in new[] { "General", "Hotkey", "Appearance", "Plugins", "Diagnostics", "About" })
        {
            StringAssert.Contains(axaml, $"Header=\"{section}\"");
        }

        StringAssert.Contains(axaml, "PluginValidationSummary");
        StringAssert.Contains(axaml, "PluginValidationEntries");
        StringAssert.Contains(axaml, "OnValidatePluginsClick");
        StringAssert.Contains(axaml, "ShowOnlyPluginValidationIssues");
        StringAssert.Contains(axaml, "ScrollViewer");
        StringAssert.Contains(axaml, "AutomationProperties.Name=\"Open plugins folder\"");

        Assert.IsFalse(axaml.Contains("CornerRadius=\"12\""), "Settings surfaces should stay at 8px radius or lower.");
        Assert.IsFalse(axaml.Contains("<Setter Property=\"CornerRadius\" Value=\"12\""), "Settings card style should not use a 12px radius.");
    }

    [TestMethod]
    public void Tokens_ExposeAccentGradientForBrandedControls()
    {
        var tokens = File.ReadAllText(FindTokensAxaml());

        StringAssert.Contains(tokens, "x:Key=\"AccentGradientBrush\"");
        StringAssert.Contains(tokens, "x:Key=\"FocusRingBrush\"");
        StringAssert.Contains(tokens, "x:Key=\"ControlBackgroundBrush\"");
    }

    [TestMethod]
    public void Controls_WhenLayered_RefineSimpleThemeWithoutReplacingIt()
    {
        var app = File.ReadAllText(FindAppAxaml());
        var controls = File.ReadAllText(FindControlsAxaml());

        // The shared sheet must be layered after (not instead of) SimpleTheme.
        StringAssert.Contains(app, "<SimpleTheme />");
        StringAssert.Contains(app, "Themes/Controls.axaml");
        StringAssert.Contains(controls, "Selector=\"Button.accent\"");
        StringAssert.Contains(controls, "Selector=\"TabItem:selected");
    }

    [TestMethod]
    public void SearchSpinner_ControlAndAnimatedAssetExist()
    {
        var spinner = File.ReadAllText(FindSearchSpinnerAxaml());
        var icon = File.ReadAllText(FindSearchLoadingIcon());

        // The in-app control rotates continuously without a hardcoded frame cap.
        StringAssert.Contains(spinner, "IterationCount=\"Infinite\"");
        StringAssert.Contains(spinner, "RotateTransform.Angle");
        Assert.IsFalse(spinner.Contains("Task.Delay"), "The spinner must not throttle motion with a timer.");

        // The deliverable SVG is genuinely animated.
        StringAssert.Contains(icon, "animateTransform");
        StringAssert.Contains(icon, "repeatCount=\"indefinite\"");
    }

    [TestMethod]
    public void MainWindow_WhenSearching_ShowsLoadingSpinner()
    {
        var axaml = File.ReadAllText(FindMainWindowAxaml());

        StringAssert.Contains(axaml, "controls:SearchSpinner");
        StringAssert.Contains(axaml, "IsVisible=\"{Binding IsSearching}\"");
    }

    [TestMethod]
    public void MainWindowCode_WhenInspected_DoesNotThrottleResizeAnimationToSixtyHertz()
    {
        var code = File.ReadAllText(FindMainWindowCodeBehind());

        Assert.IsFalse(code.Contains("Task.Delay(16"), "Snappy motion must not be capped by a hardcoded 16ms timer.");
        Assert.IsFalse(code.Contains("_boundsAnimationCancellation"), "Removed bounds animation should not leave cancellation cleanup behind.");
    }

    [TestMethod]
    public void AppCode_WhenTrayIconLoaded_UsesDedicatedTrayIcon()
    {
        var code = File.ReadAllText(FindAppCodeBehind());
        var project = File.ReadAllText(FindAppProject());

        StringAssert.Contains(code, "WinspotTrayIcon.ico");
        StringAssert.Contains(project, "Assets\\WinspotTrayIcon.ico");
    }

    [TestMethod]
    public void SettingsViewModel_WhenRenderingStrings_UsesSettingsDisplayStrings()
    {
        var viewModel = File.ReadAllText(FindSettingsViewModel());
        var strings = File.ReadAllText(FindSettingsDisplayStrings());
        var model = File.ReadAllText(FindPluginValidationDiagnosticsModel());

        StringAssert.Contains(viewModel, "SettingsDisplayStrings");
        StringAssert.Contains(strings, "UserPluginManifestCountSingularFormat");
        StringAssert.Contains(strings, "DiagnosticsFormat");
        Assert.IsFalse(viewModel.Contains("user manifest{"), "Pluralized user-facing strings should live in SettingsDisplayStrings.");
        Assert.IsFalse(viewModel.Contains("Could not open plugins folder."), "User-facing status strings should live in SettingsDisplayStrings.");
        Assert.IsFalse(model.Contains("\"Ready\""), "Plugin diagnostics models should expose health codes, not display text.");
        Assert.IsFalse(model.Contains("\"Trusted\""), "Plugin diagnostics models should not own Settings display labels.");
        Assert.IsFalse(model.Contains("<unknown plugin>"), "Unknown plugin display text should live in SettingsDisplayStrings.");
        Assert.IsFalse(model.Contains("DisplayText"), "Formatted issue text should live outside the diagnostics model.");
    }

    [TestMethod]
    public void SettingsWindowCode_WhenOpened_RefreshesPluginValidation()
    {
        var code = File.ReadAllText(FindSettingsWindowCodeBehind());
        var openedBody = ExtractMethodBody(code, "OnOpened");

        StringAssert.Contains(code, "Opened += OnOpened");
        StringAssert.Contains(openedBody, "RefreshPluginValidationAsync");
    }

    [TestMethod]
    public void SettingsWindowCode_WhenRefreshingPluginValidation_UsesWindowCancellationToken()
    {
        var code = File.ReadAllText(FindSettingsWindowCodeBehind());

        StringAssert.Contains(code, "CancellationTokenSource");
        StringAssert.Contains(code, "Closed += OnClosed");
        StringAssert.Contains(code, "_pluginValidationCancellation");
    }

    private static string FindMainWindowAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "MainWindow.axaml"));

    private static string FindMainWindowCodeBehind() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "MainWindow.axaml.cs"));

    private static string FindSettingsWindowAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "SettingsWindow.axaml"));

    private static string FindSettingsWindowCodeBehind() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "SettingsWindow.axaml.cs"));

    private static string FindAppAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "App.axaml"));

    private static string FindAppCodeBehind() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "App.axaml.cs"));

    private static string FindAppProject() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Winspot.App.csproj"));

    private static string FindSettingsViewModel() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "ViewModels", "SettingsViewModel.cs"));

    private static string FindSettingsDisplayStrings() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Strings", "SettingsDisplayStrings.cs"));

    private static string FindPluginValidationDiagnosticsModel() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Models", "PluginValidationDiagnostics.cs"));

    private static string FindTokensAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Themes", "Tokens.axaml"));

    private static string FindControlsAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Themes", "Controls.axaml"));

    private static string FindSearchSpinnerAxaml() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Controls", "SearchSpinner.axaml"));

    private static string FindSearchLoadingIcon() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Assets", "Icons", "search-loading.svg"));

    private static string ExtractMethodBody(string code, string methodName)
    {
        var methodIndex = code.IndexOf($"void {methodName}", StringComparison.Ordinal);
        Assert.IsTrue(methodIndex >= 0, $"Could not find {methodName}.");
        var openBraceIndex = code.IndexOf('{', methodIndex);
        Assert.IsTrue(openBraceIndex >= 0, $"Could not find body for {methodName}.");

        var depth = 0;
        for (var index = openBraceIndex; index < code.Length; index++)
        {
            if (code[index] == '{')
            {
                depth++;
            }
            else if (code[index] == '}')
            {
                depth--;
                if (depth == 0)
                {
                    return code.Substring(openBraceIndex, index - openBraceIndex + 1);
                }
            }
        }

        Assert.Fail($"Could not parse body for {methodName}.");
        return string.Empty;
    }

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
