namespace Winspot_App.Models;

public sealed class LauncherSettings
{
    public HotkeyBinding Hotkey { get; init; } = new();
}
