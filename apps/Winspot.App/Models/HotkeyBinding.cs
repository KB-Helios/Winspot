namespace Winspot_App.Models;

public sealed class HotkeyBinding
{
    public string Key { get; init; } = "Space";

    public List<string> Modifiers { get; init; } = new() { "Control", "Alt" };
}
