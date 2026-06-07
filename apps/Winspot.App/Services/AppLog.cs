using System.Diagnostics;

namespace Winspot_App.Services;

/// Lightweight, dependency-free crash and diagnostic logger.
///
/// The launcher runs detached from any console, so an unhandled exception
/// previously vanished without a trace. <see cref="AppLog"/> captures those
/// failures to <c>%LOCALAPPDATA%\Winspot\logs\app.log</c> (or the portable
/// <c>data\logs</c> folder) so production crashes can be diagnosed after the
/// fact. Logging is strictly best-effort: it never throws, because it is the
/// last line of defence and must not mask the original failure.
public static class AppLog
{
    private static readonly object Gate = new();
    private static string? _logFilePath;
    private static int _initialized;

    /// Wires the global, last-resort exception handlers that have no natural
    /// local <c>try</c>/<c>catch</c>. Safe to call multiple times; only the
    /// first call takes effect.
    public static void Init()
    {
        if (Interlocked.Exchange(ref _initialized, 1) == 1)
        {
            return;
        }

        AppDomain.CurrentDomain.UnhandledException += (_, args) =>
        {
            var exception = args.ExceptionObject as Exception
                ?? new Exception(args.ExceptionObject?.ToString() ?? "unknown error");
            Error("AppDomain.UnhandledException", exception);
        };

        TaskScheduler.UnobservedTaskException += (_, args) =>
        {
            Error("TaskScheduler.UnobservedTaskException", args.Exception);
            // Mark observed so the default escalation policy does not tear the
            // process down for a background task we have already recorded.
            args.SetObserved();
        };
    }

    /// Appends a timestamped error entry for <paramref name="context"/> and
    /// <paramref name="exception"/>. Never throws.
    public static void Error(string context, Exception exception)
    {
        var entry = $"{DateTimeOffset.UtcNow:O} [error] {context}: {exception}";
        Debug.WriteLine(entry);

        try
        {
            var path = ResolveLogFilePath();
            var directory = Path.GetDirectoryName(path);
            if (!string.IsNullOrEmpty(directory))
            {
                Directory.CreateDirectory(directory);
            }

            lock (Gate)
            {
                File.AppendAllText(path, entry + Environment.NewLine);
            }
        }
        catch
        {
            // Logging must never crash the app or hide the original exception.
        }
    }

    private static string ResolveLogFilePath()
    {
        return _logFilePath ??= AppPaths.ResolveLogPath(
            AppContext.BaseDirectory,
            Environment.GetEnvironmentVariable("LOCALAPPDATA"));
    }
}
