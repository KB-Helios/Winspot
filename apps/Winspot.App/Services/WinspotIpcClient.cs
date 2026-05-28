using System.IO.Pipes;
using System.Text.Json;

using Winspot_App.Models;

namespace Winspot_App.Services;

public sealed class WinspotIpcClient
{
    private const int ProtocolVersion = 1;
    private const string PipeName = "winspot-dev";
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public async Task<IReadOnlyList<SearchResultItem>> SearchAsync(
        string query,
        CancellationToken cancellationToken)
    {
        await using var pipe = new NamedPipeClientStream(
            ".",
            PipeName,
            PipeDirection.InOut,
            PipeOptions.Asynchronous);

        await pipe.ConnectAsync(750, cancellationToken);

        await using var writer = new StreamWriter(pipe, leaveOpen: true) { AutoFlush = true };
        using var reader = new StreamReader(pipe, leaveOpen: true);

        var requestId = Guid.NewGuid().ToString("N");
        var request = new IpcEnvelope(
            ProtocolVersion,
            requestId,
            new IpcPayload(
                "SearchStarted",
                JsonSerializer.SerializeToElement(new SearchStarted(requestId, query), JsonOptions)));

        var requestJson = JsonSerializer.Serialize(request, JsonOptions);
        await writer.WriteLineAsync(requestJson.AsMemory(), cancellationToken);

        var line = await reader.ReadLineAsync(cancellationToken);
        if (string.IsNullOrWhiteSpace(line))
        {
            return Array.Empty<SearchResultItem>();
        }

        var response = JsonSerializer.Deserialize<IpcEnvelope>(line, JsonOptions);
        if (response?.Payload.Type != "ResultBatch")
        {
            return Array.Empty<SearchResultItem>();
        }

        var batch = response.Payload.Data.Deserialize<ResultBatch>(JsonOptions);
        return batch?.Results ?? Array.Empty<SearchResultItem>();
    }

    public async Task<string> ExecuteAsync(
        SearchResultItem result,
        CancellationToken cancellationToken)
    {
        await using var pipe = new NamedPipeClientStream(
            ".",
            PipeName,
            PipeDirection.InOut,
            PipeOptions.Asynchronous);

        await pipe.ConnectAsync(750, cancellationToken);

        await using var writer = new StreamWriter(pipe, leaveOpen: true) { AutoFlush = true };
        using var reader = new StreamReader(pipe, leaveOpen: true);

        var requestId = Guid.NewGuid().ToString("N");
        var request = new IpcEnvelope(
            ProtocolVersion,
            requestId,
            new IpcPayload(
                "ActionRequested",
                JsonSerializer.SerializeToElement(
                    new ActionRequested(
                        requestId,
                        result.Id,
                        result.Title,
                        result.PrimaryAction),
                    JsonOptions)));

        var requestJson = JsonSerializer.Serialize(request, JsonOptions);
        await writer.WriteLineAsync(requestJson.AsMemory(), cancellationToken);

        var line = await reader.ReadLineAsync(cancellationToken);
        if (string.IsNullOrWhiteSpace(line))
        {
            throw new InvalidOperationException("Backend returned an empty action response.");
        }

        var response = JsonSerializer.Deserialize<IpcEnvelope>(line, JsonOptions);
        if (response?.Payload.Type != "ActionCompleted")
        {
            throw new InvalidOperationException("Backend returned an unexpected action response.");
        }

        var completed = response.Payload.Data.Deserialize<ActionCompleted>(JsonOptions)
            ?? throw new InvalidOperationException("Backend returned an invalid action response.");

        if (!completed.Succeeded)
        {
            throw new InvalidOperationException(completed.Message);
        }

        return completed.Message;
    }

    private sealed record IpcEnvelope(
        int ProtocolVersion,
        string RequestId,
        IpcPayload Payload);

    private sealed record IpcPayload(string Type, JsonElement Data);

    private sealed record SearchStarted(string QueryId, string Text);

    private sealed record ActionRequested(
        string ActionId,
        string ResultId,
        string Title,
        string PrimaryAction);

    private sealed record ActionCompleted(
        string ActionId,
        bool Succeeded,
        string Message);

    private sealed record ResultBatch(
        string QueryId,
        bool IsFinal,
        IReadOnlyList<SearchResultItem> Results);
}
