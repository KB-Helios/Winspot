using Avalonia.Controls;
using Avalonia.Markup.Xaml;

namespace Winspot_App.Controls;

/// A small, dependency-free loading spinner used while the launcher streams
/// search results and while settings validates plugins. The continuous rotation
/// is declared in XAML and runs on the render thread; the control draws nothing
/// expensive, so it stays cheap even at high refresh rates.
public sealed partial class SearchSpinner : UserControl
{
    public SearchSpinner()
    {
        InitializeComponent();
    }

    private void InitializeComponent()
    {
        AvaloniaXamlLoader.Load(this);
    }
}
