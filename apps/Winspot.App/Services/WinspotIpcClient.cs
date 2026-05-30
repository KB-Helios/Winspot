using System.IO.Pipes;
using System.Text.Json;

using Winspot_App.Models;

namespace Winspot_App.Services;

public interface IWinspotIpcClient
{
    Task<IReadOnlyList<SearchResultItem>> SearchAsync(
        string query,
        CancellationToken cancellationToken);

    Task<string> ExecuteAsync(
        SearchResultItem result,
        CancellationToken cancellationToken);

    Task<PreviewItem?> GetPreviewAsync(
        SearchResultItem result,
        CancellationToken cancellationToken);
}

public sealed class WinspotIpcClient : IWinspotIpcClient
{
    private const int ProtocolVersion = 1;
    private const string PipeName = "winspot-dev";
    private static readonly WinspotBackendProcess BackendProcess = new();
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    public async Task<IReadOnlyList<SearchResultItem>> SearchAsync(
        string query,
        CancellationToken cancellationToken)
    {
        await using var pipe = await ConnectAsync(cancellationToken);

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
        await using var pipe = await ConnectAsync(cancellationToken);

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

    public async Task<PreviewItem?> GetPreviewAsync(
        SearchResultItem result,
        CancellationToken cancellationToken)
    {
        await using var pipe = await ConnectAsync(cancellationToken);

        await using var writer = new StreamWriter(pipe, leaveOpen: true) { AutoFlush = true };
        using var reader = new StreamReader(pipe, leaveOpen: true);

        var requestId = Guid.NewGuid().ToString("N");
        var request = new IpcEnvelope(
            ProtocolVersion,
            requestId,
            new IpcPayload(
                "PreviewRequested",
                JsonSerializer.SerializeToElement(
                    new PreviewRequested(
                        requestId,
                        result),
                    JsonOptions)));

        var requestJson = JsonSerializer.Serialize(request, JsonOptions);
        await writer.WriteLineAsync(requestJson.AsMemory(), cancellationToken);

        var line = await reader.ReadLineAsync(cancellationToken);
        if (string.IsNullOrWhiteSpace(line))
        {
            return null;
        }

        var response = JsonSerializer.Deserialize<IpcEnvelope>(line, JsonOptions);
        if (response?.Payload.Type != "PreviewReady")
        {
            return null;
        }

        var preview = response.Payload.Data.Deserialize<PreviewReady>(JsonOptions);
        return preview is null ? null : new PreviewItem(preview.Title, preview.Body);
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

    private sealed record PreviewRequested(
        string PreviewId,
        SearchResultItem Result);

    private sealed record PreviewReady(
        string PreviewId,
        string Title,
        string Body,
        bool IsFinal);

    private sealed record ResultBatch(
        string QueryId,
        bool IsFinal,
        IReadOnlyList<SearchResultItem> Results);

    // A NamedPipeClientStream cannot be reconnected once a ConnectAsync attempt
    // has faulted, so every attempt uses a fresh stream and the caller owns the
    // returned, already-connected instance.
    private static async Task<NamedPipeClientStream> ConnectAsync(
        CancellationToken cancellationToken)
    {
        var pipe = CreatePipe();
        try
        {
            await pipe.ConnectAsync(750, cancellationToken);
            return pipe;
        }
        catch (TimeoutException)
        {
            await pipe.DisposeAsync();

            if (!await BackendProcess.TryStartAsync(cancellationToken))
            {
                throw;
            }

            var retryPipe = CreatePipe();
            try
            {
                await retryPipe.ConnectAsync(2_000, cancellationToken);
                return retryPipe;
            }
            catch
            {
                await retryPipe.DisposeAsync();
                throw;
            }
        }
        catch
        {
            await pipe.DisposeAsync();
            throw;
        }
    }

    private static NamedPipeClientStream CreatePipe() => new(
        ".",
        PipeName,
        PipeDirection.InOut,
        PipeOptions.Asynchronous);
}
