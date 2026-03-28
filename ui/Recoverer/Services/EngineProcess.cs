using System.Diagnostics;
using System.IO;
using System.Reflection;

namespace Recoverer.Services;

/// <summary>
/// Manages the lifecycle of the Rust engine subprocess.
/// Locates recoverer-engine.exe next to this assembly.
/// </summary>
public sealed class EngineProcess : IDisposable
{
    private Process? _process;

    public bool IsRunning => _process is { HasExited: false };

    /// <summary>Start the engine. No-op if already running.</summary>
    public void Start()
    {
        if (IsRunning) return;

        var enginePath = FindEnginePath();
        _process = new Process
        {
            StartInfo = new ProcessStartInfo(enginePath)
            {
                UseShellExecute = false,
                CreateNoWindow  = true,
                RedirectStandardOutput = false,
                RedirectStandardError  = false,
            },
        };
        _process.Start();
    }

    public void Stop()
    {
        try { _process?.Kill(entireProcessTree: true); } catch { }
        _process?.Dispose();
        _process = null;
    }

    private static string FindEnginePath()
    {
        var dir = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location)
                  ?? AppContext.BaseDirectory;
        var path = Path.Combine(dir, "recoverer-engine.exe");
        if (!File.Exists(path))
            throw new FileNotFoundException(
                $"Engine not found at '{path}'. Build the Rust engine and copy it to the output directory.",
                path);
        return path;
    }

    public void Dispose() => Stop();
}
