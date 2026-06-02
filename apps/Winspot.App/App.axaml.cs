using System;

using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using Avalonia.Platform;
using Avalonia.Styling;

using Winspot_App.Models;
using Winspot_App.Services;
using Winspot_App.ViewModels;

namespace Winspot_App;

public sealed partial class App : Application
{
    private readonly LauncherSettingsStore _settingsStore = new();
    private IClassicDesktopStyleApplicationLifetime? _desktop;
    private MainWindow? _mainWindow;
    private SettingsWindow? _settingsWindow;
    private TrayIcon? _trayIcon;

    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    public override void OnFrameworkInitializationCompleted()
    {
        if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        {
            _desktop = desktop;
            var settings = _settingsStore.Load();
            ApplyTheme(settings.ThemeMode);
            MotionSettings.Profile = settings.MotionProfile;

            _mainWindow = new MainWindow(settings);
            _mainWindow.ViewModel.SettingsRequested += (_, _) => ShowSettings();
            desktop.MainWindow = _mainWindow;
            desktop.ShutdownRequested += (_, _) => WinspotIpcClient.StopBackendIfOwned();
            desktop.Exit += (_, _) =>
            {
                _trayIcon?.Dispose();
                WinspotIpcClient.StopBackendIfOwned();
            };

            InitializeTrayIcon();
            UpdateTrayVisibility(settings.ShowTrayIcon);
        }

        base.OnFrameworkInitializationCompleted();
    }

    private void InitializeTrayIcon()
    {
        var openItem = new NativeMenuItem("Open Winspot");
        openItem.Click += (_, _) => ShowLauncher();

        var settingsItem = new NativeMenuItem("Settings\u2026");
        settingsItem.Click += (_, _) => ShowSettings();

        var quitItem = new NativeMenuItem("Quit Winspot");
        quitItem.Click += (_, _) => _desktop?.Shutdown();

        var menu = new NativeMenu();
        menu.Add(openItem);
        menu.Add(settingsItem);
        menu.Add(new NativeMenuItemSeparator());
        menu.Add(quitItem);

        _trayIcon = new TrayIcon
        {
            ToolTipText = "Winspot",
            Menu = menu,
            Icon = LoadTrayIcon(),
        };
        _trayIcon.Clicked += (_, _) => ShowLauncher();

        TrayIcon.SetIcons(this, new TrayIcons { _trayIcon });
    }

    private void ShowLauncher()
    {
        _mainWindow?.ShowLauncher();
    }

    private void ShowSettings()
    {
        if (_settingsWindow is not null)
        {
            _settingsWindow.Activate();
            return;
        }

        var viewModel = new SettingsViewModel(_settingsStore);
        viewModel.Saved += OnSettingsSaved;

        var window = new SettingsWindow(viewModel);
        window.Closed += (_, _) => _settingsWindow = null;
        _settingsWindow = window;
        window.Show();
        window.Activate();
    }

    private void OnSettingsSaved(object? sender, LauncherSettings settings)
    {
        ApplyTheme(settings.ThemeMode);
        MotionSettings.Profile = settings.MotionProfile;
        _mainWindow?.ApplyMotionProfile(settings.MotionProfile);
        var registered = _mainWindow?.ApplyHotkey(settings.Hotkey) ?? false;
        if (sender is SettingsViewModel viewModel)
        {
            viewModel.SetHotkeyRegistrationStatus(registered);
        }
        UpdateTrayVisibility(settings.ShowTrayIcon);
    }

    private void UpdateTrayVisibility(bool isVisible)
    {
        if (_trayIcon is not null)
        {
            _trayIcon.IsVisible = isVisible;
        }
    }

    private static WindowIcon? LoadTrayIcon()
    {
        try
        {
            using var stream = AssetLoader.Open(new Uri("avares://Winspot.App/Assets/WinspotTrayIcon.ico"));
            return new WindowIcon(stream);
        }
        catch
        {
            return null;
        }
    }

    private void ApplyTheme(ThemeMode themeMode)
    {
        RequestedThemeVariant = themeMode switch
        {
            ThemeMode.System => ThemeVariant.Default,
            ThemeMode.Light => ThemeVariant.Light,
            _ => ThemeVariant.Dark,
        };
    }
}
