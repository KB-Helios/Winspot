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

    /// <summary>
    /// Create a copy of the provided settings with the hotkey replaced by a default (cleared) binding.
    /// </summary>
    /// <param name="settings">Source settings whose non-hotkey fields will be preserved.</param>
    /// <returns>A new <see cref="LauncherSettings"/> with <see cref="LauncherSettings.Hotkey"/> set to a default <see cref="HotkeyBinding"/>; other fields are copied from <paramref name="settings"/> and <see cref="LauncherSettings.FastFlowLm"/> is normalized.</returns>
    private static LauncherSettings RepairHotkey(LauncherSettings settings) => new()
    {
        Hotkey = new HotkeyBinding(),
        LaunchOnStartup = settings.LaunchOnStartup,
        ShowTrayIcon = settings.ShowTrayIcon,
        ReduceMotion = settings.ReduceMotion,
        ThemeMode = settings.ThemeMode,
        MotionProfile = settings.MotionProfile,
        FastFlowLm = NormalizeFastFlowLm(settings.FastFlowLm),
    };

    /// <summary>
    /// Produce a normalized copy of launcher settings with a consistent motion profile and normalized FastFlowLm settings.
    /// </summary>
    /// <param name="settings">The source settings to normalize; values are copied into the returned instance with adjustments.</param>
    /// <returns>A new <see cref="LauncherSettings"/> with motion-related fields made consistent and <see cref="FastFlowLmSettings"/> normalized.</returns>
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
            FastFlowLm = NormalizeFastFlowLm(settings.FastFlowLm),
        };
    }

    /// <summary>
    /// Attempts to reconstruct LauncherSettings from a raw JSON document.
    /// </summary>
    /// <param name="json">The raw JSON text containing persisted settings.</param>
    /// <returns>A normalized <see cref="LauncherSettings"/> built from the JSON, or <c>null</c> if the JSON is invalid or the root element is not an object.</returns>
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
                FastFlowLm = ReadFastFlowLm(root, defaults.FastFlowLm),
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

    /// <summary>
            /// Reads a boolean property named <paramref name="propertyName"/> from the given JSON object and returns a fallback when the property is missing or not a boolean.
            /// </summary>
            /// <param name="root">The JSON object to read from.</param>
            /// <param name="propertyName">The property name to read.</param>
            /// <param name="fallback">Value to return when the property is missing or not a boolean.</param>
            /// <returns>`true` if the property exists and is `true`; `false` if the property exists and is `false`; otherwise returns <paramref name="fallback"/>.</returns>
            private static bool ReadBoolean(JsonElement root, string propertyName, bool fallback) =>
        root.TryGetProperty(propertyName, out var value) && value.ValueKind is JsonValueKind.True or JsonValueKind.False
            ? value.GetBoolean()
            : fallback;

    /// <summary>
    /// Normalize a FastFlowLmSettings instance to ensure sensible defaults and valid field values.
    /// </summary>
    /// <remarks>
    /// If <c>settings</c> is <c>null</c>, a new default instance is returned. String fields are trimmed and replaced with defaults when empty or whitespace. Numeric fields are validated: <c>Port</c> must be between 1 and 65535; other numeric limits (timeout and size/count values) must be greater than zero. Invalid numeric values are replaced with their defaults.
    /// </remarks>
    /// <returns>
    /// A validated <see cref="FastFlowLmSettings"/> instance with trimmed strings and defaults substituted for missing or out-of-range values.
    /// </returns>
    private static FastFlowLmSettings NormalizeFastFlowLm(FastFlowLmSettings? settings)
    {
        var defaults = new FastFlowLmSettings();
        if (settings is null)
        {
            return defaults;
        }

        return new FastFlowLmSettings
        {
            Enabled = settings.Enabled,
            ModelTag = string.IsNullOrWhiteSpace(settings.ModelTag)
                ? defaults.ModelTag
                : settings.ModelTag.Trim(),
            ExecutablePath = string.IsNullOrWhiteSpace(settings.ExecutablePath)
                ? defaults.ExecutablePath
                : settings.ExecutablePath.Trim(),
            Port = settings.Port is > 0 and <= 65535 ? settings.Port : defaults.Port,
            IdleTimeoutSeconds = settings.IdleTimeoutSeconds > 0
                ? settings.IdleTimeoutSeconds
                : defaults.IdleTimeoutSeconds,
            MaxContextFiles = settings.MaxContextFiles > 0
                ? settings.MaxContextFiles
                : defaults.MaxContextFiles,
            MaxFileBytes = settings.MaxFileBytes > 0
                ? settings.MaxFileBytes
                : defaults.MaxFileBytes,
            MaxContextBytes = settings.MaxContextBytes > 0
                ? settings.MaxContextBytes
                : defaults.MaxContextBytes,
        };
    }

    /// <summary>
    /// Extracts the "fastFlowLm" object from a JSON element and returns a normalized FastFlowLmSettings instance.
    /// </summary>
    /// <param name="root">JSON element that may contain a "fastFlowLm" property.</param>
    /// <param name="fallback">Default FastFlowLmSettings used when the property or individual fields are missing or invalid.</param>
    /// <returns>A normalized FastFlowLmSettings built from the "fastFlowLm" object, or the provided <paramref name="fallback"/> if the property is absent or not an object.</returns>
    private static FastFlowLmSettings ReadFastFlowLm(JsonElement root, FastFlowLmSettings fallback)
    {
        if (!root.TryGetProperty("fastFlowLm", out var settings) || settings.ValueKind != JsonValueKind.Object)
        {
            return fallback;
        }

        return NormalizeFastFlowLm(new FastFlowLmSettings
        {
            Enabled = ReadBoolean(settings, "enabled", fallback.Enabled),
            ModelTag = ReadString(settings, "modelTag", fallback.ModelTag),
            ExecutablePath = ReadString(settings, "executablePath", fallback.ExecutablePath),
            Port = ReadInteger(settings, "port", fallback.Port),
            IdleTimeoutSeconds = ReadInteger(settings, "idleTimeoutSeconds", fallback.IdleTimeoutSeconds),
            MaxContextFiles = ReadInteger(settings, "maxContextFiles", fallback.MaxContextFiles),
            MaxFileBytes = ReadInteger(settings, "maxFileBytes", fallback.MaxFileBytes),
            MaxContextBytes = ReadInteger(settings, "maxContextBytes", fallback.MaxContextBytes),
        });
    }

    /// <summary>
            /// Get the string value of a named property from a JsonElement, falling back to the provided default when the property is missing or not a JSON string.
            /// </summary>
            /// <param name="root">The JSON element to read from.</param>
            /// <param name="propertyName">The property name to look up.</param>
            /// <param name="fallback">The value to return when the property is missing or not a string.</param>
            /// <returns>The property's string value, or <c>fallback</c> if the property is absent or not a JSON string.</returns>
            private static string ReadString(JsonElement root, string propertyName, string fallback) =>
        root.TryGetProperty(propertyName, out var value) && value.ValueKind == JsonValueKind.String
            ? value.GetString() ?? fallback
            : fallback;

    /// <summary>
            /// Reads a 32-bit integer property from the given JSON element, returning a fallback value when the property is missing or not a valid 32-bit number.
            /// </summary>
            /// <param name="root">The JSON element to read from.</param>
            /// <param name="propertyName">The property name to look up on <paramref name="root"/>.</param>
            /// <param name="fallback">Value to return when the property is absent or not a valid int.</param>
            /// <returns>The property's int value if present and valid; otherwise <paramref name="fallback"/>.</returns>
            private static int ReadInteger(JsonElement root, string propertyName, int fallback) =>
        root.TryGetProperty(propertyName, out var value)
            && value.ValueKind == JsonValueKind.Number
            && value.TryGetInt32(out var number)
            ? number
            : fallback;

    /// <summary>
    /// Reads an enum value from a JSON object property and returns a fallback when the property is missing or invalid.
    /// </summary>
    /// <param name="root">The JSON object to read from.</param>
    /// <param name="propertyName">The property name to read.</param>
    /// <param name="fallback">The value to return when the property is missing or cannot be parsed to the enum.</param>
    /// <returns>The parsed enum value if the property is a recognized string or numeric representation; otherwise <paramref name="fallback"/>.</returns>
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
