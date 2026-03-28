using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Recoverer.Services;

namespace Recoverer.ViewModels;

public sealed partial class ResultsViewModel : ObservableObject
{
    private readonly PipeClient _pipe;
    private const ulong PageSize = 100;
    private ulong _offset;

    // Filters
    [ObservableProperty][NotifyPropertyChangedFor(nameof(HasActiveFilters))]
    private string? _filterCategory;

    [ObservableProperty][NotifyPropertyChangedFor(nameof(HasActiveFilters))]
    private int? _minConfidence;

    [ObservableProperty] private string _searchText = "";

    public bool HasActiveFilters => FilterCategory is not null || MinConfidence is not null
        || SearchText.Length > 0;

    // Results
    public ObservableCollection<FileRecord> Files { get; } = [];
    [ObservableProperty] private long _totalCount;
    [ObservableProperty] private bool _isLoading;

    // Selection
    public ObservableCollection<FileRecord> SelectedFiles { get; } = [];
    [ObservableProperty] private FileRecord? _previewFile;

    public ResultsViewModel(PipeClient pipe)
    {
        _pipe = pipe;
        _pipe.EventReceived += OnEvent;
        _ = LoadPageAsync();
    }

    [RelayCommand]
    private async Task LoadPageAsync()
    {
        IsLoading = true;
        await _pipe.SendAsync(Commands.QueryFiles(
            FilterCategory, MinConfidence,
            SearchText.Length > 0 ? SearchText : null,
            _offset, PageSize));
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

    [RelayCommand]
    private void SetFilter(string? category)
    {
        FilterCategory = category;
        ResetAndReload();
    }

    [RelayCommand]
    private void SetConfidenceFilter(string? value)
    {
        MinConfidence = int.TryParse(value, out var n) ? n : null;
        ResetAndReload();
    }

    private void ResetAndReload()
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
            TotalCount = fp.TotalCount;
            foreach (var f in fp.Files) Files.Add(f);
            IsLoading = false;
        }
    }

    public void SetPreview(FileRecord? file) => PreviewFile = file;

    public void Detach() => _pipe.EventReceived -= OnEvent;
}
