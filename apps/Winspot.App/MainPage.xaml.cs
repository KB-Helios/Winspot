using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

using Winspot_App.Services;
using Winspot_App.ViewModels;

namespace Winspot_App;

public sealed partial class MainPage : Page
{
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
}
