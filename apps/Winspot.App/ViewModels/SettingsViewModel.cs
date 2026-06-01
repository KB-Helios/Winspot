using System.ComponentModel;
using System.Runtime.CompilerServices;

using Winspot_App.Models;
using Winspot_App.Services;

namespace Winspot_App.ViewModels;

public sealed class SettingsViewModel : INotifyPropertyChanged
{
    private static readonly HashSet<string> NamedKeys = new(StringComparer.OrdinalIgnoreCase)
    {
        "Back", "Backspace", "Tab", "Enter", "Return", "Esc", "Escape",
        "Space", "Left", "Up", "Right", "Down",
    };

    private readonly LauncherSettingsStore _store;

    private bool _useControl;
    private bool _useAlt;
    private bool _useShift;
    private bool _useWin;
    private string _key = "Space";
    private bool _launchOnStartup;
    private bool _showTrayIcon = true;
    private bool _reduceMotion;
    private string _statusMessage = string.Empty;

    public SettingsViewModel()
        : this(new LauncherSettingsStore())
    {
    }

    public SettingsViewModel(LauncherSettingsStore store)
    {
        _store = store;
        LoadFrom(store.Load());
    }

    public event PropertyChangedEventHandler? PropertyChanged;

    /// Raised after settings are validated and persisted, carrying the saved
    /// snapshot so the application can re-register the hotkey and tray icon.
    public event EventHandler<LauncherSettings>? Saved;

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
        set => SetField(ref _showTrayIcon, value);
    }

    public bool ReduceMotion
    {
        get => _reduceMotion;
        set => SetField(ref _reduceMotion, value);
    }

    public string StatusMessage
    {
        get => _statusMessage;
        private set => SetField(ref _statusMessage, value);
    }

    /// The activation chord as it would be shown to the user (e.g. "Ctrl Alt Space").
    public string HotkeyPreview => BuildBinding().ToDisplayString();

    public bool IsValid => BuildModifiers().Count > 0 && IsKeyValid(_key);

    public LauncherSettings BuildSettings() => new()
    {
        Hotkey = BuildBinding(),
        LaunchOnStartup = _launchOnStartup,
        ShowTrayIcon = _showTrayIcon,
        ReduceMotion = _reduceMotion,
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
        _reduceMotion = settings.ReduceMotion;
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

    private void SetChord(ref bool field, bool value, [CallerMemberName] string? propertyName = null)
    {
        if (SetField(ref field, value, propertyName))
        {
            OnPropertyChanged(nameof(HotkeyPreview));
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
