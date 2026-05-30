using System.Diagnostics;

namespace Winspot_App.Services;

public sealed class WinspotBackendProcess
{
    private static readonly SemaphoreSlim StartupLock = new(1, 1);

    private Process? _ownedProcess;

    public async Task<bool> TryStartAsync(CancellationToken cancellationToken)
    {
        await StartupLock.WaitAsync(cancellationToken);
        try
        {
            if (IsDaemonRunning())
            {
                return true;
            }

            var executablePath = FindDaemonExecutable();
            if (executablePath is null)
            {
                return false;
            }

            // Track the daemon we spawn so we can shut it down with the app; a
            // daemon that was already running is owned by someone else and left
            // alone (see StopIfOwned).
            _ownedProcess = Process.Start(new ProcessStartInfo
            {
                FileName = executablePath,
                WorkingDirectory = Path.GetDirectoryName(executablePath) ?? AppContext.BaseDirectory,
                UseShellExecute = true,
                WindowStyle = ProcessWindowStyle.Hidden,
            });
        }
        catch
        {
            return false;
        }
        finally
        {
            StartupLock.Release();
        }

        await Task.Delay(150, cancellationToken);
        return true;
    }

    /// Stops the daemon only if this client started it, so closing the launcher
    /// doesn't leave an orphaned backend running.
    public void StopIfOwned()
    {
        var process = Interlocked.Exchange(ref _ownedProcess, null);
        if (process is null)
        {
            return;
        }

        try
        {
            if (!process.HasExited)
            {
                process.Kill(entireProcessTree: true);
            }
        }
        catch
        {
            // Best-effort shutdown; the daemon will exit on its own otherwise.
        }
        finally
        {
            process.Dispose();
        }
    }

    private static bool IsDaemonRunning()
    {
        try
        {
            return Process.GetProcessesByName("winspot-daemon").Length > 0;
        }
        catch
        {
            return false;
        }
    }

    private static string? FindDaemonExecutable()
    {
        var colocated = Path.Combine(AppContext.BaseDirectory, "winspot-daemon.exe");
        if (File.Exists(colocated))
        {
            return colocated;
        }

        for (var directory = new DirectoryInfo(AppContext.BaseDirectory);
             directory is not null;
             directory = directory.Parent)
        {
            if (!File.Exists(Path.Combine(directory.FullName, "Cargo.toml")))
            {
                continue;
            }

            var debugExecutable = Path.Combine(
                directory.FullName,
                "target",
                "debug",
                "winspot-daemon.exe");
            return File.Exists(debugExecutable) ? debugExecutable : null;
        }

        return null;
    }
}
