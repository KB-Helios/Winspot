namespace Winspot_App.Models;

public sealed class HotkeyBinding
{
    public string Key { get; init; } = "Space";

    public List<string> Modifiers { get; init; } = new() { "Control", "Alt" };

    /// Renders the binding as a space-separated chord (e.g. "Ctrl Alt Space")
    /// for display in the launcher hint, matching the order modifiers are shown
    /// on Windows.
    public string ToDisplayString()
    {
        var parts = new List<string>(Modifiers.Count + 1);
        foreach (var modifier in Modifiers)
        {
            parts.Add(NormalizeModifier(modifier));
        }

        parts.Add(NormalizeKey(Key));
        return string.Join(" ", parts);
    }

    private static string NormalizeModifier(string modifier) => modifier.Trim().ToLowerInvariant() switch
    {
        "control" or "ctrl" => "Ctrl",
        "alt" => "Alt",
        "shift" => "Shift",
        "win" or "windows" => "Win",
        _ => Capitalize(modifier.Trim()),
    };

    private static string NormalizeKey(string key)
    {
        var trimmed = key.Trim();
        return trimmed.Length <= 1 ? trimmed.ToUpperInvariant() : Capitalize(trimmed);
    }

    private static string Capitalize(string value) => value.Length == 0
        ? value
        : char.ToUpperInvariant(value[0]) + value[1..].ToLowerInvariant();
}
