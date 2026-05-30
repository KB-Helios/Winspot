using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;

using Winspot_App.Services;

namespace Winspot_App;

public sealed partial class App : Application
{
    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            desktop.MainWindow = new MainWindow();
            desktop.ShutdownRequested += (_, _) => WinspotIpcClient.StopBackendIfOwned();
            desktop.Exit += (_, _) => WinspotIpcClient.StopBackendIfOwned();
        }

        base.OnFrameworkInitializationCompleted();
    }
}
