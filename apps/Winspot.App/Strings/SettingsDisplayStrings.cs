using System.Globalization;

namespace Winspot_App.Strings;

internal static class SettingsDisplayStrings
{
    public const string BuiltInPluginsText = "Calculator, Terminal, Clipboard, Unit Conversion, FastFlowLM";
    public const string UserPluginManifestCountSingularFormat = "{0} user manifest";
    public const string UserPluginManifestCountPluralFormat = "{0} user manifests";
    public const string DiagnosticsFormat =
        "Settings: {0}\nPlugins: {1}\nPortable: {2}\nTheme: {3}\nMotion: {4}\nTray: {5}\nHotkey: {6}\nVersion: {7}";
    public const string BooleanYes = "Yes";
    public const string BooleanNo = "No";
    public const string TrayEnabled = "Enabled";
    public const string TrayHidden = "Hidden";
    public const string AppVersionFallback = "0.1.0";
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
        string appVersion) => string.Format(
            CultureInfo.InvariantCulture,
            DiagnosticsFormat,
            settingsPath,
            pluginsPath,
            isPortable ? BooleanYes : BooleanNo,
            themeMode,
            motionProfile,
            showTrayIcon ? TrayEnabled : TrayHidden,
            hotkeyPreview,
            appVersion);
}
