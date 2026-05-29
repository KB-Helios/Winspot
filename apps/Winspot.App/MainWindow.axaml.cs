using System.ComponentModel;

using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Threading;

using Winspot_App.Services;
using Winspot_App.ViewModels;

namespace Winspot_App;

public sealed partial class MainWindow : Window
{
    private const double CompactHeight = 104;
    private const double ExpandedHeight = 560;
    private CancellationTokenSource? _heightAnimationCancellation;
    private GlobalHotkeyService? _hotkeyService;

    public MainWindow()
    {
        ViewModel = new LauncherViewModel(new WinspotIpcClient());
        DataContext = ViewModel;
        InitializeComponent();

        Height = CompactHeight;
        ViewModel.PropertyChanged += OnViewModelPropertyChanged;
        Opened += OnOpened;
        Closing += OnClosing;
        KeyDown += OnKeyDown;
    }

    public LauncherViewModel ViewModel { get; }

    public void FocusSearch()
    {
        SearchBox.Focus(NavigationMethod.Unspecified);
    }

    public void Reveal()
    {
        SpotlightSurface.Classes.Add("staging");
        _ = Dispatcher.UIThread.InvokeAsync(() =>
        {
            SpotlightSurface.Classes.Remove("staging");
            FocusSearch();
        }, DispatcherPriority.Background);
    }

    private async void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(LauncherViewModel.IsExpanded))
        {
            await AnimateHeightAsync(ViewModel.IsExpanded ? ExpandedHeight : CompactHeight);
        }
    }

    private void OnOpened(object? sender, EventArgs e)
    {
        Reveal();
        _ = Dispatcher.UIThread.InvokeAsync(async () =>
        {
            await Task.Delay(400).ConfigureAwait(true);
            RegisterHotkey();
        }, DispatcherPriority.Background);
    }

    private void OnClosing(object? sender, WindowClosingEventArgs e)
    {
        _heightAnimationCancellation?.Cancel();
        _heightAnimationCancellation?.Dispose();
        ViewModel.PropertyChanged -= OnViewModelPropertyChanged;
        if (_hotkeyService is not null)
        {
            _hotkeyService.Pressed -= OnGlobalHotkeyPressed;
            _hotkeyService.Dispose();
        }
    }

    private async void OnKeyDown(object? sender, KeyEventArgs e)
    {
        if (e.Key == Key.Escape)
        {
            Hide();
            e.Handled = true;
            return;
        }

        if (e.Key != Key.Enter)
        {
            return;
        }

        e.Handled = true;
        await ViewModel.ExecuteSelectedAsync();
    }

    private void OnGlobalHotkeyPressed(object? sender, EventArgs e)
    {
        Dispatcher.UIThread.Post(() =>
        {
            if (IsVisible && IsActive)
            {
                Hide();
                return;
            }

            Show();
            Activate();
            Reveal();
        });
    }

    private async Task AnimateHeightAsync(double targetHeight)
    {
        var previous = _heightAnimationCancellation;
        previous?.Cancel();
        previous?.Dispose();

        var cancellation = new CancellationTokenSource();
        _heightAnimationCancellation = cancellation;

        try
        {
            var startHeight = Height;
            const int frames = 14;
            for (var frame = 1; frame <= frames; frame++)
            {
                if (cancellation.IsCancellationRequested)
                {
                    return;
                }

                var progress = frame / (double)frames;
                var eased = 1 - Math.Pow(1 - progress, 3);
                Height = startHeight + ((targetHeight - startHeight) * eased);
                await Task.Delay(16, cancellation.Token).ConfigureAwait(true);
            }

            Height = targetHeight;
        }
        catch (OperationCanceledException)
        {
        }
    }

    private void RegisterHotkey()
    {
        if (_hotkeyService is not null)
        {
            return;
        }

        var handle = GetWindowHandle();
        if (handle == 0)
        {
            return;
        }

        var hotkeyService = new GlobalHotkeyService(handle);
        hotkeyService.Pressed += OnGlobalHotkeyPressed;
        if (hotkeyService.Register(new LauncherSettingsStore().Load().Hotkey))
        {
            _hotkeyService = hotkeyService;
            return;
        }

        hotkeyService.Pressed -= OnGlobalHotkeyPressed;
        hotkeyService.Dispose();
    }

    private nint GetWindowHandle()
    {
        try
        {
            return TryGetPlatformHandle()?.Handle ?? 0;
        }
        catch
        {
            return 0;
        }
    }
}
