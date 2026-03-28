using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Recoverer.Services;

namespace Recoverer.ViewModels;

public sealed partial class RecoveryViewModel : ObservableObject
{
    private readonly PipeClient _pipe;

    [ObservableProperty] private string  _destination = "";
    [ObservableProperty] private bool    _recreateStructure = true;
    [ObservableProperty] private bool    _isRecovering;
    [ObservableProperty] private ulong   _recovered;
    [ObservableProperty] private ulong   _warnings;
    [ObservableProperty] private ulong   _failed;
    [ObservableProperty] private ulong   _total;
    [ObservableProperty] private bool    _isComplete;
    [ObservableProperty] private bool    _showSameVolumeWarning;

    /// <summary>Progress 0-100 for ProgressBar (which requires double, not ulong).</summary>
    public double ProgressPct =>
        Total > 0 ? Math.Round((double)Recovered / Total * 100.0, 1) : 0.0;

    public IReadOnlyList<FileRecord> FilesToRecover { get; private set; } = [];

    public RecoveryViewModel(PipeClient pipe)
    {
        _pipe = pipe;
        _pipe.EventReceived += OnEvent;

        // Default destination: user's desktop
        Destination = System.IO.Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.Desktop),
            "Recovered Files");
    }

    public void SetFiles(IReadOnlyList<FileRecord> files)
    {
        FilesToRecover = files;
        Total = (ulong)files.Count;
    }

    [RelayCommand]
    private async Task StartRecoveryAsync()
    {
        if (string.IsNullOrWhiteSpace(Destination) || FilesToRecover.Count == 0) return;

        IsRecovering = true;
        IsComplete   = false;
        Recovered = Warnings = Failed = 0;

        try
        {
            var ids = FilesToRecover.Select(f => f.Id).ToArray();
            await _pipe.SendAsync(Commands.RecoverFiles(ids, Destination, RecreateStructure));
        }
        catch
        {
            IsRecovering = false;
        }
    }

    [RelayCommand]
    private async Task CancelRecoveryAsync()
    {
        try { await _pipe.SendAsync(Commands.CancelScan()); }
        catch { /* pipe already closed */ }
    }

    private void OnEvent(EngineEvent ev)
    {
        switch (ev)
        {
            case RecoveryProgressEvent rp:
                Recovered = rp.Recovered;
                Warnings  = rp.Warnings;
                Failed    = rp.Failed;
                Total     = rp.Total;
                break;
            case RecoveryCompleteEvent rc:
                Recovered    = rc.Recovered;
                Warnings     = rc.Warnings;
                Failed       = rc.Failed;
                IsRecovering = false;
                IsComplete   = true;
                break;
            case ErrorEvent err when err.Code == "SAME_VOLUME":
                ShowSameVolumeWarning = true;
                IsRecovering = false;
                break;
        }
    }

    public bool CanStartRecovery => !IsRecovering && !IsComplete;

    partial void OnRecoveredChanged(ulong value) => OnPropertyChanged(nameof(ProgressPct));
    partial void OnTotalChanged(ulong value)     => OnPropertyChanged(nameof(ProgressPct));
    partial void OnIsRecoveringChanged(bool value) => OnPropertyChanged(nameof(CanStartRecovery));
    partial void OnIsCompleteChanged(bool value) => OnPropertyChanged(nameof(CanStartRecovery));

    public void OpenDestinationInExplorer()
    {
        if (System.IO.Directory.Exists(Destination))
            System.Diagnostics.Process.Start("explorer.exe", Destination);
    }

    public void Detach() => _pipe.EventReceived -= OnEvent;
}
