using System.IO;
using System.IO.Pipes;
using System.Text;
using Microsoft.UI.Dispatching;

namespace Recoverer.Services;

/// <summary>
/// Async named pipe client that reads newline-delimited JSON events from the engine
/// and allows sending commands. Reconnects automatically if the engine restarts.
/// </summary>
public sealed class PipeClient : IDisposable
{
    private const string PipeName = "recoverer-engine";

    private NamedPipeClientStream? _pipe;
    private StreamReader? _reader;
    private StreamWriter? _writer;
    private CancellationTokenSource _cts = new();
    private readonly DispatcherQueue _dispatcher;

    public event Action<EngineEvent>? EventReceived;
    public event Action? Connected;
    public event Action? Disconnected;

    public PipeClient(DispatcherQueue dispatcher)
    {
        _dispatcher = dispatcher;
    }

    /// <summary>Connect and start the read loop. Returns when connected.</summary>
    public async Task ConnectAsync(CancellationToken ct = default)
    {
        _pipe = new NamedPipeClientStream(".", PipeName, PipeDirection.InOut,
            PipeOptions.Asynchronous);
        using var cts = CancellationTokenSource.CreateLinkedTokenSource(ct);
        cts.CancelAfter(10_000);
        await _pipe.ConnectAsync(cts.Token);

        _reader = new StreamReader(_pipe, Encoding.UTF8, detectEncodingFromByteOrderMarks: false,
            bufferSize: 4096, leaveOpen: true);
        _writer = new StreamWriter(_pipe, Encoding.UTF8, bufferSize: 512, leaveOpen: true)
        {
            AutoFlush = true,
            NewLine = "\n"
        };

        _dispatcher.TryEnqueue(() => Connected?.Invoke());
        _ = ReadLoopAsync(_cts.Token);
    }

    public async Task SendAsync(string commandJson)
    {
        if (_writer is null) return;
        await _writer.WriteLineAsync(commandJson);
    }

    private async Task ReadLoopAsync(CancellationToken ct)
    {
        try
        {
            while (!ct.IsCancellationRequested && _reader is not null)
            {
                var line = await _reader.ReadLineAsync(ct);
                if (line is null) break;
                line = line.Trim();
                if (line.Length == 0) continue;

                var ev = EngineEvent.Deserialize(line);
                if (ev is not null)
                    _dispatcher.TryEnqueue(() => EventReceived?.Invoke(ev));
            }
        }
        catch (OperationCanceledException) { }
        catch (IOException) { }

        _dispatcher.TryEnqueue(() => Disconnected?.Invoke());
    }

    public void Dispose()
    {
        _cts.Cancel();
        _writer?.Dispose();
        _reader?.Dispose();
        _pipe?.Dispose();
        _cts.Dispose();
    }
}
