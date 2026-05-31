namespace Winspot_App.Models;

public sealed record SearchResultItem(
    string Id,
    string Title,
    string Subtitle,
    string Kind,
    double Score,
    string PrimaryAction,
    IReadOnlyList<ActionItem>? Actions = null,
    string? Source = null,
    string? IconHint = null)
{
    public IReadOnlyList<ActionItem> DisplayActions => Actions is { Count: > 0 }
        ? Actions
        : new[] { new ActionItem(PrimaryAction, PrimaryAction) };

    public string IconGlyph => Kind switch
    {
        "App" => "\uECAA",
        "File" => "\uE8A5",
        "Folder" => "\uE8B7",
        "Command" => "\uE756",
        "Setting" => "\uE713",
        "Plugin" => "\uE71B",
        "Process" => "\uE9F5",
        _ => "\uE721",
    };

    public string IconLabel => $"{Kind} result";

    /// File-system path for kinds whose result id encodes one (apps, files,
    /// folders), used to fetch the real shell icon. Other kinds fall back to the
    /// font glyph above.
    public string? IconPath => Kind switch
    {
        "App" => StripIdPrefix("app:"),
        "File" => StripIdPrefix("file:"),
        "Folder" => StripIdPrefix("folder:"),
        _ => null,
    };

    public bool HasIcon => IconPath is not null;

    private string? StripIdPrefix(string prefix) =>
        Id.StartsWith(prefix, StringComparison.Ordinal) ? Id[prefix.Length..] : null;
}

public sealed record ActionItem(
    string Id,
    string Label);

public sealed record ActionViewItem(
    string Id,
    string Label,
    bool IsFocused);
