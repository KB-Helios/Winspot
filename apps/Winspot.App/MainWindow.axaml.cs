using System.ComponentModel;

using Avalonia;
using Avalonia.Animation;
using Avalonia.Animation.Easings;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Rendering.Composition;
using Avalonia.Rendering.Composition.Animations;
using Avalonia.Threading;

using Winspot_App.Models;
using Winspot_App.Services;
using Winspot_App.ViewModels;

namespace Winspot_App;

public sealed partial class MainWindow : Window
{
    private static readonly TimeSpan RevealDuration = TimeSpan.FromMilliseconds(160);
    private static readonly CubicEaseOut RevealEasing = new();

    private readonly Transitions? _resultsHostTransitions;
    private GlobalHotkeyService? _hotkeyService;
    private HotkeyBinding _hotkey;

    public MainWindow()
        : this(new LauncherSettingsStore().Load())
    {
    }

    public MainWindow(LauncherSettings settings)
    {
        _hotkey = settings.Hotkey;
        ViewModel = new LauncherViewModel(new WinspotIpcClient(), settings.Hotkey);
        DataContext = ViewModel;
        InitializeComponent();

        _resultsHostTransitions = ResultsHost.Transitions;
        Height = LauncherWindowLayout.CompactHeight;
        ApplyMotionProfile(settings.MotionProfile);

        ViewModel.PropertyChanged += OnViewModelPropertyChanged;
        Opened += OnOpened;
        Closing += OnClosing;
        KeyDown += OnKeyDown;
    }

    public LauncherViewModel ViewModel { get; }

    /// Brings the launcher to the foreground and focuses the search box. Used by
    /// the tray icon's "Open Winspot" command.
    public void ShowLauncher()
    {
        Show();
        Activate();
        Reveal();
    }

    /// Re-registers the global hotkey and refreshes the hint after the user
    /// changes the activation chord in settings.
    public bool ApplyHotkey(HotkeyBinding hotkey)
    {
        var previousHotkey = _hotkey;
        _hotkey = hotkey;
        ViewModel.UpdateHotkeyHint(hotkey);

        if (_hotkeyService is not null)
        {
            _hotkeyService.Pressed -= OnGlobalHotkeyPressed;
            _hotkeyService.Dispose();
            _hotkeyService = null;
        }

        if (RegisterHotkey())
        {
            return true;
        }

        _hotkey = previousHotkey;
        ViewModel.UpdateHotkeyHint(previousHotkey);
        RegisterHotkey();
        return false;
    }

    internal void ApplyMotionProfile(MotionProfile motionProfile)
    {
        MotionSettings.Profile = motionProfile;
        ResultsHost.Transitions = MotionSettings.ReduceMotion ? null : _resultsHostTransitions;
    }

    public void FocusSearch()
    {
        SearchBox.Focus(NavigationMethod.Unspecified);
    }

    public void Reveal()
    {
        ApplyResponsiveBounds();
        PlayRevealAnimation();
        _ = Dispatcher.UIThread.InvokeAsync(() =>
        {
            FocusSearch();
        }, DispatcherPriority.Background);
    }

