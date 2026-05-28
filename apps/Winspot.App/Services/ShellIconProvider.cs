using System.Collections.Concurrent;

using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Storage;
using Windows.Storage.FileProperties;

namespace Winspot_App.Services;

internal sealed class ShellIconProvider
{
    private const int ResultIconSize = 28;
    private readonly ConcurrentDictionary<string, Task<ImageSource?>> _iconCache = new(StringComparer.OrdinalIgnoreCase);

    public Task<ImageSource?> GetIconAsync(string? path)
    {
        if (string.IsNullOrWhiteSpace(path))
        {
            return Task.FromResult<ImageSource?>(null);
        }

        return _iconCache.GetOrAdd(path, LoadIconAsync);
    }

    private static async Task<ImageSource?> LoadIconAsync(string path)
    {
        try
        {
            if (!File.Exists(path))
            {
                return null;
            }

            var file = await StorageFile.GetFileFromPathAsync(path);
            using var thumbnail = await file.GetThumbnailAsync(
                ThumbnailMode.SingleItem,
                ResultIconSize,
                ThumbnailOptions.UseCurrentScale);

            if (thumbnail is null || thumbnail.Size == 0)
            {
                return null;
            }

            var bitmap = new BitmapImage
            {
                DecodePixelHeight = ResultIconSize,
                DecodePixelWidth = ResultIconSize,
            };

            await bitmap.SetSourceAsync(thumbnail);
            return bitmap;
        }
        catch
        {
            return null;
        }
    }
}
