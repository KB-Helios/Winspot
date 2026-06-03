using System.ComponentModel;
using System.Runtime.CompilerServices;

using Winspot_App.Models;
using Winspot_App.Services;
using Winspot_App.Strings;

namespace Winspot_App.ViewModels;

public sealed class SettingsViewModel : INotifyPropertyChanged
{
    private static readonly HashSet<string> NamedKeys = new(StringComparer.OrdinalIgnoreCase)
    {
        "Back", "Backspace", "Tab", "Enter", "Return", "Esc", "Escape",
        "Space", "Left", "Up", "Right", "Down",
    };

    private readonly LauncherSettingsStore _store;
    private readonly IWinspotIpcClient _ipcClient;

    private bool _useControl;
    private bool _useAlt;
    private bool _useShift;
    private bool _useWin;
    private string _key = "Space";
    private bool _launchOnStartup;
    private bool _showTrayIcon = true;
    private ThemeMode _themeMode = ThemeMode.Dark;
    private MotionProfile _motionProfile = MotionProfile.Snappy240;
    private int _selectedSectionIndex;
    private string _statusMessage = string.Empty;
    private bool _isPluginValidationRunning;
    private PluginValidationReport _pluginValidationReport = PluginValidationReport.Empty;
    private IReadOnlyList<PluginValidationEntry> _pluginValidationEntries = Array.Empty<PluginValidationEntry>();
    private string _pluginValidationSummary = SettingsDisplayStrings.PluginValidationNotChecked;
    private string _pluginValidationIssueSummary =
        SettingsDisplayStrings.FormatPluginValidationIssueSummary(PluginValidationReport.Empty);
    private string _pluginValidationHealth = "Unchecked";
    private string _pluginValidationLastCheckedText = string.Empty;
    private bool _showOnlyPluginValidationIssues = true;

    public SettingsViewModel()
        : this(new LauncherSettingsStore())
    {
    }

    public SettingsViewModel(LauncherSettingsStore store)
        : this(store, AppPaths.ResolvePluginsPath(AppContext.BaseDirectory, Environment.GetEnvironmentVariable("LOCALAPPDATA")))
    {
    }

    public SettingsViewModel(LauncherSettingsStore store, string pluginsPath)
        : this(store, pluginsPath, new WinspotIpcClient())
    {
    }

