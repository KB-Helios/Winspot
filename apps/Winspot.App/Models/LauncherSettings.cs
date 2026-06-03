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

    /// FastFlowLM local server integration. The daemon reads this same
    /// `fastFlowLm` object from settings.json and does not start FLM unless the
    /// user executes an AI result.
    public FastFlowLmSettings FastFlowLm { get; init; } = new();

    /// Windows Capture integration. The daemon reads this same `capture`
    /// object and saves capture artifacts to the configured output directory.
    public CaptureSettings Capture { get; init; } = new();
}

public sealed class FastFlowLmSettings
{
    public bool Enabled { get; init; } = true;

    public string ModelTag { get; init; } = "gemma4-it:e2b";

    public string ExecutablePath { get; init; } = "flm";

    public int Port { get; init; } = 52625;

    public int IdleTimeoutSeconds { get; init; } = 120;

    public int MaxContextFiles { get; init; } = 5;

    public int MaxFileBytes { get; init; } = 1024 * 1024;

    public int MaxContextBytes { get; init; } = 4 * 1024 * 1024;
}

public sealed class CaptureSettings
{
    public bool Enabled { get; init; } = true;

    public string OutputDirectory { get; init; } = string.Empty;

    public int DefaultRecordSeconds { get; init; } = 8;

    public int MaxRecordSeconds { get; init; } = 60;

    public bool IncludeCursor { get; init; } = true;

    public int PreCaptureDelayMs { get; init; } = 250;
}
