using System.Collections.ObjectModel;
using System.IO;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Recoverer.Services;
using Recoverer;

namespace Recoverer.ViewModels;

public sealed partial class SetupViewModel : ObservableObject
{
    private readonly PipeClient _pipe;

    // Drive selection
    [ObservableProperty] private bool   _scanEntireDrive = true;
    [ObservableProperty] private string _selectedDrive   = "";
    [ObservableProperty] private string _customFolder    = "";

    // Category filters (all on by default)
    [ObservableProperty] private bool _wantImages    = true;
    [ObservableProperty] private bool _wantVideos    = true;
    [ObservableProperty] private bool _wantDocuments = true;
    [ObservableProperty] private bool _wantAudio     = true;
    [ObservableProperty] private bool _wantArchives  = true;
    [ObservableProperty] private bool _wantOther     = true;

    // Scan depth (mutually exclusive)
    [ObservableProperty] private bool _quickScan  = false;
    [ObservableProperty] private bool _deepScan   = true;
    [ObservableProperty] private bool _carveOnly  = false;

    // Each setter only fires when value==true, and OnXChanged(false) is a no-op,
    // so there is no mutual recursion.
    partial void OnQuickScanChanged(bool value)  { if (value) { DeepScan = false; CarveOnly = false; } }
    partial void OnDeepScanChanged(bool value)   { if (value) { QuickScan = false; CarveOnly = false; } }
    partial void OnCarveOnlyChanged(bool value)  { if (value) { QuickScan = false; DeepScan = false; } }

    // Drive list for dropdown
    public ObservableCollection<DriveEntry> Drives { get; } = [];

    // Previous scan sessions
    public ObservableCollection<ScanSession> Sessions { get; } = [];

    [ObservableProperty] private bool _showFirstLaunchBanner = true;

    // Raised when the user picks a session to browse — SetupPage handles navigation
    public event Action? ResultsRequested;

    public SetupViewModel(PipeClient pipe)
    {
        _pipe = pipe;
        _pipe.Connected    += () => {
            StartScanCommand.NotifyCanExecuteChanged();
            _ = RefreshSessionsAsync();
        };
        _pipe.Disconnected += () => StartScanCommand.NotifyCanExecuteChanged();
        _pipe.EventReceived += OnEvent;
        RefreshDrives();
    }

    private void OnEvent(EngineEvent ev)
    {
        if (ev is SessionsListEvent sl)
        {
            Sessions.Clear();
            foreach (var s in sl.Sessions) Sessions.Add(s);
        }
    }

    private async Task RefreshSessionsAsync() =>
        await _pipe.SendAsync(Commands.ListSessions());

    [RelayCommand]
    private async Task BrowseSessionAsync(ScanSession session)
    {
        await _pipe.SendAsync(Commands.SwitchSession(session.Id));
        ResultsRequested?.Invoke();
    }

    private void RefreshDrives()
    {
        Drives.Clear();
        foreach (var d in DriveInfo.GetDrives().Where(d => d.IsReady &&
            (d.DriveType == DriveType.Fixed || d.DriveType == DriveType.Removable)))
        {
            Drives.Add(new DriveEntry(
                d.Name.TrimEnd('\\'),
                d.VolumeLabel,
                d.TotalSize,
                d.TotalSize - d.AvailableFreeSpace,
                d.DriveType == DriveType.Removable));
            if (SelectedDrive.Length == 0) SelectedDrive = d.Name.TrimEnd('\\');
        }
    }

    [RelayCommand(CanExecute = nameof(CanStartScan))]
    private async Task StartScanAsync()
    {
        var drive = ScanEntireDrive ? SelectedDrive : ExtractDriveLetter(CustomFolder);
        var depth = CarveOnly ? ScanDepth.CarveOnly : DeepScan ? ScanDepth.Deep : ScanDepth.Quick;
        var cats  = BuildCategoryList();
        await _pipe.SendAsync(Commands.StartScan(drive, depth, cats));
    }

    private static string ExtractDriveLetter(string path) =>
        path.Length >= 2 && path[1] == ':' ? path[..2] : path;

    private bool CanStartScan() =>
        _pipe.IsConnected &&
        (ScanEntireDrive ? SelectedDrive.Length > 0 : CustomFolder.Length > 0);

    [RelayCommand]
    private void DismissBanner() => ShowFirstLaunchBanner = false;

    partial void OnScanEntireDriveChanged(bool value) =>
        StartScanCommand.NotifyCanExecuteChanged();

    partial void OnSelectedDriveChanged(string value) =>
        StartScanCommand.NotifyCanExecuteChanged();

    partial void OnCustomFolderChanged(string value) =>
        StartScanCommand.NotifyCanExecuteChanged();

    private IEnumerable<string> BuildCategoryList()
    {
        // Empty list = all categories
        if (WantImages && WantVideos && WantDocuments && WantAudio && WantArchives && WantOther)
            return [];

        var list = new List<string>();
        if (WantImages)    list.Add("Images");
        if (WantVideos)    list.Add("Videos");
        if (WantDocuments) list.Add("Documents");
        if (WantAudio)     list.Add("Audio");
        if (WantArchives)  list.Add("Archives");
        if (WantOther)     list.Add("Other");
        return list;
    }
}

public sealed record DriveEntry(string Letter, string Label, long TotalBytes, long UsedBytes, bool IsRemovable = false)
{
    public string DisplayName =>
        $"{Letter}  {Label}{(IsRemovable ? " [USB]" : "")}  ({UsedBytes / 1_073_741_824.0:F1} / {TotalBytes / 1_073_741_824.0:F1} GB)";
}