    public SettingsViewModel(LauncherSettingsStore store, string pluginsPath, IWinspotIpcClient ipcClient)
    {
        _store = store;
        _ipcClient = ipcClient;
        PluginsPath = pluginsPath;
        SettingsPath = store.SettingsPath;
        IsPortable = AppPaths.IsPortable(AppContext.BaseDirectory);
        LoadFrom(store.Load());
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    /// Raised after settings are validated and persisted, carrying the saved
    /// snapshot so the application can re-register the hotkey and tray icon.
    public event EventHandler<LauncherSettings>? Saved;

    public IReadOnlyList<ThemeMode> ThemeModeOptions { get; } = Enum.GetValues<ThemeMode>();

    public IReadOnlyList<MotionProfile> MotionProfileOptions { get; } = Enum.GetValues<MotionProfile>();

    public int SelectedSectionIndex
    {
        get => _selectedSectionIndex;
        set => SetField(ref _selectedSectionIndex, value);
    }

    public string SettingsPath { get; }

    public string PluginsPath { get; }

    public bool IsPortable { get; }

    public int UserPluginManifestCount => Directory.Exists(PluginsPath)
        ? Directory.EnumerateFiles(PluginsPath, "*.json", SearchOption.TopDirectoryOnly).Count()
        : 0;

    public string UserPluginManifestCountText => SettingsDisplayStrings.FormatUserPluginManifestCount(UserPluginManifestCount);

    public string BuiltInPluginsText => SettingsDisplayStrings.BuiltInPluginsText;

    public string AppVersion => typeof(SettingsViewModel).Assembly.GetName().Version?.ToString(3)
        ?? SettingsDisplayStrings.AppVersionFallback;

    public string DiagnosticsText => SettingsDisplayStrings.FormatDiagnostics(
        SettingsPath,
        PluginsPath,
        IsPortable,
        ThemeMode.ToString(),
        MotionProfile.ToString(),
        ShowTrayIcon,
        HotkeyPreview,
        AppVersion,
        PluginValidationSummary);

    public bool UseControl
    {
        get => _useControl;
        set => SetChord(ref _useControl, value);
    }

    public bool UseAlt
    {
        get => _useAlt;
        set => SetChord(ref _useAlt, value);
    }

    public bool UseShift
    {
        get => _useShift;
        set => SetChord(ref _useShift, value);
    }

    public bool UseWin
    {
        get => _useWin;
        set => SetChord(ref _useWin, value);
    }

    public string Key
    {
        get => _key;
        set
        {
            if (SetField(ref _key, value))
            {
                OnPropertyChanged(nameof(HotkeyPreview));
                OnPropertyChanged(nameof(DiagnosticsText));
            }
        }
    }

    public bool LaunchOnStartup
    {
        get => _launchOnStartup;
        set => SetField(ref _launchOnStartup, value);
    }

    public bool ShowTrayIcon
    {
        get => _showTrayIcon;
        set
        {
            if (SetField(ref _showTrayIcon, value))
            {
                OnPropertyChanged(nameof(DiagnosticsText));
            }
        }
    }

    public bool ReduceMotion
    {
        get => _motionProfile == MotionProfile.Reduced;
        set => MotionProfile = value ? MotionProfile.Reduced : MotionProfile.Snappy240;
    }

    public ThemeMode ThemeMode
    {
        get => _themeMode;
        set
        {
            if (SetField(ref _themeMode, value))
            {
                OnPropertyChanged(nameof(DiagnosticsText));
            }
        }
    }

    public MotionProfile MotionProfile
    {
        get => _motionProfile;
        set
        {
            if (SetField(ref _motionProfile, value))
            {
                OnPropertyChanged(nameof(ReduceMotion));
                OnPropertyChanged(nameof(DiagnosticsText));
            }
        }
    }

    public string StatusMessage
    {
        get => _statusMessage;
        private set => SetField(ref _statusMessage, value);
    }

    public bool IsPluginValidationRunning
    {
        get => _isPluginValidationRunning;
        private set => SetField(ref _isPluginValidationRunning, value);
    }

    public string PluginValidationSummary
    {
        get => _pluginValidationSummary;
        private set
        {
            if (SetField(ref _pluginValidationSummary, value))
            {
                OnPropertyChanged(nameof(DiagnosticsText));
            }
        }
    }

    public string PluginValidationIssueSummary
    {
        get => _pluginValidationIssueSummary;
        private set => SetField(ref _pluginValidationIssueSummary, value);
    }

    public string PluginValidationHealth
    {
        get => _pluginValidationHealth;
        private set => SetField(ref _pluginValidationHealth, value);
    }

    public string PluginValidationLastCheckedText
    {
        get => _pluginValidationLastCheckedText;
        private set => SetField(ref _pluginValidationLastCheckedText, value);
    }

    public bool ShowOnlyPluginValidationIssues
    {
        get => _showOnlyPluginValidationIssues;
        set
        {
            if (SetField(ref _showOnlyPluginValidationIssues, value))
            {
                RefreshPluginValidationEntries();
            }
        }
    }

    public IReadOnlyList<PluginValidationEntry> PluginValidationEntries
    {
        get => _pluginValidationEntries;
        private set => SetField(ref _pluginValidationEntries, value);
    }

    /// The activation chord as it would be shown to the user (e.g. "Ctrl Alt Space").
    public string HotkeyPreview => BuildBinding().ToDisplayString();

    public bool IsValid => BuildModifiers().Count > 0 && IsKeyValid(_key);

    public LauncherSettings BuildSettings() => new()
    {
        Hotkey = BuildBinding(),
        LaunchOnStartup = _launchOnStartup,
        ShowTrayIcon = _showTrayIcon,
        ReduceMotion = ReduceMotion,
        ThemeMode = _themeMode,
        MotionProfile = _motionProfile,
    };

    public bool TrySave()
    {
        if (BuildModifiers().Count == 0)
        {
            StatusMessage = "Choose at least one modifier (Ctrl, Alt, Shift, or Win).";
            return false;
        }

        if (!IsKeyValid(_key))
        {
            StatusMessage = "Enter a single letter or number, or a key such as Space or Enter.";
            return false;
        }

        var settings = BuildSettings();
        if (settings.Hotkey.IsReservedByWindows())
        {
            StatusMessage = SettingsStatusMessages.ReservedWindowsHotkey;
            return false;
        }

        _store.Save(settings);
        StartupRegistration.Apply(settings.LaunchOnStartup);

        StatusMessage = "Saved.";
        Saved?.Invoke(this, settings);
        return true;
    }

    public void SetHotkeyRegistrationStatus(bool registered)
    {
        StatusMessage = registered
            ? "Saved. Hotkey is ready."
            : "Saved, but that hotkey is unavailable. The previous hotkey is still active.";
    }

    public async Task OpenPluginsFolderAsync(CancellationToken cancellationToken)
    {
        try
        {
            var action = new ActionItem(
                SettingsDisplayStrings.OpenActionId,
                SettingsDisplayStrings.OpenActionId,
                SettingsDisplayStrings.OpenActionId);
            var result = new SearchResultItem(
                $"folder:{PluginsPath}",
                SettingsDisplayStrings.PluginsFolderTitle,
                PluginsPath,
                SettingsDisplayStrings.PluginsFolderKind,
                1,
                SettingsDisplayStrings.OpenActionId,
                new[] { action });

            await _ipcClient.ExecuteAsync(result, action, cancellationToken).ConfigureAwait(true);
            StatusMessage = SettingsDisplayStrings.OpenedPluginsFolder;
        }
        catch
        {
            StatusMessage = SettingsDisplayStrings.OpenPluginsFolderFailed;
        }
    }

    public async Task RefreshPluginValidationAsync(CancellationToken cancellationToken)
    {
        IsPluginValidationRunning = true;
        PluginValidationSummary = SettingsDisplayStrings.PluginValidationChecking;
        PluginValidationHealth = "Checking";

        try
        {
            _pluginValidationReport = await _ipcClient.GetPluginDiagnosticsAsync(cancellationToken).ConfigureAwait(true);
            PluginValidationHealth = _pluginValidationReport.Health;
            PluginValidationSummary = SettingsDisplayStrings.FormatPluginValidationSummary(_pluginValidationReport);
            PluginValidationIssueSummary = SettingsDisplayStrings.FormatPluginValidationIssueSummary(_pluginValidationReport);
            PluginValidationLastCheckedText = SettingsDisplayStrings.FormatPluginValidationLastChecked(DateTimeOffset.Now);
            RefreshPluginValidationEntries();
        }
        catch (OperationCanceledException)
        {
        }
        catch
        {
            _pluginValidationReport = PluginValidationReport.Empty;
            PluginValidationEntries = Array.Empty<PluginValidationEntry>();
            PluginValidationHealth = "Unavailable";
            PluginValidationSummary = SettingsDisplayStrings.PluginValidationUnavailable;
            PluginValidationIssueSummary =
                SettingsDisplayStrings.FormatPluginValidationIssueSummary(PluginValidationReport.Empty);
            PluginValidationLastCheckedText = string.Empty;
        }
        finally
        {
            IsPluginValidationRunning = false;
        }
    }

    private void LoadFrom(LauncherSettings settings)
    {
        var modifiers = settings.Hotkey.Modifiers
            .Select(m => m.Trim().ToLowerInvariant())
            .ToHashSet();

        _useControl = modifiers.Contains("control") || modifiers.Contains("ctrl");
        _useAlt = modifiers.Contains("alt");
        _useShift = modifiers.Contains("shift");
        _useWin = modifiers.Contains("win") || modifiers.Contains("windows");
        _key = settings.Hotkey.Key;
        _launchOnStartup = settings.LaunchOnStartup;
        _showTrayIcon = settings.ShowTrayIcon;
        _themeMode = settings.ThemeMode;
        _motionProfile = settings.ReduceMotion ? MotionProfile.Reduced : settings.MotionProfile;
    }

    private HotkeyBinding BuildBinding() => new()
    {
        Key = string.IsNullOrWhiteSpace(_key) ? string.Empty : _key.Trim(),
        Modifiers = BuildModifiers(),
    };

    private List<string> BuildModifiers()
    {
        var modifiers = new List<string>();
        if (_useControl)
        {
            modifiers.Add("Control");
        }

        if (_useAlt)
        {
            modifiers.Add("Alt");
        }

        if (_useShift)
        {
            modifiers.Add("Shift");
        }

        if (_useWin)
        {
            modifiers.Add("Win");
        }

        return modifiers;
    }

    private void RefreshPluginValidationEntries()
    {
        PluginValidationEntries = ShowOnlyPluginValidationIssues
            ? _pluginValidationReport.SafeEntries.Where(entry => entry.HasIssues).ToArray()
            : _pluginValidationReport.SafeEntries.ToArray();
    }

    private static bool IsKeyValid(string? key)
    {
        if (string.IsNullOrWhiteSpace(key))
        {
            return false;
        }

        var trimmed = key.Trim();
        if (trimmed.Length == 1)
        {
            var c = char.ToUpperInvariant(trimmed[0]);
            return c is >= 'A' and <= 'Z' or >= '0' and <= '9';
        }

        return NamedKeys.Contains(trimmed);
    }

    private void SetChord(ref bool field, bool value, [CallerMemberName] string? propertyName = null)
    {
        if (SetField(ref field, value, propertyName))
        {
            OnPropertyChanged(nameof(HotkeyPreview));
            OnPropertyChanged(nameof(DiagnosticsText));
        }
    }

    private bool SetField<T>(ref T field, T value, [CallerMemberName] string? propertyName = null)
    {
        if (EqualityComparer<T>.Default.Equals(field, value))
        {
            return false;
        }

        field = value;
        OnPropertyChanged(propertyName);
        return true;
    }

    private void OnPropertyChanged(string? propertyName)
    {
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
    }
}
