using Avalonia;

namespace Winspot_App;

internal static class LauncherWindowLayout
{
    public const double CompactHeight = 72;
    public const double ExpandedHeight = 560;

    private const double MinimumWidth = 420;
    private const double MaximumWidth = 760;
    private const double HorizontalBreathingRoom = 48;
    private const double VerticalBreathingRoom = 24;
    private const double VerticalCenterBias = 0.44;

    public static Rect CalculateBounds(Rect workingArea, bool isExpanded)
    {
        var width = Math.Clamp(workingArea.Width - HorizontalBreathingRoom, MinimumWidth, MaximumWidth);
        var height = isExpanded
            ? Math.Min(ExpandedHeight, Math.Max(CompactHeight, workingArea.Height - (VerticalBreathingRoom * 4)))
            : CompactHeight;

        var x = workingArea.X + ((workingArea.Width - width) / 2);
        var desiredY = workingArea.Y + (workingArea.Height * VerticalCenterBias) - (height / 2);
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
