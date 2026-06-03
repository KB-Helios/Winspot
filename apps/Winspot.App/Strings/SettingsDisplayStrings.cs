using System.Globalization;
using System.IO;

using Winspot_App.Models;

namespace Winspot_App.Strings;

internal static class SettingsDisplayStrings
{
    public const string BuiltInPluginsText = "Calculator, Terminal, Clipboard, Unit Conversion, FastFlowLM";
    public const string UserPluginManifestCountSingularFormat = "{0} user manifest";
    public const string UserPluginManifestCountPluralFormat = "{0} user manifests";
    public const string DiagnosticsFormat =
        "Settings: {0}\nPlugins: {1}\nPortable: {2}\nTheme: {3}\nMotion: {4}\nTray: {5}\nHotkey: {6}\nVersion: {7}\nPlugin validation: {8}";
    public const string BooleanYes = "Yes";
    public const string BooleanNo = "No";
    public const string TrayEnabled = "Enabled";
    public const string TrayHidden = "Hidden";
    public const string AppVersionFallback = "0.1.0";
    public const string PluginValidationNotChecked = "Plugin validation not checked.";
    public const string PluginValidationChecking = "Checking plugin manifests.";
    public const string PluginValidationUnavailable = "Plugin validation unavailable.";
    public const string PluginValidationReadyFormat = "{0}. {1}";
    public const string PluginValidationCountSummaryFormat = "{0} accepted, {1} disabled, {2} rejected";
    public const string PluginValidationIssueSummaryFormat = "{0} warning(s), {1} error(s)";
    public const string PluginValidationLastCheckedFormat = "Last checked {0:t}";
    public const string PluginValidationHealthUnchecked = "Unchecked";
    public const string PluginValidationHealthChecking = "Checking";
    public const string PluginValidationHealthUnavailable = "Unavailable";
    public const string PluginValidationHealthReady = "Ready";
    public const string PluginValidationHealthWarning = "Warning";
    public const string PluginValidationHealthError = "Error";
    public const string PluginValidationUnknownPlugin = "<unknown plugin>";
    public const string PluginValidationTrusted = "Trusted";
    public const string PluginValidationSearchOnly = "Search-only";
    public const string PluginValidationIssueDisplayFormat = "[{0} {1} {2}] {3}";
    public const string PluginsFolderTitle = "Winspot plugins";
    public const string PluginsFolderKind = "Folder";
    public const string OpenActionId = "Open";
    public const string OpenedPluginsFolder = "Opened plugins folder.";
    public const string OpenPluginsFolderFailed = "Could not open plugins folder.";

    public static string FormatUserPluginManifestCount(int count) => string.Format(
        CultureInfo.InvariantCulture,
        count == 1 ? UserPluginManifestCountSingularFormat : UserPluginManifestCountPluralFormat,
        count);

    public static string FormatDiagnostics(
        string settingsPath,
        string pluginsPath,
        bool isPortable,
        string themeMode,
        string motionProfile,
        bool showTrayIcon,
        string hotkeyPreview,
        string appVersion,
        string pluginValidationSummary) => string.Format(
            CultureInfo.InvariantCulture,
            DiagnosticsFormat,
            settingsPath,
            pluginsPath,
            isPortable ? BooleanYes : BooleanNo,
            themeMode,
            motionProfile,
            showTrayIcon ? TrayEnabled : TrayHidden,
            hotkeyPreview,
            appVersion,
            pluginValidationSummary);

    public static string FormatPluginValidationSummary(PluginValidationReport report) => string.Format(
        CultureInfo.InvariantCulture,
        PluginValidationReadyFormat,
        FormatPluginValidationHealth(report.Health),
        FormatPluginValidationCountSummary(report));

    public static string FormatPluginValidationHealth(PluginValidationHealth health) => health switch
    {
        PluginValidationHealth.Error => PluginValidationHealthError,
        PluginValidationHealth.Warning => PluginValidationHealthWarning,
        _ => PluginValidationHealthReady,
    };

    public static string FormatPluginValidationCountSummary(PluginValidationReport report) => string.Format(
        CultureInfo.InvariantCulture,
        PluginValidationCountSummaryFormat,
        report.AcceptedCount,
        report.DisabledCount,
        report.RejectedCount);

    public static string FormatPluginValidationIssueSummary(PluginValidationReport report) => string.Format(
        CultureInfo.InvariantCulture,
        PluginValidationIssueSummaryFormat,
        report.WarningCount,
        report.ErrorCount);

    public static string FormatPluginValidationLastChecked(DateTimeOffset checkedAt) => string.Format(
        CultureInfo.CurrentCulture,
        PluginValidationLastCheckedFormat,
        checkedAt);

    public static string FormatPluginValidationEntryName(PluginValidationEntry entry)
    {
        if (!string.IsNullOrWhiteSpace(entry.Name))
        {
            return entry.Name;
        }

        if (!string.IsNullOrWhiteSpace(entry.Id))
        {
            return entry.Id;
        }

        if (!string.IsNullOrWhiteSpace(entry.ManifestPath))
        {
            try
            {
                return Path.GetFileName(entry.ManifestPath) ?? entry.ManifestPath;
            }
            catch (ArgumentException)
            {
                return entry.ManifestPath;
            }
        }

        return PluginValidationUnknownPlugin;
    }

    public static string FormatPluginValidationTrust(bool trusted) =>
        trusted ? PluginValidationTrusted : PluginValidationSearchOnly;

    public static string FormatPluginValidationIssue(PluginValidationIssue issue) => string.Format(
        CultureInfo.InvariantCulture,
        PluginValidationIssueDisplayFormat,
        issue.Severity,
        issue.Stage,
        issue.Code,
        issue.Message);
}
