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
    private bool _fastFlowLmEnabled = true;
    private string _fastFlowLmModelTag = "gemma4-it:e2b";
    private string _fastFlowLmExecutablePath = "flm";
    private string _fastFlowLmPort = "52625";
    private string _fastFlowLmIdleTimeoutSeconds = "120";
    private string _fastFlowLmMaxContextFiles = "5";
    private string _fastFlowLmMaxFileBytes = (1024 * 1024).ToString(System.Globalization.CultureInfo.InvariantCulture);
    private string _fastFlowLmMaxContextBytes = (4 * 1024 * 1024).ToString(System.Globalization.CultureInfo.InvariantCulture);
    private int _selectedSectionIndex;
    private string _statusMessage = string.Empty;

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
        AppVersion);

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

    public bool FastFlowLmEnabled
    {
        get => _fastFlowLmEnabled;
        set => SetField(ref _fastFlowLmEnabled, value);
    }

    public string FastFlowLmModelTag
    {
        get => _fastFlowLmModelTag;
        set => SetField(ref _fastFlowLmModelTag, value);
    }

    public string FastFlowLmExecutablePath
    {
        get => _fastFlowLmExecutablePath;
        set => SetField(ref _fastFlowLmExecutablePath, value);
    }

    public string FastFlowLmPort
    {
        get => _fastFlowLmPort;
        set => SetField(ref _fastFlowLmPort, value);
    }

    public string FastFlowLmIdleTimeoutSeconds
    {
        get => _fastFlowLmIdleTimeoutSeconds;
        set => SetField(ref _fastFlowLmIdleTimeoutSeconds, value);
    }

    public string FastFlowLmMaxContextFiles
    {
        get => _fastFlowLmMaxContextFiles;
        set => SetField(ref _fastFlowLmMaxContextFiles, value);
    }

    public string FastFlowLmMaxFileBytes
    {
        get => _fastFlowLmMaxFileBytes;
        set => SetField(ref _fastFlowLmMaxFileBytes, value);
    }

    public string FastFlowLmMaxContextBytes
    {
        get => _fastFlowLmMaxContextBytes;
        set => SetField(ref _fastFlowLmMaxContextBytes, value);
    }

    public string StatusMessage
    {
        get => _statusMessage;
        private set => SetField(ref _statusMessage, value);
    }

    /// The activation chord as it would be shown to the user (e.g. "Ctrl Alt Space").
    public string HotkeyPreview => BuildBinding().ToDisplayString();

    public bool IsValid => BuildModifiers().Count > 0 && IsKeyValid(_key) && FastFlowLmNumbersAreValid();

    public LauncherSettings BuildSettings() => new()
    {
        Hotkey = BuildBinding(),
        LaunchOnStartup = _launchOnStartup,
        ShowTrayIcon = _showTrayIcon,
        ReduceMotion = ReduceMotion,
        ThemeMode = _themeMode,
        MotionProfile = _motionProfile,
        FastFlowLm = BuildFastFlowLmSettings(),
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

        if (!FastFlowLmNumbersAreValid())
        {
            StatusMessage = "FastFlowLM numeric settings must be positive whole numbers.";
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
        _fastFlowLmEnabled = settings.FastFlowLm.Enabled;
        _fastFlowLmModelTag = settings.FastFlowLm.ModelTag;
        _fastFlowLmExecutablePath = settings.FastFlowLm.ExecutablePath;
        _fastFlowLmPort = settings.FastFlowLm.Port.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _fastFlowLmIdleTimeoutSeconds = settings.FastFlowLm.IdleTimeoutSeconds.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _fastFlowLmMaxContextFiles = settings.FastFlowLm.MaxContextFiles.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _fastFlowLmMaxFileBytes = settings.FastFlowLm.MaxFileBytes.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _fastFlowLmMaxContextBytes = settings.FastFlowLm.MaxContextBytes.ToString(System.Globalization.CultureInfo.InvariantCulture);
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

    private FastFlowLmSettings BuildFastFlowLmSettings()
    {
        var defaults = new FastFlowLmSettings();
        return new FastFlowLmSettings
        {
            Enabled = _fastFlowLmEnabled,
            ModelTag = string.IsNullOrWhiteSpace(_fastFlowLmModelTag)
                ? defaults.ModelTag
                : _fastFlowLmModelTag.Trim(),
            ExecutablePath = string.IsNullOrWhiteSpace(_fastFlowLmExecutablePath)
                ? defaults.ExecutablePath
                : _fastFlowLmExecutablePath.Trim(),
            Port = ParsePositiveInt(_fastFlowLmPort, defaults.Port),
            IdleTimeoutSeconds = ParsePositiveInt(_fastFlowLmIdleTimeoutSeconds, defaults.IdleTimeoutSeconds),
            MaxContextFiles = ParsePositiveInt(_fastFlowLmMaxContextFiles, defaults.MaxContextFiles),
            MaxFileBytes = ParsePositiveInt(_fastFlowLmMaxFileBytes, defaults.MaxFileBytes),
            MaxContextBytes = ParsePositiveInt(_fastFlowLmMaxContextBytes, defaults.MaxContextBytes),
        };
    }

    private bool FastFlowLmNumbersAreValid() =>
        IsPositiveInt(_fastFlowLmPort, maxValue: 65535)
        && IsPositiveInt(_fastFlowLmIdleTimeoutSeconds)
        && IsPositiveInt(_fastFlowLmMaxContextFiles)
        && IsPositiveInt(_fastFlowLmMaxFileBytes)
        && IsPositiveInt(_fastFlowLmMaxContextBytes);

    private static bool IsPositiveInt(string? value, int maxValue = int.MaxValue) =>
        int.TryParse(
            value,
            System.Globalization.NumberStyles.None,
            System.Globalization.CultureInfo.InvariantCulture,
            out var parsed)
        && parsed > 0
        && parsed <= maxValue;

    private static int ParsePositiveInt(string? value, int fallback, int maxValue = int.MaxValue) =>
        int.TryParse(
            value,
            System.Globalization.NumberStyles.None,
            System.Globalization.CultureInfo.InvariantCulture,
            out var parsed)
        && parsed > 0
        && parsed <= maxValue
            ? parsed
            : fallback;

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
