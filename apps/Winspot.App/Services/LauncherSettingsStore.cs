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
            var shouldPersistRepair = false;
            LauncherSettings settings;
            try
            {
                settings = NormalizeSettings(JsonSerializer.Deserialize<LauncherSettings>(json, JsonOptions) ?? new LauncherSettings());
            }
            catch (JsonException)
            {
                settings = RecoverSettingsFromJson(json) ?? new LauncherSettings();
                shouldPersistRepair = true;
            }

            if (NeedsHotkeyRepair(settings.Hotkey))
            {
                settings = RepairHotkey(settings);
                shouldPersistRepair = true;
            }

            if (shouldPersistRepair)
            {
                TrySaveRepair(settings);
            }

            return settings;
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

    private static LauncherSettings? RecoverSettingsFromJson(string json)
    {
        try
        {
            using var document = JsonDocument.Parse(json);
            var root = document.RootElement;
            if (root.ValueKind != JsonValueKind.Object)
            {
                return null;
            }

            var defaults = new LauncherSettings();
            var recovered = new LauncherSettings
            {
                Hotkey = ReadHotkey(root) ?? defaults.Hotkey,
                LaunchOnStartup = ReadBoolean(root, "launchOnStartup", defaults.LaunchOnStartup),
                ShowTrayIcon = ReadBoolean(root, "showTrayIcon", defaults.ShowTrayIcon),
                ReduceMotion = ReadBoolean(root, "reduceMotion", defaults.ReduceMotion),
                ThemeMode = ReadEnum(root, "themeMode", defaults.ThemeMode),
                MotionProfile = ReadEnum(root, "motionProfile", defaults.MotionProfile),
            };

            return NormalizeSettings(recovered);
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static HotkeyBinding? ReadHotkey(JsonElement root)
    {
        if (!root.TryGetProperty("hotkey", out var hotkey) || hotkey.ValueKind != JsonValueKind.Object)
        {
            return null;
        }

        var key = hotkey.TryGetProperty("key", out var keyElement) && keyElement.ValueKind == JsonValueKind.String
            ? keyElement.GetString() ?? string.Empty
            : string.Empty;
        var modifiers = new List<string>();
        if (hotkey.TryGetProperty("modifiers", out var modifiersElement)
            && modifiersElement.ValueKind == JsonValueKind.Array)
        {
            modifiers.AddRange(
                modifiersElement
                    .EnumerateArray()
                    .Where(element => element.ValueKind == JsonValueKind.String)
                    .Select(element => element.GetString() ?? string.Empty));
        }

        return new HotkeyBinding
        {
            Key = key,
            Modifiers = modifiers,
        };
    }

    private static bool ReadBoolean(JsonElement root, string propertyName, bool fallback) =>
        root.TryGetProperty(propertyName, out var value) && value.ValueKind is JsonValueKind.True or JsonValueKind.False
            ? value.GetBoolean()
            : fallback;

    private static TEnum ReadEnum<TEnum>(JsonElement root, string propertyName, TEnum fallback)
        where TEnum : struct, Enum
    {
        if (!root.TryGetProperty(propertyName, out var value))
        {
            return fallback;
        }

        if (value.ValueKind == JsonValueKind.String
            && Enum.TryParse<TEnum>(value.GetString(), ignoreCase: true, out var parsed))
        {
            return parsed;
        }

        if (value.ValueKind == JsonValueKind.Number
            && value.TryGetInt32(out var numeric)
            && Enum.IsDefined(typeof(TEnum), numeric))
        {
            return (TEnum)Enum.ToObject(typeof(TEnum), numeric);
        }

        return fallback;
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
