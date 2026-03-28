using System.Collections.ObjectModel;
using System.IO;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Recoverer.Services;

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

    // Scan depth
    [ObservableProperty] private bool _deepScan = true;  // default: deep

    // Drive list for dropdown
    public ObservableCollection<DriveEntry> Drives { get; } = [];

    [ObservableProperty] private bool _showFirstLaunchBanner = true;

    public SetupViewModel(PipeClient pipe)
    {
        _pipe = pipe;
        RefreshDrives();
    }

    private void RefreshDrives()
    {
        Drives.Clear();
        foreach (var d in DriveInfo.GetDrives().Where(d => d.IsReady && d.DriveType == DriveType.Fixed))
        {
            Drives.Add(new DriveEntry(
                d.Name.TrimEnd('\\'),
                d.VolumeLabel,
                d.TotalSize,
                d.TotalSize - d.AvailableFreeSpace));
            if (SelectedDrive.Length == 0) SelectedDrive = d.Name.TrimEnd('\\');
        }
    }

    [RelayCommand]
    private async Task StartScanAsync()
    {
        var drive = ScanEntireDrive ? SelectedDrive : CustomFolder;
        var depth = DeepScan ? ScanDepth.Deep : ScanDepth.Quick;
        var cats  = BuildCategoryList();
        await _pipe.SendAsync(Commands.StartScan(drive, depth, cats));
    }

    [RelayCommand]
    private void DismissBanner() => ShowFirstLaunchBanner = false;

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

public sealed record DriveEntry(string Letter, string Label, long TotalBytes, long UsedBytes)
{
    public string DisplayName =>
        $"{Letter}  {Label}  ({UsedBytes / 1_073_741_824.0:F1} / {TotalBytes / 1_073_741_824.0:F1} GB)";
}
