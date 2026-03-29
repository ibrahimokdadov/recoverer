using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Recoverer.Services;

namespace Recoverer.ViewModels;

public sealed partial class ResultsViewModel : ObservableObject
{
    private readonly PipeClient _pipe;
    private const ulong PageSize = 500;
    private ulong _offset;

    // Filters
    [ObservableProperty][NotifyPropertyChangedFor(nameof(HasActiveFilters))]
    private string? _filterCategory;

    [ObservableProperty][NotifyPropertyChangedFor(nameof(HasActiveFilters))]
    private int? _minConfidence;

    // Slider value (0 = no filter). Drives MinConfidence.
    [ObservableProperty] private int _confidenceThreshold = 0;
    partial void OnConfidenceThresholdChanged(int value)
    {
        MinConfidence = value > 0 ? value : null;
        ResetAndReload();
    }

    [ObservableProperty][NotifyPropertyChangedFor(nameof(HasActiveFilters))]
    private bool _hideRecovered;
    partial void OnHideRecoveredChanged(bool _) => ResetAndReload();

    [ObservableProperty] private bool _collapseFragments = true;
    partial void OnCollapseFragmentsChanged(bool _) => ResetAndReload();

    [ObservableProperty] private string _searchText = "";

    public bool HasActiveFilters => FilterCategory is not null || MinConfidence is not null
        || SearchText.Length > 0;

    // Results
    public ObservableCollection<FileRecord> Files { get; } = [];
    [ObservableProperty][NotifyPropertyChangedFor(nameof(HasMore))][NotifyPropertyChangedFor(nameof(RemainingCount))]
    private long _totalCount;
    [ObservableProperty] private bool _isLoading;

    public bool HasMore       => Files.Count > 0 && (long)Files.Count < TotalCount;
    public long RemainingCount => TotalCount - Files.Count;
    private bool _suppressNextFilesPage;

    // Selection
    public ObservableCollection<FileRecord> SelectedFiles { get; } = [];
    [ObservableProperty] private FileRecord? _previewFile;

    public ResultsViewModel(PipeClient pipe)
    {
        _pipe = pipe;
        _pipe.EventReceived += OnEvent;
    }

    [RelayCommand]
    private async Task LoadPageAsync()
    {
        IsLoading = true;
        await _pipe.SendAsync(Commands.QueryFiles(
            FilterCategory, MinConfidence,
            SearchText.Length > 0 ? SearchText : null,
            _offset, PageSize, HideRecovered, CollapseFragments));
    }

    [RelayCommand]
    private async Task LoadMoreAsync()
    {
        _offset += PageSize;
        await LoadPageAsync();
    }

    [RelayCommand]
    private void SelectAll()
    {
        SelectedFiles.Clear();
        foreach (var f in Files) SelectedFiles.Add(f);
    }

    [RelayCommand]
    private void SelectHighConfidence()
    {
        SelectedFiles.Clear();
        foreach (var f in Files.Where(f => f.Confidence >= 80))
            SelectedFiles.Add(f);
    }

    // Fetches ALL file IDs matching the current filter (not just the loaded page)
    // and returns them so the caller can navigate to recovery.
    public async Task<List<FileRecord>> FetchAllFilteredAsync()
    {
        // Re-use a single large-limit query — TotalCount is the authoritative count
        var tcs = new TaskCompletionSource<List<FileRecord>>();

        void Handler(EngineEvent ev)
        {
            if (ev is FilesPageEvent fp)
            {
                _pipe.EventReceived -= Handler;
                tcs.TrySetResult([.. fp.Files]);
            }
        }

        _suppressNextFilesPage = true;
        _pipe.EventReceived += Handler;
        await _pipe.SendAsync(Commands.QueryFiles(
            FilterCategory, MinConfidence,
            SearchText.Length > 0 ? SearchText : null,
            offset: 0, limit: TotalCount > 0 ? (ulong)TotalCount : 10_000,
            excludeRecovered: HideRecovered, collapseFragments: CollapseFragments));

        // Safety timeout — if no reply in 10 s, return empty
        var timeout = Task.Delay(10_000);
        var winner = await Task.WhenAny(tcs.Task, timeout);
        if (winner == timeout)
        {
            _pipe.EventReceived -= Handler;
            return [];
        }
        return await tcs.Task;
    }

    [RelayCommand]
    private void SetFilter(string? category)
    {
        FilterCategory = category;
        ResetAndReload();
    }

    public void ResetAndReload()
    {
        _offset = 0;
        Files.Clear();
        _ = LoadPageAsync();
    }

    // Debounced search — called from code-behind after 200ms
    public void ApplySearch(string text)
    {
        SearchText = text;
        ResetAndReload();
    }

    private void OnEvent(EngineEvent ev)
    {
        if (ev is FilesPageEvent fp)
        {
            if (_suppressNextFilesPage) { _suppressNextFilesPage = false; return; }
            TotalCount = fp.TotalCount;
            foreach (var f in fp.Files) Files.Add(f);
            IsLoading = false;
            OnPropertyChanged(nameof(HasMore));
            OnPropertyChanged(nameof(RemainingCount));
        }
        else if (ev is RecoveryCompleteEvent)
        {
            // Persist newly recovered clusters to the shared index so future scans
            // of the same drive start with those files already marked as recovered.
            _ = _pipe.SendAsync(Commands.ApplyScanHistory());
        }
        else if (ev is PhaseChangeEvent pc && (pc.NewPhase == "vss" || pc.NewPhase == "mft_scan"))
        {
            // New scan started — clear stale results
            Files.Clear();
            SelectedFiles.Clear();
            TotalCount = 0;
            _offset = 0;
        }
    }

    public void SetPreview(FileRecord? file) => PreviewFile = file;

    public void Detach() => _pipe.EventReceived -= OnEvent;
}
