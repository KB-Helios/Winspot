namespace Winspot_App.Models;

public sealed class LauncherSettings
{
    public HotkeyBinding Hotkey { get; init; } = new();

    /// Whether Winspot registers itself to start when the user signs in.
    public bool LaunchOnStartup { get; init; }

    /// Whether the launcher keeps a notification-area (system tray) icon.
    public bool ShowTrayIcon { get; init; } = true;
}
