using Avalonia;

namespace Winspot_App;

internal static class LauncherWindowLayout
{
    public const double ShadowMargin = 64;
    public const double CompactSurfaceHeight = 60;
    public const double ExpandedSurfaceHeight = 560;
    public const double CompactHeight = CompactSurfaceHeight + (ShadowMargin * 2);
    public const double ExpandedHeight = ExpandedSurfaceHeight + (ShadowMargin * 2);

    private const double MinimumSurfaceWidth = 420;
    private const double MaximumSurfaceWidth = 760;
    private const double HorizontalBreathingRoom = 48;
    private const double VerticalBreathingRoom = 24;
    private const double VerticalCenterBias = 0.44;

    public static Rect CalculateBounds(Rect workingArea, bool isExpanded)
    {
        var availableSurfaceWidth = Math.Max(1, workingArea.Width - HorizontalBreathingRoom - (ShadowMargin * 2));
        var preferredSurfaceWidth = Math.Clamp(
            workingArea.Width - HorizontalBreathingRoom,
            MinimumSurfaceWidth,
            MaximumSurfaceWidth);
        var surfaceWidth = Math.Min(preferredSurfaceWidth, availableSurfaceWidth);

        var availableSurfaceHeight = Math.Max(
            CompactSurfaceHeight,
            workingArea.Height - (VerticalBreathingRoom * 2) - (ShadowMargin * 2));
        var surfaceHeight = isExpanded
            ? Math.Min(ExpandedSurfaceHeight, availableSurfaceHeight)
            : CompactSurfaceHeight;

        var width = surfaceWidth + (ShadowMargin * 2);
        var height = surfaceHeight + (ShadowMargin * 2);

        var surfaceX = workingArea.X + ((workingArea.Width - surfaceWidth) / 2);
        var surfaceDesiredY = workingArea.Y + (workingArea.Height * VerticalCenterBias) - (surfaceHeight / 2);
        var x = surfaceX - ShadowMargin;
        var desiredY = surfaceDesiredY - ShadowMargin;
        var minY = workingArea.Y + VerticalBreathingRoom;
        var maxY = workingArea.Bottom - height - VerticalBreathingRoom;
        var y = maxY < minY ? minY : Math.Clamp(desiredY, minY, maxY);

        return new Rect(
            Math.Round(x),
            Math.Round(y),
            Math.Round(width),
            Math.Round(height));
    }

    public static Rect ToDips(PixelRect pixelRect, double scaling)
    {
        var safeScaling = scaling <= 0 ? 1 : scaling;
        return new Rect(
            pixelRect.X / safeScaling,
            pixelRect.Y / safeScaling,
            pixelRect.Width / safeScaling,
            pixelRect.Height / safeScaling);
    }

    public static PixelPoint ToPixels(Rect bounds, double scaling)
    {
        var safeScaling = scaling <= 0 ? 1 : scaling;
        return new PixelPoint(
            (int)Math.Round(bounds.X * safeScaling),
            (int)Math.Round(bounds.Y * safeScaling));
    }
}
