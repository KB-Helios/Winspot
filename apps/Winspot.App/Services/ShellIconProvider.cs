using System.Collections.Concurrent;
using System.Runtime.InteropServices;

using Avalonia;
using Avalonia.Media.Imaging;
using Avalonia.Platform;

namespace Winspot_App.Services;

/// Loads the native Windows shell icon for a file-system path (app shortcut,
/// file, or folder) as an Avalonia <see cref="Bitmap"/>. Results are cached by
/// path so the result list can request the same icon on every keystroke without
/// re-hitting the shell. Returns <c>null</c> off Windows or when no icon is
/// available, letting callers fall back to a font glyph.
public sealed class ShellIconProvider
{
    public static ShellIconProvider Shared { get; } = new();

    private readonly ConcurrentDictionary<string, Bitmap?> _cache = new(StringComparer.OrdinalIgnoreCase);

    public Bitmap? TryLoad(string? path)
    {
        if (string.IsNullOrWhiteSpace(path) || !OperatingSystem.IsWindows())
        {
            return null;
        }

        return _cache.GetOrAdd(path, LoadFromShell);
    }

    private static Bitmap? LoadFromShell(string path)
    {
        try
        {
            var info = default(SHFILEINFO);
            var result = SHGetFileInfo(
                path,
                0,
                ref info,
                (uint)Marshal.SizeOf<SHFILEINFO>(),
                SHGFI_ICON | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES);

            if (result == IntPtr.Zero || info.hIcon == IntPtr.Zero)
            {
                return null;
            }

            try
            {
                return IconToBitmap(info.hIcon);
            }
            finally
            {
                DestroyIcon(info.hIcon);
            }
        }
        catch
        {
            return null;
        }
    }

    private static Bitmap? IconToBitmap(IntPtr hIcon)
    {
        if (!GetIconInfo(hIcon, out var iconInfo))
        {
            return null;
        }

        var colorBitmap = iconInfo.hbmColor;
        var maskBitmap = iconInfo.hbmMask;
        try
        {
            if (colorBitmap == IntPtr.Zero)
            {
                return null;
            }

            var bitmap = default(BITMAP);
            if (GetObject(colorBitmap, Marshal.SizeOf<BITMAP>(), ref bitmap) == 0)
            {
                return null;
            }

            var width = bitmap.bmWidth;
            var height = bitmap.bmHeight;
            if (width <= 0 || height <= 0)
            {
                return null;
            }

            var header = new BITMAPINFO
            {
                biSize = (uint)Marshal.SizeOf<BITMAPINFO>(),
                biWidth = width,
                biHeight = -height, // top-down so rows are in display order
                biPlanes = 1,
                biBitCount = 32,
                biCompression = BI_RGB,
            };

            var byteCount = width * height * 4;
            var buffer = Marshal.AllocHGlobal(byteCount);
            var screenDc = GetDC(IntPtr.Zero);
            try
            {
                if (screenDc == IntPtr.Zero
                    || GetDIBits(screenDc, colorBitmap, 0, (uint)height, buffer, ref header, DIB_RGB_COLORS) == 0)
                {
                    return null;
                }

                var managed = new byte[byteCount];
                Marshal.Copy(buffer, managed, 0, byteCount);

                var writeable = new WriteableBitmap(
                    new PixelSize(width, height),
                    new Vector(96, 96),
                    PixelFormat.Bgra8888,
                    AlphaFormat.Unpremul);

                using var frame = writeable.Lock();
                var sourceStride = width * 4;
                for (var row = 0; row < height; row++)
                {
                    Marshal.Copy(
                        managed,
                        row * sourceStride,
                        frame.Address + (row * frame.RowBytes),
                        sourceStride);
                }

                return writeable;
            }
            finally
            {
                if (screenDc != IntPtr.Zero)
                {
                    ReleaseDC(IntPtr.Zero, screenDc);
                }

                Marshal.FreeHGlobal(buffer);
            }
        }
        finally
        {
            if (colorBitmap != IntPtr.Zero)
            {
                DeleteObject(colorBitmap);
            }

            if (maskBitmap != IntPtr.Zero)
            {
                DeleteObject(maskBitmap);
            }
        }
    }

    private const uint SHGFI_ICON = 0x000000100;
    private const uint SHGFI_SMALLICON = 0x000000001;
    private const uint SHGFI_USEFILEATTRIBUTES = 0x000000010;
    private const uint BI_RGB = 0;
    private const uint DIB_RGB_COLORS = 0;

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct SHFILEINFO
    {
        public IntPtr hIcon;
        public int iIcon;
        public uint dwAttributes;

        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)]
        public string szDisplayName;

        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 80)]
        public string szTypeName;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct ICONINFO
    {
        public bool fIcon;
        public int xHotspot;
        public int yHotspot;
        public IntPtr hbmMask;
        public IntPtr hbmColor;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct BITMAP
    {
        public int bmType;
        public int bmWidth;
        public int bmHeight;
        public int bmWidthBytes;
        public ushort bmPlanes;
        public ushort bmBitsPixel;
        public IntPtr bmBits;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct BITMAPINFO
    {
        public uint biSize;
        public int biWidth;
        public int biHeight;
        public ushort biPlanes;
        public ushort biBitCount;
        public uint biCompression;
        public uint biSizeImage;
        public int biXPelsPerMeter;
        public int biYPelsPerMeter;
        public uint biClrUsed;
        public uint biClrImportant;
        public uint biColors;
    }

    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    private static extern IntPtr SHGetFileInfo(
        string pszPath,
        uint dwFileAttributes,
        ref SHFILEINFO psfi,
        uint cbFileInfo,
        uint uFlags);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetIconInfo(IntPtr hIcon, out ICONINFO piconinfo);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DestroyIcon(IntPtr hIcon);

    [DllImport("user32.dll")]
    private static extern IntPtr GetDC(IntPtr hWnd);

    [DllImport("user32.dll")]
    private static extern int ReleaseDC(IntPtr hWnd, IntPtr hDC);

    [DllImport("gdi32.dll")]
    private static extern int GetObject(IntPtr hgdiobj, int cbBuffer, ref BITMAP lpvObject);

    [DllImport("gdi32.dll")]
    private static extern int GetDIBits(
        IntPtr hdc,
        IntPtr hbmp,
        uint uStartScan,
        uint cScanLines,
        IntPtr lpvBits,
        ref BITMAPINFO lpbi,
        uint uUsage);

    [DllImport("gdi32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool DeleteObject(IntPtr hObject);
}
