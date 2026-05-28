using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;

using Winspot_App.Models;
using Winspot_App.Services;
using Winspot_App.ViewModels;
using Windows.System;

namespace Winspot_App;

public sealed partial class MainPage : Page
{
    private readonly ShellIconProvider _shellIconProvider = new();

    public MainPage()
    {
        ViewModel = new LauncherViewModel(new WinspotIpcClient());
        InitializeComponent();
        Loaded += OnLoaded;
    }

    public LauncherViewModel ViewModel { get; }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        SearchBox.Focus(FocusState.Programmatic);
    }

    private async void OnLauncherKeyDown(object sender, KeyRoutedEventArgs e)
    {
        if (e.Key != VirtualKey.Enter)
        {
            return;
        }

        e.Handled = true;
        await ViewModel.ExecuteSelectedAsync();
    }

    private async void OnResultIconImageLoaded(object sender, RoutedEventArgs e)
    {
        if (sender is not Image image || image.DataContext is not SearchResultItem result)
        {
            return;
        }

        image.Source = null;
        var iconPath = result.IconPath;
        var icon = await _shellIconProvider.GetIconAsync(iconPath);
        if (ReferenceEquals(image.DataContext, result))
        {
            image.Source = icon;
        }
    }
}
