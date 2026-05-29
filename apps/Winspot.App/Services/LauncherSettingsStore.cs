using System.Text.Json;

using Winspot_App.Models;

namespace Winspot_App.Services;

internal sealed class LauncherSettingsStore
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
            return JsonSerializer.Deserialize<LauncherSettings>(json, JsonOptions) ?? new LauncherSettings();
        }
        catch
        {
            return new LauncherSettings();
        }
    }

    private void Save(LauncherSettings settings)
    {
        var directory = Path.GetDirectoryName(_settingsPath);
        if (!string.IsNullOrEmpty(directory))
        {
            Directory.CreateDirectory(directory);
        }

        File.WriteAllText(_settingsPath, JsonSerializer.Serialize(settings, JsonOptions));
    }

    private static string DefaultSettingsPath()
    {
        var localAppData = Environment.GetEnvironmentVariable("LOCALAPPDATA");
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
}