    private async void OnViewModelPropertyChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (e.PropertyName == nameof(LauncherViewModel.IsExpanded))
        {
            var (bounds, scaling) = GetResponsiveBounds(ViewModel.IsExpanded);
            await AnimateBoundsAsync(bounds, scaling);
        }
    }

    private void OnOpened(object? sender, EventArgs e)
    {
        Reveal();
        _ = Dispatcher.UIThread.InvokeAsync(async () =>
        {
            await Task.Delay(400).ConfigureAwait(true);
            RegisterHotkey();
        }, DispatcherPriority.Background);
    }

    private void OnClosing(object? sender, WindowClosingEventArgs e)
    {
        ViewModel.PropertyChanged -= OnViewModelPropertyChanged;
        if (_hotkeyService is not null)
        {
            _hotkeyService.Pressed -= OnGlobalHotkeyPressed;
            _hotkeyService.Dispose();
        }
    }

    private async void OnKeyDown(object? sender, KeyEventArgs e)
    {
        if (e.Key == Key.Escape)
        {
            Hide();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Down)
        {
            ViewModel.MoveSelectionDown();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Up)
        {
            ViewModel.MoveSelectionUp();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Tab)
        {
            ViewModel.FocusActions();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Right)
        {
            if (!ShouldHandleActionNavigationKey(e.Key, ViewModel.FocusedActionIndex))
            {
                return;
            }

            ViewModel.MoveActionRight();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Left)
        {
            if (!ShouldHandleActionNavigationKey(e.Key, ViewModel.FocusedActionIndex))
            {
                return;
            }

            if (ViewModel.FocusedActionIndex > 0)
            {
                ViewModel.MoveActionLeft();
            }
            else
            {
                ViewModel.FocusResults();
            }

            e.Handled = true;
            return;
        }

        if (e.Key != Key.Enter)
        {
            return;
        }

        e.Handled = true;
        var hideBeforeExecution = ShouldHideBeforeExecutingResult(ViewModel.SelectedResult);
        if (hideBeforeExecution)
        {
            Hide();
        }

        if (await ViewModel.AcceptSelectionAsync() && !hideBeforeExecution)
        {
            Hide();
        }
    }

    private void OnGlobalHotkeyPressed(object? sender, EventArgs e)
    {
        Dispatcher.UIThread.Post(() =>
        {
            if (ShouldHideForHotkey(IsVisible, IsActive))
            {
                Hide();
                return;
            }

            Show();
            Activate();
            Reveal();
        });
    }

    public static bool ShouldHandleActionNavigationKey(Key key, int focusedActionIndex) =>
        key is Key.Left or Key.Right && focusedActionIndex >= 0;

    public static bool ShouldHideForHotkey(bool isVisible, bool isActive) => isVisible;

    public static bool ShouldHideBeforeExecutingResult(SearchResultItem? result) =>
        result?.Id.StartsWith("plugin:windows-capture:", StringComparison.Ordinal) == true
        || string.Equals(result?.Source, "windows-capture", StringComparison.OrdinalIgnoreCase);

    private void ApplyResponsiveBounds()
    {
        var (bounds, scaling) = GetResponsiveBounds(ViewModel.IsExpanded);
        Width = bounds.Width;
        Height = bounds.Height;
        Position = LauncherWindowLayout.ToPixels(bounds, scaling);
    }

    private (Rect Bounds, double Scaling) GetResponsiveBounds(bool isExpanded)
    {
        var screen = Screens.ScreenFromWindow(this) ?? Screens.Primary;
        if (screen is null)
        {
            var fallbackHeight = isExpanded
                ? LauncherWindowLayout.ExpandedHeight
                : LauncherWindowLayout.CompactHeight;
            return (new Rect(0, 0, Width, fallbackHeight), 1);
        }

        var scaling = screen.Scaling <= 0 ? 1 : screen.Scaling;
        var workingArea = LauncherWindowLayout.ToDips(screen.WorkingArea, scaling);
        return (LauncherWindowLayout.CalculateBounds(workingArea, isExpanded), scaling);
    }

    private void PlayRevealAnimation()
    {
        var visual = ElementComposition.GetElementVisual(SpotlightSurface);
        if (visual is null)
        {
            return;
        }

        visual.StopAnimation("Opacity");
        visual.StopAnimation("Scale");
        visual.StopAnimation("Offset");

        if (MotionSettings.ReduceMotion)
        {
            visual.Opacity = 1;
            visual.Scale = new Vector3D(1, 1, 1);
            visual.Offset = new Vector3D();
            return;
        }

        var centerX = Math.Max(SpotlightSurface.Bounds.Width, 1) / 2;
        var centerY = Math.Max(SpotlightSurface.Bounds.Height, 1) / 2;
        visual.CenterPoint = new Vector3D(centerX, centerY, 0);
        visual.Opacity = 0;
        visual.Scale = new Vector3D(0.94, 0.94, 1);
        visual.Offset = new Vector3D(0, -8, 0);

        var compositor = visual.Compositor;
        var opacityAnimation = compositor.CreateScalarKeyFrameAnimation();
        opacityAnimation.Duration = RevealDuration;
        opacityAnimation.StopBehavior = AnimationStopBehavior.SetToFinalValue;
        opacityAnimation.InsertKeyFrame(0, 0);
        opacityAnimation.InsertKeyFrame(1, 1, RevealEasing);

        var scaleAnimation = compositor.CreateVector3DKeyFrameAnimation();
        scaleAnimation.Duration = RevealDuration;
        scaleAnimation.StopBehavior = AnimationStopBehavior.SetToFinalValue;
        scaleAnimation.InsertKeyFrame(0, new Vector3D(0.94, 0.94, 1));
        scaleAnimation.InsertKeyFrame(1, new Vector3D(1, 1, 1), RevealEasing);

        var offsetAnimation = compositor.CreateVector3DKeyFrameAnimation();
        offsetAnimation.Duration = RevealDuration;
        offsetAnimation.StopBehavior = AnimationStopBehavior.SetToFinalValue;
        offsetAnimation.InsertKeyFrame(0, new Vector3D(0, -8, 0));
        offsetAnimation.InsertKeyFrame(1, new Vector3D(), RevealEasing);

        visual.StartAnimation("Opacity", opacityAnimation);
        visual.StartAnimation("Scale", scaleAnimation);
        visual.StartAnimation("Offset", offsetAnimation);
    }

    private Task AnimateBoundsAsync(Rect targetBounds, double scaling)
    {
        var targetPosition = LauncherWindowLayout.ToPixels(targetBounds, scaling);
        Width = targetBounds.Width;
        Height = targetBounds.Height;
        Position = targetPosition;
        return Task.CompletedTask;
    }

    private bool RegisterHotkey()
    {
        if (_hotkeyService is not null)
        {
            ViewModel.UpdateHotkeyRegistrationStatus(true);
            return true;
        }

        var handle = GetWindowHandle();
        if (handle == 0)
        {
            ViewModel.UpdateHotkeyRegistrationStatus(false);
            return false;
        }

        var hotkeyService = new GlobalHotkeyService(handle);
        hotkeyService.Pressed += OnGlobalHotkeyPressed;
        if (hotkeyService.Register(_hotkey))
        {
            _hotkeyService = hotkeyService;
            ViewModel.UpdateHotkeyRegistrationStatus(true);
            return true;
        }

        hotkeyService.Pressed -= OnGlobalHotkeyPressed;
        hotkeyService.Dispose();
        ViewModel.UpdateHotkeyRegistrationStatus(false);
        return false;
    }

    private nint GetWindowHandle()
    {
        try
        {
            return TryGetPlatformHandle()?.Handle ?? 0;
        }
        catch
        {
            return 0;
        }
    }
}
