using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Recoverer.Services;

namespace Recoverer.ViewModels;

public sealed partial class ScanningViewModel : ObservableObject
{
    private readonly PipeClient _pipe;

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
                    "vss"      => "Checking Volume Shadow Copies...",
                    "mft_scan" => "Scanning file records...",
                    "carving"  => "Deep scanning unallocated space...",
                    _          => pc.NewPhase
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
                Feed.Insert(0, new DiscoveryItem(
                    ff.Filename ?? $"[carved file]",
                    ff.Category,
                    ff.SizeBytes,
                    ff.Confidence));
                // Keep feed at most 200 items for performance
                if (Feed.Count > 200) Feed.RemoveAt(Feed.Count - 1);
                break;

            case ScanCompleteEvent sc:
                PhaseLabel = $"Scan complete — {sc.TotalFound:N0} files found";
                Pct = 100;
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
}

public sealed record DiscoveryItem(string Name, string Category, ulong SizeBytes, byte Confidence);
