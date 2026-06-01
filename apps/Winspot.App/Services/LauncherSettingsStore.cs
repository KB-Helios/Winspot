using System.Text.Json;

using Winspot_App.Models;

namespace Winspot_App.Services;

public sealed class LauncherSettingsStore
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true,
    };

    private readonly string _settingsPath;

    public LauncherSettingsStore()
        : this(DefaultSettingsPath())
    {
    }

    public LauncherSettingsStore(string settingsPath)
    {
        _settingsPath = settingsPath;
    }

    public string SettingsPath => _settingsPath;

    public LauncherSettings Load()
    {
        try
        {
            if (!File.Exists(_settingsPath))
            {
                var defaults = new LauncherSettings();
                Save(defaults);
                return defaults;
            }

            var json = File.ReadAllText(_settingsPath);
            var settings = JsonSerializer.Deserialize<LauncherSettings>(json, JsonOptions) ?? new LauncherSettings();
            if (!settings.Hotkey.IsReservedByWindows())
            {
                return settings;
            }

            var repaired = new LauncherSettings
            {
                Hotkey = new HotkeyBinding(),
                LaunchOnStartup = settings.LaunchOnStartup,
                ShowTrayIcon = settings.ShowTrayIcon,
                ReduceMotion = settings.ReduceMotion,
            };
            Save(repaired);
            return repaired;
        }
        catch
        {
            return new LauncherSettings();
        }
    }

    public void Save(LauncherSettings settings)
    {
        var directory = Path.GetDirectoryName(_settingsPath);
        if (!string.IsNullOrEmpty(directory))
        {
            Directory.CreateDirectory(directory);
        }

        File.WriteAllText(_settingsPath, JsonSerializer.Serialize(settings, JsonOptions));
    }

    public static string ResolveSettingsPath(string baseDirectory, string? localAppData)
    {
        var portableMarker = Path.Combine(baseDirectory, "Winspot.portable");
        if (File.Exists(portableMarker))
        {
            return Path.Combine(baseDirectory, "data", "settings.json");
        }

        if (string.IsNullOrWhiteSpace(localAppData))
        {
            var userProfile = Environment.GetEnvironmentVariable("USERPROFILE");
            if (!string.IsNullOrWhiteSpace(userProfile))
            {
                localAppData = Path.Combine(userProfile, "AppData", "Local");
            }
        }

        if (string.IsNullOrWhiteSpace(localAppData))
        {
            localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        }

        return Path.Combine(localAppData, "Winspot", "settings.json");
    }

    private static string DefaultSettingsPath()
    {
        var localAppData = Environment.GetEnvironmentVariable("LOCALAPPDATA");
        return ResolveSettingsPath(AppContext.BaseDirectory, localAppData);
    }
}
