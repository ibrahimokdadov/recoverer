using CommunityToolkit.Mvvm.ComponentModel;
using Recoverer.Services;
using Microsoft.UI.Dispatching;

namespace Recoverer.ViewModels;

/// <summary>
/// App-wide state: owns PipeClient + EngineProcess, tracks current scan phase,
/// and notifies MainWindow to unlock nav items.
/// </summary>
public sealed partial class MainViewModel : ObservableObject, IDisposable
{
    public readonly PipeClient Pipe;
    public readonly EngineProcess Engine;

    [ObservableProperty] private AppPhase _phase = AppPhase.Setup;
    [ObservableProperty] private string _statusText = "Ready";

    // Active scan state shared across pages
    [ObservableProperty] private ulong _filesFound;
    [ObservableProperty] private byte  _scanPct;

    public MainViewModel(DispatcherQueue dispatcher)
    {
        Engine = new EngineProcess();
        Pipe   = new PipeClient(dispatcher);

        Pipe.EventReceived  += OnEvent;
        Pipe.Disconnected   += () => StatusText = "Engine disconnected";
        Pipe.Connected      += () => StatusText = "Engine connected";
    }

    public async Task StartEngineAsync()
    {
        Engine.Start();
        // Give the engine 1.5s to open the pipe before connecting
        await Task.Delay(1500);
        await Pipe.ConnectAsync();
    }

    private void OnEvent(EngineEvent ev)
    {
        switch (ev)
        {
            case PhaseChangeEvent pc:
                if (pc.NewPhase == "mft_scan" || pc.NewPhase == "vss")
                    Phase = AppPhase.Scanning;
                break;
            case ScanCompleteEvent:
                Phase = AppPhase.Results;
                break;
            case ProgressEvent pr:
                FilesFound = pr.FilesFound;
                ScanPct    = pr.Pct;
                break;
        }
    }

    public void Dispose()
    {
        Pipe.Dispose();
        Engine.Dispose();
    }
}
