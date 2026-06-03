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
    private bool _captureEnabled = true;
    private string _captureOutputDirectory = string.Empty;
    private string _captureDefaultRecordSeconds = "8";
    private string _captureMaxRecordSeconds = "60";
    private bool _captureIncludeCursor = true;
    private string _capturePreCaptureDelayMs = "250";
    private int _selectedSectionIndex;
    private string _statusMessage = string.Empty;
    private bool _isPluginValidationRunning;
    private PluginValidationReport _pluginValidationReport = PluginValidationReport.Empty;
    private IReadOnlyList<PluginValidationEntryViewItem> _pluginValidationEntries = Array.Empty<PluginValidationEntryViewItem>();
    private string _pluginValidationSummary = SettingsDisplayStrings.PluginValidationNotChecked;
    private string _pluginValidationIssueSummary = string.Empty;
    private string _pluginValidationHealth = SettingsDisplayStrings.PluginValidationHealthUnchecked;
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

    public bool CaptureEnabled
    {
        get => _captureEnabled;
        set => SetField(ref _captureEnabled, value);
    }

    public string CaptureOutputDirectory
    {
        get => _captureOutputDirectory;
        set => SetField(ref _captureOutputDirectory, value);
    }

    public string CaptureDefaultRecordSeconds
    {
        get => _captureDefaultRecordSeconds;
        set => SetField(ref _captureDefaultRecordSeconds, value);
    }

    public string CaptureMaxRecordSeconds
    {
        get => _captureMaxRecordSeconds;
        set => SetField(ref _captureMaxRecordSeconds, value);
    }

    public bool CaptureIncludeCursor
    {
        get => _captureIncludeCursor;
        set => SetField(ref _captureIncludeCursor, value);
    }

    public string CapturePreCaptureDelayMs
    {
        get => _capturePreCaptureDelayMs;
        set => SetField(ref _capturePreCaptureDelayMs, value);
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

    public IReadOnlyList<PluginValidationEntryViewItem> PluginValidationEntries
    {
        get => _pluginValidationEntries;
        private set => SetField(ref _pluginValidationEntries, value);
    }

    /// The activation chord as it would be shown to the user (e.g. "Ctrl Alt Space").
    public string HotkeyPreview => BuildBinding().ToDisplayString();

    public bool IsValid => BuildModifiers().Count > 0
        && IsKeyValid(_key)
        && FastFlowLmNumbersCanBeSaved()
        && CaptureNumbersCanBeSaved();

    /// <summary>
    /// Builds a LauncherSettings snapshot from the view-model's current state.
    /// </summary>
    /// <returns>A <see cref="LauncherSettings"/> populated with the view-model's hotkey, launch-on-startup and tray settings, motion and theme settings, and FastFlowLM configuration.</returns>
    public LauncherSettings BuildSettings() => new()
    {
        Hotkey = BuildBinding(),
        LaunchOnStartup = _launchOnStartup,
        ShowTrayIcon = _showTrayIcon,
        ReduceMotion = ReduceMotion,
        ThemeMode = _themeMode,
        MotionProfile = _motionProfile,
        FastFlowLm = BuildFastFlowLmSettings(),
        Capture = BuildCaptureSettings(),
    };

    /// <summary>
    /// Validates current view-model settings and, if valid, persists them, applies startup registration, updates status, and raises the Saved event.
    /// </summary>
    /// <remarks>
    /// Validation performed:
    /// - Requires at least one hotkey modifier (Ctrl, Alt, Shift, or Win).
    /// - Requires a valid hotkey key (single letter/digit or a named key such as Space or Enter).
    /// - Requires FastFlowLM numeric fields to be positive whole numbers when FastFlowLM is enabled.
    /// - Rejects hotkeys reserved by Windows.
    /// On validation failure the method sets <see cref="StatusMessage"/> to an explanatory message and does not persist changes.
    /// On success the method saves settings to the store, applies startup registration according to the saved setting, sets <see cref="StatusMessage"/> to "Saved.", and invokes the <see cref="Saved"/> event with the persisted settings.
    /// </remarks>
    /// <returns>`true` if settings were validated and saved successfully, `false` otherwise.</returns>
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

        if (_fastFlowLmEnabled && !FastFlowLmNumbersAreValid())
        {
            StatusMessage = "FastFlowLM numeric settings must be positive whole numbers.";
            return false;
        }

        if (_captureEnabled && !CaptureNumbersAreValid())
        {
            StatusMessage = SettingsStatusMessages.CaptureValidation;
            return false;
        }

        if (!IsCaptureOutputDirectoryValid(_captureOutputDirectory))
        {
            StatusMessage = "Capture output directory path is invalid.";
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
        if (IsPluginValidationRunning)
        {
            return;
        }

        var previousReport = _pluginValidationReport;
        var previousEntries = PluginValidationEntries;
        var previousHealth = PluginValidationHealth;
        var previousSummary = PluginValidationSummary;
        var previousIssueSummary = PluginValidationIssueSummary;
        var previousLastCheckedText = PluginValidationLastCheckedText;

        IsPluginValidationRunning = true;
        PluginValidationSummary = SettingsDisplayStrings.PluginValidationChecking;
        PluginValidationHealth = SettingsDisplayStrings.PluginValidationHealthChecking;

        try
        {
            _pluginValidationReport = await _ipcClient.GetPluginDiagnosticsAsync(cancellationToken).ConfigureAwait(true);
            PluginValidationHealth = SettingsDisplayStrings.FormatPluginValidationHealth(_pluginValidationReport.Health);
            PluginValidationSummary = SettingsDisplayStrings.FormatPluginValidationSummary(_pluginValidationReport);
            PluginValidationIssueSummary = SettingsDisplayStrings.FormatPluginValidationIssueSummary(_pluginValidationReport);
            PluginValidationLastCheckedText = SettingsDisplayStrings.FormatPluginValidationLastChecked(DateTimeOffset.Now);
            RefreshPluginValidationEntries();
        }
        catch (OperationCanceledException)
        {
            _pluginValidationReport = previousReport;
            PluginValidationEntries = previousEntries;
            PluginValidationHealth = previousHealth;
            PluginValidationSummary = previousSummary;
            PluginValidationIssueSummary = previousIssueSummary;
            PluginValidationLastCheckedText = previousLastCheckedText;
        }
        catch
        {
            _pluginValidationReport = PluginValidationReport.Empty;
            PluginValidationEntries = Array.Empty<PluginValidationEntryViewItem>();
            PluginValidationHealth = SettingsDisplayStrings.PluginValidationHealthUnavailable;
            PluginValidationSummary = SettingsDisplayStrings.PluginValidationUnavailable;
            PluginValidationIssueSummary = string.Empty;
            PluginValidationLastCheckedText = string.Empty;
            OnPropertyChanged(nameof(DiagnosticsText));
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
        _fastFlowLmEnabled = settings.FastFlowLm.Enabled;
        _fastFlowLmModelTag = settings.FastFlowLm.ModelTag;
        _fastFlowLmExecutablePath = settings.FastFlowLm.ExecutablePath;
        _fastFlowLmPort = settings.FastFlowLm.Port.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _fastFlowLmIdleTimeoutSeconds = settings.FastFlowLm.IdleTimeoutSeconds.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _fastFlowLmMaxContextFiles = settings.FastFlowLm.MaxContextFiles.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _fastFlowLmMaxFileBytes = settings.FastFlowLm.MaxFileBytes.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _fastFlowLmMaxContextBytes = settings.FastFlowLm.MaxContextBytes.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _captureEnabled = settings.Capture.Enabled;
        _captureOutputDirectory = settings.Capture.OutputDirectory;
        _captureDefaultRecordSeconds = settings.Capture.DefaultRecordSeconds.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _captureMaxRecordSeconds = settings.Capture.MaxRecordSeconds.ToString(System.Globalization.CultureInfo.InvariantCulture);
        _captureIncludeCursor = settings.Capture.IncludeCursor;
        _capturePreCaptureDelayMs = settings.Capture.PreCaptureDelayMs.ToString(System.Globalization.CultureInfo.InvariantCulture);
    }

    /// <summary>
    /// Constructs a HotkeyBinding that reflects the view-model's current key and modifier selection.
    /// </summary>
    /// <returns>A HotkeyBinding whose Key is the trimmed key string (or empty if none) and whose Modifiers are the current modifier list.</returns>
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
        var entries = ShowOnlyPluginValidationIssues
            ? _pluginValidationReport.SafeEntries.Where(entry => entry.HasIssues).ToArray()
            : _pluginValidationReport.SafeEntries.ToArray();

        PluginValidationEntries = entries.Select(CreatePluginValidationEntryViewItem).ToArray();
    }

    private static PluginValidationEntryViewItem CreatePluginValidationEntryViewItem(PluginValidationEntry entry) => new(
        SettingsDisplayStrings.FormatPluginValidationEntryName(entry),
        entry.Status,
        SettingsDisplayStrings.FormatPluginValidationTrust(entry.Trusted),
        entry.ManifestPath,
        entry.SafeIssues
            .Select(issue => new PluginValidationIssueViewItem(SettingsDisplayStrings.FormatPluginValidationIssue(issue)))
        .ToArray());

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

    private static bool IsCaptureOutputDirectoryValid(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return true;
        }

        try
        {
            var trimmed = value.Trim();
            _ = Path.GetFullPath(trimmed);
            return true;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (NotSupportedException)
        {
            return false;
        }
        catch (PathTooLongException)
        {
            return false;
        }
    }

    /// <summary>
    /// Create a FastFlowLmSettings instance from the view-model's FastFlowLM fields.
    /// </summary>
    /// <returns>
    /// A FastFlowLmSettings populated from the view-model: `Enabled` taken from the backing flag; `ModelTag` and `ExecutablePath` trimmed and replaced by defaults when empty; numeric fields parsed from their string representations using `ParsePositiveInt`, falling back to default values when parsing fails or values are invalid.
    /// </returns>
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
            Port = ParsePositiveInt(_fastFlowLmPort, defaults.Port, maxValue: 65535),
            IdleTimeoutSeconds = ParsePositiveInt(_fastFlowLmIdleTimeoutSeconds, defaults.IdleTimeoutSeconds),
            MaxContextFiles = ParsePositiveInt(_fastFlowLmMaxContextFiles, defaults.MaxContextFiles),
            MaxFileBytes = ParsePositiveInt(_fastFlowLmMaxFileBytes, defaults.MaxFileBytes),
            MaxContextBytes = ParsePositiveInt(_fastFlowLmMaxContextBytes, defaults.MaxContextBytes),
        };
    }

    private CaptureSettings BuildCaptureSettings()
    {
        var defaults = new CaptureSettings();
        var maxRecordSeconds = ParsePositiveInt(_captureMaxRecordSeconds, defaults.MaxRecordSeconds, maxValue: 300);
        var trimmedDirectory = string.IsNullOrWhiteSpace(_captureOutputDirectory)
            ? string.Empty
            : _captureOutputDirectory.Trim();
        return new CaptureSettings
        {
            Enabled = _captureEnabled,
            OutputDirectory = IsCaptureOutputDirectoryValid(trimmedDirectory)
                ? trimmedDirectory
                : string.Empty,
            DefaultRecordSeconds = Math.Min(
                ParsePositiveInt(_captureDefaultRecordSeconds, defaults.DefaultRecordSeconds, maxValue: 300),
                maxRecordSeconds),
            MaxRecordSeconds = maxRecordSeconds,
            IncludeCursor = _captureIncludeCursor,
            PreCaptureDelayMs = ParseNonNegativeInt(_capturePreCaptureDelayMs, defaults.PreCaptureDelayMs, maxValue: 5000),
        };
    }

    private bool FastFlowLmNumbersCanBeSaved() =>
        !_fastFlowLmEnabled || FastFlowLmNumbersAreValid();

    private bool FastFlowLmNumbersAreValid() =>
        IsPositiveInt(_fastFlowLmPort, maxValue: 65535)
        && IsPositiveInt(_fastFlowLmIdleTimeoutSeconds)
        && IsPositiveInt(_fastFlowLmMaxContextFiles)
        && IsPositiveInt(_fastFlowLmMaxFileBytes)
        && IsPositiveInt(_fastFlowLmMaxContextBytes);

    private bool CaptureNumbersCanBeSaved() =>
        !_captureEnabled || CaptureNumbersAreValid();

    private bool CaptureNumbersAreValid()
    {
        if (!IsPositiveInt(_captureDefaultRecordSeconds, maxValue: 300)
            || !IsPositiveInt(_captureMaxRecordSeconds, maxValue: 300)
            || !IsNonNegativeInt(_capturePreCaptureDelayMs, maxValue: 5000))
        {
            return false;
        }

        var defaultRecordSeconds = int.Parse(
            _captureDefaultRecordSeconds,
            System.Globalization.NumberStyles.None,
            System.Globalization.CultureInfo.InvariantCulture);
        var maxRecordSeconds = int.Parse(
            _captureMaxRecordSeconds,
            System.Globalization.NumberStyles.None,
            System.Globalization.CultureInfo.InvariantCulture);
        return defaultRecordSeconds <= maxRecordSeconds;
    }

    private static bool IsPositiveInt(string? value, int maxValue = int.MaxValue) =>
        int.TryParse(
            value,
            System.Globalization.NumberStyles.None,
            System.Globalization.CultureInfo.InvariantCulture,
            out var parsed)
        && parsed > 0
        && parsed <= maxValue;

    private static bool IsNonNegativeInt(string? value, int maxValue = int.MaxValue) =>
        int.TryParse(
            value,
            System.Globalization.NumberStyles.None,
            System.Globalization.CultureInfo.InvariantCulture,
            out var parsed)
        && parsed >= 0
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

    private static int ParseNonNegativeInt(string? value, int fallback, int maxValue = int.MaxValue) =>
        int.TryParse(
            value,
            System.Globalization.NumberStyles.None,
            System.Globalization.CultureInfo.InvariantCulture,
            out var parsed)
        && parsed >= 0
        && parsed <= maxValue
            ? parsed
            : fallback;

    /// <summary>
    /// Sets a boolean backing field for a hotkey modifier and, when the value changes, raises property-changed notifications for the modifier plus the derived HotkeyPreview and DiagnosticsText properties.
    /// </summary>
    /// <param name="field">Reference to the backing boolean field for the modifier (e.g., _useControl).</param>
    /// <param name="value">New boolean value to assign to the backing field.</param>
    /// <param name="propertyName">Name of the property being set; provided automatically by the caller via CallerMemberName if omitted.</param>
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

public sealed record PluginValidationEntryViewItem(
    string DisplayName,
    string Status,
    string TrustSummary,
    string? ManifestPath,
    IReadOnlyList<PluginValidationIssueViewItem> Issues);

public sealed record PluginValidationIssueViewItem(string DisplayText);
