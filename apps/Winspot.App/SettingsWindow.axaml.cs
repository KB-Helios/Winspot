using System.Threading;
using System.Threading.Tasks;

using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Markup.Xaml;

using Winspot_App.Services;
using Winspot_App.ViewModels;

namespace Winspot_App;

public sealed partial class SettingsWindow : Window
{
    private CancellationTokenSource? _pluginValidationCancellation;

    public SettingsWindow()
        : this(new SettingsViewModel())
    {
    }

    public SettingsWindow(SettingsViewModel viewModel)
    {
        ViewModel = viewModel;
        DataContext = viewModel;
        InitializeComponent();
        Opened += OnOpened;
        Closed += OnClosed;
    }

    public SettingsViewModel ViewModel { get; }

    private void InitializeComponent()
    {
        AvaloniaXamlLoader.Load(this);
    }

    private void OnSaveClick(object? sender, RoutedEventArgs e)
    {
        if (ViewModel.TrySave()
            && !ViewModel.StatusMessage.Contains("unavailable", StringComparison.OrdinalIgnoreCase))
        {
            Close();
        }
    }

    private void OnCancelClick(object? sender, RoutedEventArgs e)
    {
        Close();
    }

    private async void OnOpenPluginsFolderClick(object? sender, RoutedEventArgs e)
    {
        try
        {
            await ViewModel.OpenPluginsFolderAsync(CancellationToken.None);
        }
        catch (Exception exception)
        {
            AppLog.Error(nameof(OnOpenPluginsFolderClick), exception);
        }
    }

    private async void OnValidatePluginsClick(object? sender, RoutedEventArgs e)
    {
        await RefreshPluginValidationAsync();
    }

    private async void OnOpened(object? sender, EventArgs e)
    {
        await RefreshPluginValidationAsync();
    }

    private async Task RefreshPluginValidationAsync()
    {
        var cancellation = ResetPluginValidationCancellation();
        try
        {
            await ViewModel.RefreshPluginValidationAsync(cancellation.Token);
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception exception)
        {
            AppLog.Error(nameof(RefreshPluginValidationAsync), exception);
        }
    }

    private CancellationTokenSource ResetPluginValidationCancellation()
    {
        var previous = _pluginValidationCancellation;
        previous?.Cancel();
        _pluginValidationCancellation = new CancellationTokenSource();
        previous?.Dispose();
        return _pluginValidationCancellation;
    }

    private void OnClosed(object? sender, EventArgs e)
    {
        _pluginValidationCancellation?.Cancel();
        _pluginValidationCancellation?.Dispose();
        _pluginValidationCancellation = null;
    }
}
