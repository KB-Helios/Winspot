using System.Runtime.InteropServices;

using Microsoft.UI.Xaml;

using Winspot_App.Services;

// To learn more about WinUI, the WinUI project structure,
// and more about our project templates, see: http://aka.ms/winui-project-info.

namespace Winspot_App;

/// <summary>
/// The application window. This hosts a Frame that displays pages. Add your
/// UI and logic to MainPage.xaml / MainPage.xaml.cs instead of here so you
/// can use Page features such as navigation events and the Loaded lifecycle.
/// </summary>
public sealed partial class MainWindow : Window
{
    private const int SwHide = 0;
    private const int SwRestore = 9;
    private const int SwShowNormal = 1;

    private readonly nint _windowHandle;
    private readonly GlobalHotkeyService _hotkeyService;

    public MainWindow()
    {
        InitializeComponent();
        _windowHandle = WinRT.Interop.WindowNative.GetWindowHandle(this);

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);

        AppWindow.SetIcon("Assets/AppIcon.ico");

        // Navigate the root frame to the main page on startup.
        RootFrame.Navigate(typeof(MainPage));

        var settings = new LauncherSettingsStore().Load();
        _hotkeyService = new GlobalHotkeyService(_windowHandle);
        _hotkeyService.Pressed += OnGlobalHotkeyPressed;
        _hotkeyService.Register(settings.Hotkey);
        Closed += OnClosed;
    }

    private void OnClosed(object sender, WindowEventArgs args)
    {
        _hotkeyService.Pressed -= OnGlobalHotkeyPressed;
        _hotkeyService.Dispose();
    }

    private void OnGlobalHotkeyPressed(object? sender, EventArgs e)
    {
        if (IsWindowVisible(_windowHandle) && GetForegroundWindow() == _windowHandle)
        {
            ShowWindow(_windowHandle, SwHide);
            return;
        }

        ShowWindow(_windowHandle, IsIconic(_windowHandle) ? SwRestore : SwShowNormal);
        Activate();
        SetForegroundWindow(_windowHandle);
        if (RootFrame.Content is MainPage mainPage)
        {
            mainPage.FocusSearch();
        }
    }

    [DllImport("user32.dll")]
    private static extern bool ShowWindow(nint hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    private static extern bool SetForegroundWindow(nint hWnd);

    [DllImport("user32.dll")]
    private static extern nint GetForegroundWindow();

    [DllImport("user32.dll")]
    private static extern bool IsWindowVisible(nint hWnd);

    [DllImport("user32.dll")]
    private static extern bool IsIconic(nint hWnd);
}
