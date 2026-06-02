namespace Winspot_App.Services;

public static class AppPaths
{
    private const string PortableMarkerFileName = "Winspot.portable";
    private const string AppDataFolderName = "Winspot";

    public static bool IsPortable(string baseDirectory) =>
        File.Exists(Path.Combine(baseDirectory, PortableMarkerFileName));

    public static string ResolveSettingsPath(string baseDirectory, string? localAppData)
    {
        if (IsPortable(baseDirectory))
        {
            return Path.Combine(baseDirectory, "data", "settings.json");
        }

        return Path.Combine(ResolveLocalAppData(localAppData), AppDataFolderName, "settings.json");
    }

    public static string ResolvePluginsPath(string baseDirectory, string? localAppData)
    {
        if (IsPortable(baseDirectory))
        {
            return Path.Combine(baseDirectory, "plugins");
        }

        return Path.Combine(ResolveLocalAppData(localAppData), AppDataFolderName, "plugins");
    }

    private static string ResolveLocalAppData(string? localAppData)
    {
        if (!string.IsNullOrWhiteSpace(localAppData))
        {
            return localAppData;
        }

        var userProfile = Environment.GetEnvironmentVariable("USERPROFILE");
        if (!string.IsNullOrWhiteSpace(userProfile))
        {
            return Path.Combine(userProfile, "AppData", "Local");
        }

        return Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
    }
}
