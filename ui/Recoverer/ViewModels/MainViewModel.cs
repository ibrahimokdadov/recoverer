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
    public PipeClient Pipe { get; }
    public EngineProcess Engine { get; }
    public ScanningViewModel Scanning { get; }
    public ResultsViewModel  Results  { get; }

    [ObservableProperty] private AppPhase _phase = AppPhase.Setup;
    [ObservableProperty] private string _statusText = "Ready";

    // Active scan state shared across pages
    [ObservableProperty] private ulong _filesFound;
    [ObservableProperty] private byte  _scanPct;

    public MainViewModel(DispatcherQueue dispatcher)
    {
        Engine   = new EngineProcess();
        Pipe     = new PipeClient(dispatcher);
        Scanning = new ScanningViewModel(Pipe);
        Results  = new ResultsViewModel(Pipe);

        Pipe.EventReceived  += OnEvent;
        Pipe.Disconnected   += () => StatusText = "Engine disconnected";
        Pipe.Connected      += () => StatusText = "Engine connected";
    }

    private void OnEvent(EngineEvent ev)
    {
        switch (ev)
        {
            case PhaseChangeEvent pc:
                if (pc.NewPhase == "vss")
                {
                    Scanning.Reset();
                    Phase = AppPhase.Scanning;
                }
                else if (pc.NewPhase == "mft_scan" || pc.NewPhase == "carving")
                    Phase = AppPhase.Scanning;
                break;
            case FileFoundEvent:
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
