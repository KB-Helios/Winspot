using System.Runtime.InteropServices;

using Winspot_App.Models;
using Windows.System;

namespace Winspot_App.Services;

internal sealed class GlobalHotkeyService : IDisposable
{
    private const int GwlWndProc = -4;
    private const int HotkeyId = 0x5753;
    private const int WmHotkey = 0x0312;
    private const uint ModAlt = 0x0001;
    private const uint ModControl = 0x0002;
    private const uint ModShift = 0x0004;
    private const uint ModWin = 0x0008;
    private const uint ModNoRepeat = 0x4000;

    private readonly nint _windowHandle;
    private readonly WndProc _wndProc;
    private nint _previousWndProc;
    private bool _isRegistered;

    public GlobalHotkeyService(nint windowHandle)
    {
        _windowHandle = windowHandle;
        _wndProc = HotkeyWndProc;
    }

    public event EventHandler? Pressed;

    public bool Register(HotkeyBinding binding)
    {
        if (_windowHandle == 0 || !TryParse(binding, out var modifiers, out var key))
        {
            return false;
        }

        _previousWndProc = SetWindowLongPtr(
            _windowHandle,
            GwlWndProc,
            Marshal.GetFunctionPointerForDelegate(_wndProc));

        if (_previousWndProc == 0)
        {
            return false;
        }

        _isRegistered = RegisterHotKey(_windowHandle, HotkeyId, modifiers | ModNoRepeat, key);
        return _isRegistered;
    }

    public void Dispose()
    {
        if (_isRegistered)
        {
            UnregisterHotKey(_windowHandle, HotkeyId);
            _isRegistered = false;
        }

        if (_previousWndProc != 0)
        {
            SetWindowLongPtr(_windowHandle, GwlWndProc, _previousWndProc);
            _previousWndProc = 0;
        }
    }

    private static bool TryParse(HotkeyBinding binding, out uint modifiers, out uint key)
    {
        modifiers = 0;
        key = 0;

        foreach (var modifier in binding.Modifiers)
        {
            modifiers |= modifier.Trim().ToLowerInvariant() switch
            {
                "alt" => ModAlt,
                "control" or "ctrl" => ModControl,
                "shift" => ModShift,
                "win" or "windows" => ModWin,
                _ => 0,
            };
        }

        if (modifiers == 0)
        {
            return false;
        }

        if (!Enum.TryParse<VirtualKey>(binding.Key, true, out var virtualKey))
        {
            return false;
        }

        key = (uint)virtualKey;
        return key != 0;
    }

    private nint HotkeyWndProc(nint hWnd, uint message, nuint wParam, nint lParam)
    {
        if (message == WmHotkey && wParam == HotkeyId)
        {
            Pressed?.Invoke(this, EventArgs.Empty);
            return 0;
        }

        return CallWindowProc(_previousWndProc, hWnd, message, wParam, lParam);
    }

    private static nint SetWindowLongPtr(nint hWnd, int index, nint newLong)
    {
        return IntPtr.Size == 8
            ? SetWindowLongPtr64(hWnd, index, newLong)
            : SetWindowLongPtr32(hWnd, index, newLong);
    }

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool RegisterHotKey(nint hWnd, int id, uint fsModifiers, uint vk);

    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool UnregisterHotKey(nint hWnd, int id);

    [DllImport("user32.dll", EntryPoint = "SetWindowLongW", SetLastError = true)]
    private static extern nint SetWindowLongPtr32(nint hWnd, int nIndex, nint dwNewLong);

    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW", SetLastError = true)]
    private static extern nint SetWindowLongPtr64(nint hWnd, int nIndex, nint dwNewLong);

    [DllImport("user32.dll")]
    private static extern nint CallWindowProc(
        nint lpPrevWndFunc,
        nint hWnd,
        uint msg,
        nuint wParam,
        nint lParam);

    private delegate nint WndProc(nint hWnd, uint message, nuint wParam, nint lParam);
}
