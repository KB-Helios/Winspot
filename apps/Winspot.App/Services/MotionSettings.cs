using Winspot_App.Models;

namespace Winspot_App.Services;

/// Single source of truth for whether the UI should play motion. When reduced
/// motion is on, reveal/resize/transition animations collapse to their final
/// state so the launcher still works but stays still.
public static class MotionSettings
{
    public static MotionProfile Profile { get; set; } = MotionProfile.Snappy240;

    public static bool ReduceMotion
    {
        get => Profile == MotionProfile.Reduced;
        set => Profile = value ? MotionProfile.Reduced : MotionProfile.Snappy240;
    }
}
