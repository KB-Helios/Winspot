using System;
using System.IO;
using System.Linq;
using System.Xml.Linq;

using Microsoft.VisualStudio.TestTools.UnitTesting;

namespace Winspot_App.Tests.Build;

[TestClass]
public sealed class WinspotAppProjectTests
{
    [TestMethod]
    public void CopyWinspotDaemonTargets_WhenDaemonIsMissing_BuildsBeforeCopyCondition()
    {
        var project = XDocument.Load(FindAppProjectFile());

        AssertCopyTargetBuildsBeforeCheckingDaemonExists(project, "CopyWinspotDaemonToOutput");
        AssertCopyTargetBuildsBeforeCheckingDaemonExists(project, "CopyWinspotDaemonToPublish");
    }

    private static void AssertCopyTargetBuildsBeforeCheckingDaemonExists(XDocument project, string targetName)
    {
        var target = project
            .Descendants("Target")
            .Single(element => (string?)element.Attribute("Name") == targetName);

        var targetCondition = (string?)target.Attribute("Condition") ?? string.Empty;
        Assert.IsFalse(
            targetCondition.Contains("WinspotDaemonExe", StringComparison.Ordinal),
            $"{targetName} must not be skipped before BuildWinspotDaemon can run.");

        var copy = target.Elements("Copy").Single();
        var copyCondition = (string?)copy.Attribute("Condition") ?? string.Empty;
        StringAssert.Contains(copyCondition, "WinspotDaemonExe");
    }

    private static string FindAppProjectFile() =>
        FindRepoFile(Path.Combine("apps", "Winspot.App", "Winspot.App.csproj"));

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
