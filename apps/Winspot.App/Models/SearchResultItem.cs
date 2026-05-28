namespace Winspot_App.Models;

public sealed record SearchResultItem(
    string Id,
    string Title,
    string Subtitle,
    string Kind,
    double Score,
    string PrimaryAction)
{
    public string IconGlyph => Kind switch
    {
        "App" => "\uECAA",
        "File" => "\uE8A5",
        "Folder" => "\uE8B7",
        "Command" => "\uE756",
        "Setting" => "\uE713",
        "Plugin" => "\uE71B",
        _ => "\uE721",
    };

    public string IconLabel => $"{Kind} result";
}
