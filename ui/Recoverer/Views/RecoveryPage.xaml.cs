using Windows.Storage.Pickers;
using WinRT.Interop;
using Microsoft.UI.Xaml.Controls;
using Recoverer.ViewModels;

namespace Recoverer.Views;

public sealed partial class RecoveryPage : Page
{
    public RecoveryViewModel ViewModel { get; }

    public RecoveryPage()
    {
        ViewModel = new RecoveryViewModel(App.Current.ViewModel.Pipe);
        InitializeComponent();
    }

    protected override void OnNavigatedTo(Microsoft.UI.Xaml.Navigation.NavigationEventArgs e)
    {
        if (e.Parameter is IReadOnlyList<FileRecord> files)
            ViewModel.SetFiles(files);
    }

    private async void BrowseDestination_Click(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        var picker = new FolderPicker();
        picker.FileTypeFilter.Add("*");
        InitializeWithWindow.Initialize(picker,
            WindowNative.GetWindowHandle(App.Current.MainWindow!));
        var folder = await picker.PickSingleFolderAsync();
        if (folder is not null) ViewModel.Destination = folder.Path;
    }

    private void OpenExplorer_Click(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
        => ViewModel.OpenDestinationInExplorer();

    private void ScanAgain_Click(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        var win = App.Current.MainWindow!;
        win.NavigateToSetup();
    }

    protected override void OnNavigatedFrom(Microsoft.UI.Xaml.Navigation.NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        ViewModel.Detach();
    }
}
