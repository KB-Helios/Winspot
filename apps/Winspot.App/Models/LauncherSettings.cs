namespace Winspot_App.Models;

public enum ThemeMode
{
    System,
    Dark,
    Light,
}

public enum MotionProfile
{
    Reduced,
    Snappy240,
}

public sealed class LauncherSettings
{
    public HotkeyBinding Hotkey { get; init; } = new();

    /// Whether Winspot registers itself to start when the user signs in.
    public bool LaunchOnStartup { get; init; }

    /// Whether the launcher keeps a notification-area (system tray) icon.
    public bool ShowTrayIcon { get; init; } = true;

    /// When true, the launcher skips reveal/resize/transition animations and
    /// snaps directly to the final state.
    public bool ReduceMotion { get; init; }

    /// Theme preference for the Avalonia application. Dark remains the default
    /// so existing installs keep today's look until the user opts into system
    /// or light theme behavior.
    public ThemeMode ThemeMode { get; init; } = ThemeMode.Dark;

    /// Motion profile for launcher reveal/expansion. Snappy240 means Winspot
    /// avoids artificial 60Hz waits; actual FPS is determined by the display
    /// and compositor.
    public MotionProfile MotionProfile { get; init; } = MotionProfile.Snappy240;
}
