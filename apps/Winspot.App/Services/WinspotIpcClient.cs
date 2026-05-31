using System.IO.Pipes;
using System.Text.Json;

using Winspot_App.Models;

namespace Winspot_App.Services;

public interface IWinspotIpcClient
{
    async IAsyncEnumerable<IReadOnlyList<SearchResultItem>> StreamSearchAsync(
        string query,
        [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken)
    {
        yield return await SearchAsync(query, cancellationToken);
    }

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

    /// Stops the daemon if this app started it. Called on shutdown so closing
    /// the launcher doesn't leave an orphaned backend behind.
    public static void StopBackendIfOwned() => BackendProcess.StopIfOwned();

    public async Task<IReadOnlyList<SearchResultItem>> SearchAsync(
        string query,
        CancellationToken cancellationToken)
    {
        var latest = Array.Empty<SearchResultItem>() as IReadOnlyList<SearchResultItem>;
        await foreach (var batch in StreamSearchAsync(query, cancellationToken))
        {
            latest = batch;
        }

        return latest;
    }

    public async IAsyncEnumerable<IReadOnlyList<SearchResultItem>> StreamSearchAsync(
        string query,
        [System.Runtime.CompilerServices.EnumeratorCancellation] CancellationToken cancellationToken)
    {
        await using var pipe = await ConnectAsync(cancellationToken);

        await using var writer = new StreamWriter(pipe, leaveOpen: true) { AutoFlush = true };
        using var reader = new StreamReader(pipe, leaveOpen: true);

        await NegotiateAsync(writer, reader, cancellationToken);

        var requestId = Guid.NewGuid().ToString("N");
        var request = new IpcEnvelope(
            ProtocolVersion,
            requestId,
            new IpcPayload(
                "SearchStarted",
                JsonSerializer.SerializeToElement(new SearchStarted(requestId, query), JsonOptions)));

        var requestJson = JsonSerializer.Serialize(request, JsonOptions);
        await writer.WriteLineAsync(requestJson.AsMemory(), cancellationToken);

        while (!cancellationToken.IsCancellationRequested)
        {
            var line = await reader.ReadLineAsync(cancellationToken);
            if (string.IsNullOrWhiteSpace(line))
            {
                yield break;
            }

            var response = JsonSerializer.Deserialize<IpcEnvelope>(line, JsonOptions);
            if (response?.Payload.Type == "ResultBatch")
            {
                var batch = response.Payload.Data.Deserialize<ResultBatch>(JsonOptions);
                if (batch?.Results is not null)
                {
                    yield return batch.Results;
                }
            }

            if (response?.Payload.Type == "SearchCompleted")
            {
                yield break;
            }

            if (response?.Payload.Type == "Error")
            {
                var error = response.Payload.Data.Deserialize<BackendError>(JsonOptions);
                throw new InvalidOperationException(error?.Message ?? "Backend returned an error.");
            }
        }
    }

    public async Task<string> ExecuteAsync(
        SearchResultItem result,
        CancellationToken cancellationToken)
    {
        await using var pipe = await ConnectAsync(cancellationToken);

        await using var writer = new StreamWriter(pipe, leaveOpen: true) { AutoFlush = true };
        using var reader = new StreamReader(pipe, leaveOpen: true);

        await NegotiateAsync(writer, reader, cancellationToken);

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

        await NegotiateAsync(writer, reader, cancellationToken);

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

        PreviewItem? latest = null;
        while (!cancellationToken.IsCancellationRequested)
        {
            var line = await reader.ReadLineAsync(cancellationToken);
            if (string.IsNullOrWhiteSpace(line))
            {
                return latest;
            }

            var response = JsonSerializer.Deserialize<IpcEnvelope>(line, JsonOptions);
            if (response?.Payload.Type == "PreviewChunk")
            {
                var preview = response.Payload.Data.Deserialize<PreviewChunk>(JsonOptions);
                if (preview is not null)
                {
                    latest = new PreviewItem(preview.Title, preview.Body);
                }
            }

            if (response?.Payload.Type == "PreviewReady")
            {
                var preview = response.Payload.Data.Deserialize<PreviewReady>(JsonOptions);
                return preview is null ? latest : new PreviewItem(preview.Title, preview.Body);
            }
        }

        return latest;
    }

    private sealed record IpcEnvelope(
        int ProtocolVersion,
        string RequestId,
        IpcPayload Payload);

    private sealed record IpcPayload(string Type, JsonElement Data);

    private sealed record Hello(
        int MinProtocolVersion,
        int MaxProtocolVersion,
        string ClientName);

    private sealed record HelloAccepted(
        int ProtocolVersion,
        int MaxJsonLineBytes,
        string ServerName);

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

    private sealed record PreviewChunk(
        string PreviewId,
        string Title,
        string Body,
        bool IsFinal);

    private sealed record BackendError(
        string Code,
        string Message,
        bool Retryable);

    private sealed record ResultBatch(
        string QueryId,
        bool IsFinal,
        int BatchIndex,
        IReadOnlyList<SearchResultItem> Results);

    private static async Task NegotiateAsync(
        StreamWriter writer,
        StreamReader reader,
        CancellationToken cancellationToken)
    {
        var requestId = Guid.NewGuid().ToString("N");
        var hello = new IpcEnvelope(
            ProtocolVersion,
            requestId,
            new IpcPayload(
                "Hello",
                JsonSerializer.SerializeToElement(
                    new Hello(1, 2, "Winspot.App"),
                    JsonOptions)));
        await writer.WriteLineAsync(
            JsonSerializer.Serialize(hello, JsonOptions).AsMemory(),
            cancellationToken);

        var line = await reader.ReadLineAsync(cancellationToken);
        if (string.IsNullOrWhiteSpace(line))
        {
            throw new InvalidOperationException("Backend did not negotiate an IPC protocol.");
        }

        var response = JsonSerializer.Deserialize<IpcEnvelope>(line, JsonOptions);
        if (response?.Payload.Type != "HelloAccepted")
        {
            throw new InvalidOperationException("Backend rejected IPC protocol negotiation.");
        }

        var accepted = response.Payload.Data.Deserialize<HelloAccepted>(JsonOptions);
        if (accepted is null || accepted.ProtocolVersion < ProtocolVersion)
        {
            throw new InvalidOperationException("Backend IPC protocol is incompatible.");
        }
    }

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
