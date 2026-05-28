namespace Winspot_App.Models;

public sealed record SearchResultItem(
    string Id,
    string Title,
    string Subtitle,
    string Kind,
    double Score,
    string PrimaryAction);
