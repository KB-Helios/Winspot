using System.Globalization;

using Avalonia.Data.Converters;
using Avalonia.Media.Imaging;

using Winspot_App.Services;

namespace Winspot_App.Converters;

/// Resolves a file-system path (bound from <c>SearchResultItem.IconPath</c>) to
/// the native shell icon. Returns <c>null</c> when no path/icon is available so
/// the glyph fallback in the result template shows instead.
public sealed class IconPathToBitmapConverter : IValueConverter
{
    public static IconPathToBitmapConverter Instance { get; } = new();

    public object? Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
        => value is string path ? ShellIconProvider.Shared.TryLoad(path) : null;

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
        => throw new NotSupportedException();
}
