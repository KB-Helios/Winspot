using System.Text.Json;
using System.Text.Json.Serialization;

using Winspot_App.Models;

namespace Winspot_App.Services;

public sealed class LauncherSettingsStore
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter() },
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
            var settings = NormalizeSettings(JsonSerializer.Deserialize<LauncherSettings>(json, JsonOptions) ?? new LauncherSettings());
            if (!NeedsHotkeyRepair(settings.Hotkey))
            {
                return settings;
            }

            var repaired = RepairHotkey(settings);
            TrySaveRepair(repaired);
            return repaired;
        }
        catch (JsonException)
        {
            return new LauncherSettings();
        }
        catch (IOException)
        {
            return new LauncherSettings();
        }
        catch (UnauthorizedAccessException)
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
        => AppPaths.ResolveSettingsPath(baseDirectory, localAppData);

    private static string DefaultSettingsPath()
    {
        var localAppData = Environment.GetEnvironmentVariable("LOCALAPPDATA");
        return ResolveSettingsPath(AppContext.BaseDirectory, localAppData);
    }

    private static bool NeedsHotkeyRepair(HotkeyBinding? hotkey) =>
        hotkey is null
        || string.IsNullOrWhiteSpace(hotkey.Key)
        || hotkey.Modifiers is null
        || hotkey.Modifiers.Count == 0
        || hotkey.Modifiers.Any(string.IsNullOrWhiteSpace)
        || hotkey.IsReservedByWindows();

    private static LauncherSettings RepairHotkey(LauncherSettings settings) => new()
    {
        Hotkey = new HotkeyBinding(),
        LaunchOnStartup = settings.LaunchOnStartup,
        ShowTrayIcon = settings.ShowTrayIcon,
        ReduceMotion = settings.ReduceMotion,
        ThemeMode = settings.ThemeMode,
        MotionProfile = settings.MotionProfile,
    };

    private static LauncherSettings NormalizeSettings(LauncherSettings settings)
    {
        var motionProfile = settings.ReduceMotion ? MotionProfile.Reduced : settings.MotionProfile;
        return new LauncherSettings
        {
            Hotkey = settings.Hotkey,
            LaunchOnStartup = settings.LaunchOnStartup,
            ShowTrayIcon = settings.ShowTrayIcon,
            ReduceMotion = motionProfile == MotionProfile.Reduced,
            ThemeMode = settings.ThemeMode,
            MotionProfile = motionProfile,
        };
    }

    private void TrySaveRepair(LauncherSettings repaired)
    {
        try
        {
            Save(repaired);
        }
        catch (IOException)
        {
        }
        catch (UnauthorizedAccessException)
        {
        }
    }
}
