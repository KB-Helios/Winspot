using System;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App.Models;

namespace Winspot_App.Tests.Models;

[TestClass]
public sealed class PluginValidationDiagnosticsTests
{
    [TestMethod]
    public void Summary_WithNoIssues_ReportsReadyCounts()
    {
        var report = new PluginValidationReport(new[]
        {
            new PluginValidationEntry("calculator", "Calculator", null, "BuiltIn", "Accepted", true, Array.Empty<PluginValidationIssue>()),
            new PluginValidationEntry("notes", "Notes", "C:\\Plugins\\notes.json", "User", "Accepted", false, Array.Empty<PluginValidationIssue>()),
            new PluginValidationEntry("off", "Off", "C:\\Plugins\\off.json", "User", "Disabled", false, Array.Empty<PluginValidationIssue>()),
        });

        Assert.AreEqual(2, report.AcceptedCount);
        Assert.AreEqual(1, report.DisabledCount);
        Assert.AreEqual(0, report.RejectedCount);
        Assert.AreEqual(0, report.WarningCount);
        Assert.AreEqual(0, report.ErrorCount);
        Assert.AreEqual("Ready", report.Health);
        Assert.AreEqual("2 accepted, 1 disabled, 0 rejected", report.CountSummary);
    }

    [TestMethod]
    public void Summary_WithIssues_ReportsHighestSeverity()
    {
        var report = new PluginValidationReport(new[]
        {
            new PluginValidationEntry(
                "bad",
                "Bad",
                "C:\\Plugins\\bad.json",
                "User",
                "Rejected",
                false,
                new[]
                {
                    new PluginValidationIssue("Error", "Manifest", "invalid_manifest", "plugin id is invalid"),
                    new PluginValidationIssue("Warning", "Policy", "ignored_user_executable_capability", "capability ignored"),
                }),
        });

        Assert.AreEqual(1, report.RejectedCount);
        Assert.AreEqual(1, report.WarningCount);
        Assert.AreEqual(1, report.ErrorCount);
        Assert.AreEqual("Error", report.Health);
        Assert.AreEqual(2, report.IssueCount);
    }

    [TestMethod]
    public void Entry_WhenFieldsMissing_UsesPathOrUnknownLabel()
    {
        var pathEntry = new PluginValidationEntry(null, null, "C:\\Plugins\\broken.json", "User", "Rejected", false, Array.Empty<PluginValidationIssue>());
        var unknownEntry = new PluginValidationEntry(null, null, null, "User", "Rejected", false, Array.Empty<PluginValidationIssue>());

        Assert.AreEqual("broken.json", pathEntry.DisplayName);
        Assert.AreEqual("<unknown plugin>", unknownEntry.DisplayName);
    }
}
