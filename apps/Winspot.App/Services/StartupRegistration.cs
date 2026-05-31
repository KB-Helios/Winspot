using System.Diagnostics;
using System.Runtime.Versioning;

using Microsoft.Win32;

namespace Winspot_App.Services;

/// Registers (or removes) Winspot in the per-user "Run" key so it can launch
/// automatically when the user signs in. All operations are best-effort: a
/// failure to read or write the registry must never crash the launcher.
internal static class StartupRegistration
{
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "Winspot";

    public static void Apply(bool launchOnStartup)
    {
        if (!OperatingSystem.IsWindows())
        {
            return;
        }

        try
        {
            if (launchOnStartup)
            {
                Enable();
            }
            else
            {
                Disable();
            }
        }
        catch
        {
            // Startup registration is a convenience; ignore registry failures.
        }
    }

    [SupportedOSPlatform("windows")]
    private static void Enable()
    {
        var executablePath = Process.GetCurrentProcess().MainModule?.FileName;
        if (string.IsNullOrWhiteSpace(executablePath))
        {
            return;
        }

        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true)
            ?? Registry.CurrentUser.CreateSubKey(RunKeyPath);
        key?.SetValue(ValueName, $"\"{executablePath}\"");
    }

    [SupportedOSPlatform("windows")]
    private static void Disable()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true);
        if (key?.GetValue(ValueName) is not null)
        {
            key.DeleteValue(ValueName, throwOnMissingValue: false);
        }
    }
}
