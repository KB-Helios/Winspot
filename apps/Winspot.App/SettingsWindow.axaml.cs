using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Markup.Xaml;

using Winspot_App.ViewModels;

namespace Winspot_App;

public sealed partial class SettingsWindow : Window
{
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
        await ViewModel.OpenPluginsFolderAsync(CancellationToken.None);
    }

    private async void OnValidatePluginsClick(object? sender, RoutedEventArgs e)
    {
        await ViewModel.RefreshPluginValidationAsync(CancellationToken.None);
    }

    private async void OnOpened(object? sender, EventArgs e)
    {
        await ViewModel.RefreshPluginValidationAsync(CancellationToken.None);
    }
}
