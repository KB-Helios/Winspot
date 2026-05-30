using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Xml.Linq;

using Avalonia.Media;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App;

namespace Winspot_App.Tests.Windowing;

[TestClass]
public sealed class LauncherVisualStyleTests
{
    private static readonly XNamespace AxamlNamespace = "https://github.com/avaloniaui";
    private static readonly XNamespace XamlNamespace = "http://schemas.microsoft.com/winfx/2006/xaml";

    [TestMethod]
    public void MainWindow_UsesCalmGraphiteVisualTokens()
    {
        var document = LoadMainWindowAxaml();
        var colors = GetAttributeColors(document).ToArray();

        CollectionAssert.Contains(colors, Color.Parse("#FF111620"));
        CollectionAssert.Contains(colors, Color.Parse("#FF1A2230"));
        CollectionAssert.Contains(colors, Color.Parse("#FFE8EDF7"));
        CollectionAssert.Contains(colors, Color.Parse("#FFAEB7C6"));
        CollectionAssert.DoesNotContain(colors, Color.Parse("#B8F5D8"), "The Calm Graphite direction should avoid bright teal as a dominant UI accent.");
        CollectionAssert.DoesNotContain(colors, Color.Parse("#285E55"), "Selected rows should use neutral graphite, not green/teal selection blocks.");
    }

    [TestMethod]
    public void MainWindow_CompactSearchIsSingleSurface()
    {
        var document = LoadMainWindowAxaml();
        var window = document.Root;
        Assert.IsNotNull(window);

        Assert.AreEqual(LauncherWindowLayout.CompactHeight.ToString(CultureInfo.InvariantCulture), window!.Attribute("Height")?.Value);
        Assert.AreEqual(LauncherWindowLayout.CompactHeight.ToString(CultureInfo.InvariantCulture), window.Attribute("MinHeight")?.Value);
        Assert.AreEqual("Transparent", window.Attribute("TransparencyLevelHint")?.Value);

        var searchShellStyle = FindStyle(document, "Border.search-shell");
        Assert.AreEqual("Transparent", FindSetter(searchShellStyle, "Background").Attribute("Value")?.Value);
        Assert.AreEqual("0", FindSetter(searchShellStyle, "BorderThickness").Attribute("Value")?.Value);
        Assert.AreEqual("18,0", FindSetter(searchShellStyle, "Padding").Attribute("Value")?.Value);

        var resultsList = FindElement(document, "ListBox", "WinspotResultsList");
        Assert.AreEqual("Disabled", resultsList.Attribute("ScrollViewer.HorizontalScrollBarVisibility")?.Value);

        var surface = FindElement(document, "Border", "SpotlightSurface");
        Assert.AreEqual(LauncherWindowLayout.ShadowMargin.ToString(CultureInfo.InvariantCulture), surface.Attribute("Margin")?.Value);
        AssertShadowFitsInsideWindowMargin(surface.Attribute("BoxShadow")?.Value, LauncherWindowLayout.ShadowMargin);
    }

    private static XDocument LoadMainWindowAxaml()
    {
        return XDocument.Load(FindMainWindowAxaml());
    }

    private static IEnumerable<Color> GetAttributeColors(XDocument document)
    {
        return document
            .Descendants()
            .Attributes()
            .Select(attribute => attribute.Value)
            .Where(value => value.StartsWith('#'))
            .Select(Color.Parse);
    }

    private static XElement FindElement(XDocument document, string elementName, string automationOrXamlName)
    {
        var element = document
            .Descendants(AxamlNamespace + elementName)
            .SingleOrDefault(candidate =>
                candidate.Attribute(XamlNamespace + "Name")?.Value == automationOrXamlName ||
                candidate.Attribute("AutomationProperties.AutomationId")?.Value == automationOrXamlName);

        Assert.IsNotNull(element, $"Expected to find {elementName} named {automationOrXamlName}.");
        return element!;
    }

    private static XElement FindStyle(XDocument document, string selector)
    {
        var style = document
            .Descendants(AxamlNamespace + "Style")
            .SingleOrDefault(candidate => candidate.Attribute("Selector")?.Value == selector);

        Assert.IsNotNull(style, $"Expected to find style selector {selector}.");
        return style!;
    }

    private static XElement FindSetter(XElement style, string property)
    {
        var setter = style
            .Elements(AxamlNamespace + "Setter")
            .SingleOrDefault(candidate => candidate.Attribute("Property")?.Value == property);

        Assert.IsNotNull(setter, $"Expected to find setter for {property}.");
        return setter!;
    }

    private static void AssertShadowFitsInsideWindowMargin(string? boxShadow, double margin)
    {
        Assert.IsFalse(string.IsNullOrWhiteSpace(boxShadow), "Expected the spotlight surface to define a shadow.");

        var parts = boxShadow!
            .Split(' ', StringSplitOptions.RemoveEmptyEntries)
            .Take(4)
            .Select(value => double.Parse(value, CultureInfo.InvariantCulture))
            .ToArray();

        Assert.AreEqual(4, parts.Length, "Expected BoxShadow to define offset-x, offset-y, blur, and spread.");
        var shadowExtent = Math.Abs(parts[1]) + parts[2] + Math.Max(parts[3], 0);
        Assert.IsTrue(
            margin >= shadowExtent,
            $"Expected the {margin} DIP window margin to contain the {shadowExtent} DIP shadow extent.");
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
