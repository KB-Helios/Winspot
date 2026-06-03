namespace Winspot_App.Models;

public enum PluginValidationHealth
{
    Ready,
    Warning,
    Error,
}

public sealed record PluginValidationReport(IReadOnlyList<PluginValidationEntry>? Entries)
{
    public static PluginValidationReport Empty { get; } = new(Array.Empty<PluginValidationEntry>());

    public IReadOnlyList<PluginValidationEntry> SafeEntries => Entries ?? Array.Empty<PluginValidationEntry>();

    public int AcceptedCount => SafeEntries.Count(entry => IsStatus(entry, "Accepted"));

    public int DisabledCount => SafeEntries.Count(entry => IsStatus(entry, "Disabled"));

    public int RejectedCount => SafeEntries.Count(entry => IsStatus(entry, "Rejected"));

    public int WarningCount => SafeEntries.Sum(entry => entry.SafeIssues.Count(issue => IsSeverity(issue, "Warning")));

    public int ErrorCount => SafeEntries.Sum(entry => entry.SafeIssues.Count(issue => IsSeverity(issue, "Error")));

    public int IssueCount => WarningCount + ErrorCount;

    public PluginValidationHealth Health => ErrorCount > 0
        ? PluginValidationHealth.Error
        : WarningCount > 0
            ? PluginValidationHealth.Warning
            : PluginValidationHealth.Ready;

    private static bool IsStatus(PluginValidationEntry entry, string status) =>
        string.Equals(entry.Status, status, StringComparison.OrdinalIgnoreCase);

    private static bool IsSeverity(PluginValidationIssue issue, string severity) =>
        string.Equals(issue.Severity, severity, StringComparison.OrdinalIgnoreCase);
}

public sealed record PluginValidationEntry(
    string? Id,
    string? Name,
    string? ManifestPath,
    string Source,
    string Status,
    bool Trusted,
    IReadOnlyList<PluginValidationIssue>? Issues)
{
    public IReadOnlyList<PluginValidationIssue> SafeIssues => Issues ?? Array.Empty<PluginValidationIssue>();

    public bool HasIssues => SafeIssues.Count > 0;
}

public sealed record PluginValidationIssue(
    string Severity,
    string Stage,
    string Code,
    string Message);
