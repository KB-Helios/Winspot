using Avalonia;

using Winspot_App.Services;

namespace Winspot_App;

internal static class Program
{
    [STAThread]
    public static void Main(string[] args)
    {
        // Install global crash handlers before anything else so failures during
        // startup are still recorded to the log file.
        AppLog.Init();
        try
        {
            BuildAvaloniaApp().StartWithClassicDesktopLifetime(args);
        }
        catch (Exception exception)
        {
            AppLog.Error("Program.Main", exception);
            throw;
        }
    }

    public static AppBuilder BuildAvaloniaApp()
    {
        return AppBuilder.Configure<App>()
            .UsePlatformDetect()
            .WithInterFont()
            .LogToTrace();
    }
}
