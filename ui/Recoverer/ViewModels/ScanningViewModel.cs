using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Recoverer.Services;

namespace Recoverer.ViewModels;

public sealed partial class ScanningViewModel : ObservableObject
{
    private readonly PipeClient _pipe;
    private ulong _feedEventCounter;

    [ObservableProperty] private string _phaseLabel = "Checking Volume Shadow Copies...";
    [ObservableProperty] private byte   _pct;
    [ObservableProperty] private ulong  _filesFound;
    [ObservableProperty] private string _etaText = "";
    [ObservableProperty] private string _currentPath = "";
    [ObservableProperty] private bool   _isPaused;

    // Category counts
    [ObservableProperty] private ulong _imagesCount;
    [ObservableProperty] private ulong _videosCount;
    [ObservableProperty] private ulong _documentsCount;
    [ObservableProperty] private ulong _audioCount;
    [ObservableProperty] private ulong _archivesCount;
    [ObservableProperty] private ulong _otherCount;

    // Live discovery feed — newest items first
    public ObservableCollection<DiscoveryItem> Feed { get; } = [];

    public ScanningViewModel(PipeClient pipe)
    {
        _pipe = pipe;
        _pipe.EventReceived += OnEvent;
    }

    private void OnEvent(EngineEvent ev)
    {
        switch (ev)
        {
            case PhaseChangeEvent pc:
                PhaseLabel = pc.NewPhase switch
                {
                    "vss"               => "Checking Volume Shadow Copies...",
                    "mft_scan"          => "Scanning file records...",
                    "carving"           => "Deep scanning unallocated space...",
                    "fragment_grouping" => "Grouping fragment chains...",
                    _                   => pc.NewPhase
                };
                break;

            case ProgressEvent pr:
                Pct        = pr.Pct;
                FilesFound = pr.FilesFound;
                EtaText    = pr.EtaSecs.HasValue
                    ? FormatEta(pr.EtaSecs.Value)
                    : "";
                break;

            case FileFoundEvent ff:
                IncrementCategory(ff.Category);
                // Only update the live feed every 50 events to keep the UI responsive
                if (++_feedEventCounter % 50 == 0)
                {
                    Feed.Insert(0, new DiscoveryItem(
                        ff.Filename ?? "[carved file]",
                        ff.Category,
                        ff.SizeBytes,
                        ff.Confidence));
                    if (Feed.Count > 200) Feed.RemoveAt(Feed.Count - 1);
                }
                break;

            case ScanCompleteEvent sc:
                PhaseLabel = $"Scan complete — {sc.TotalFound:N0} files found";
                Pct = 100;
                // Cross-reference new scan against recovery history so previously
                // recovered files on this drive show up as recovered immediately.
                _ = _pipe.SendAsync(Commands.ApplyScanHistory());
                break;

            case ErrorEvent err:
                PhaseLabel = $"Engine error ({err.Code}): {err.Message}";
                break;
        }
    }

    [RelayCommand]
    private async Task PauseResumeAsync()
    {
        try
        {
            if (IsPaused)
            {
                await _pipe.SendAsync(Commands.ResumeScan());
                IsPaused = false;
            }
            else
            {
                await _pipe.SendAsync(Commands.PauseScan());
                IsPaused = true;
            }
        }
        catch (Exception) { /* send failed — IsPaused unchanged, button keeps correct label */ }
    }

    [RelayCommand]
    private async Task CancelAsync() =>
        await _pipe.SendAsync(Commands.CancelScan());

    private void IncrementCategory(string cat)
    {
        switch (cat)
        {
            case "Images":    ImagesCount++;    break;
            case "Videos":    VideosCount++;    break;
            case "Documents": DocumentsCount++; break;
            case "Audio":     AudioCount++;     break;
            case "Archives":  ArchivesCount++;  break;
            default:          OtherCount++;     break;
        }
    }

    private static string FormatEta(ulong secs) => secs switch
    {
        < 60   => $"{secs}s remaining",
        < 3600 => $"{secs / 60}m {secs % 60}s remaining",
        _      => $"{secs / 3600}h {secs % 3600 / 60}m remaining"
    };

    public void Detach() => _pipe.EventReceived -= OnEvent;

    public void Reset()
    {
        PhaseLabel     = "Checking Volume Shadow Copies...";
        Pct            = 0;
        FilesFound     = 0;
        EtaText        = "";
        CurrentPath    = "";
        IsPaused       = false;
        ImagesCount    = 0;
        VideosCount    = 0;
        DocumentsCount = 0;
        AudioCount     = 0;
        ArchivesCount  = 0;
        OtherCount     = 0;
        _feedEventCounter = 0;
        Feed.Clear();
    }
}

public sealed record DiscoveryItem(string Name, string Category, ulong SizeBytes, byte Confidence);
