using System.IO;
using System.IO.Pipes;
using System.Text.Json;
using System.Threading;
using System.Threading.Tasks;
using System;

using Microsoft.VisualStudio.TestTools.UnitTesting;

using Winspot_App.Services;

namespace Winspot_App.Tests.Services;

[TestClass]
public sealed class WinspotIpcClientTests
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);

    [TestMethod]
    public async Task GetPluginDiagnosticsAsync_WhenReadyResponse_ReturnsReportAndSendsRequest()
    {
        var pipeName = CreatePipeName();
        var responseJson = CreateEnvelopeJson(
            "PluginDiagnosticsReady",
            new
            {
                report = new
                {
                    entries = new[]
                    {
                        new
                        {
                            id = "notes",
                            name = "Notes",
                            manifestPath = "C:\\Plugins\\notes.json",
                            source = "User",
                            status = "Accepted",
                            trusted = true,
                            issues = Array.Empty<object>(),
                        },
                    },
                },
            });
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        var serverTask = ServeOneDiagnosticsRequestAsync(pipeName, responseJson, timeout.Token);
        var client = new WinspotIpcClient(pipeName);

        var report = await client.GetPluginDiagnosticsAsync(timeout.Token);
        var requestType = await serverTask;

        Assert.AreEqual("PluginDiagnosticsRequested", requestType);
        Assert.AreEqual(1, report.SafeEntries.Count);
        Assert.AreEqual("notes", report.SafeEntries[0].Id);
        Assert.AreEqual("Accepted", report.SafeEntries[0].Status);
    }

    [TestMethod]
    public async Task GetPluginDiagnosticsAsync_WhenErrorResponse_ThrowsBackendMessage()
    {
        var pipeName = CreatePipeName();
        var responseJson = CreateEnvelopeJson(
            "Error",
            new
            {
                code = "plugin_diagnostics_failed",
                message = "validation failed",
                retryable = false,
            });
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(5));
        var serverTask = ServeOneDiagnosticsRequestAsync(pipeName, responseJson, timeout.Token);
        var client = new WinspotIpcClient(pipeName);

        var exception = await Assert.ThrowsExactlyAsync<InvalidOperationException>(
            () => client.GetPluginDiagnosticsAsync(timeout.Token));
        var requestType = await serverTask;

        Assert.AreEqual("PluginDiagnosticsRequested", requestType);
        Assert.AreEqual("validation failed", exception.Message);
    }

    private static async Task<string> ServeOneDiagnosticsRequestAsync(
        string pipeName,
        string responseJson,
        CancellationToken cancellationToken)
    {
        await using var server = new NamedPipeServerStream(
            pipeName,
            PipeDirection.InOut,
            1,
            PipeTransmissionMode.Byte,
            PipeOptions.Asynchronous);
        await server.WaitForConnectionAsync(cancellationToken);

        using var reader = new StreamReader(server, leaveOpen: true);
        await using var writer = new StreamWriter(server, leaveOpen: true) { AutoFlush = true };

        var hello = await reader.ReadLineAsync(cancellationToken);
        AssertPayloadType(hello, "Hello");

        await writer.WriteLineAsync(
            CreateEnvelopeJson(
                "HelloAccepted",
                new
                {
                    protocolVersion = 1,
                    maxJsonLineBytes = 32768,
                    serverName = "Winspot.Test",
                }).AsMemory(),
            cancellationToken);

        var request = await reader.ReadLineAsync(cancellationToken);
        Assert.IsFalse(string.IsNullOrWhiteSpace(request));
        var requestType = ReadPayloadType(request);
        await writer.WriteLineAsync(responseJson.AsMemory(), cancellationToken);
        return requestType;
    }

    private static string CreatePipeName() => $"winspot-test-{Guid.NewGuid():N}";

    private static string CreateEnvelopeJson(string payloadType, object data) => JsonSerializer.Serialize(
        new
        {
            protocolVersion = 1,
            requestId = Guid.NewGuid().ToString("N"),
            payload = new
            {
                type = payloadType,
                data,
            },
        },
        JsonOptions);

    private static void AssertPayloadType(string? json, string expected)
    {
        Assert.IsFalse(string.IsNullOrWhiteSpace(json));
        Assert.AreEqual(expected, ReadPayloadType(json));
    }

    private static string ReadPayloadType(string json)
    {
        using var document = JsonDocument.Parse(json);
        return document.RootElement
            .GetProperty("payload")
            .GetProperty("type")
            .GetString() ?? string.Empty;
    }
}
