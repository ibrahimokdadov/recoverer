using Windows.Storage.Pickers;
using WinRT.Interop;
using Microsoft.UI.Xaml.Controls;
using Recoverer.ViewModels;

namespace Recoverer.Views;

public sealed partial class SetupPage : Page
{
    public SetupViewModel ViewModel { get; }

    public SetupPage()
    {
        ViewModel = new SetupViewModel(App.Current.ViewModel.Pipe);
        InitializeComponent();
    }

    private async void BrowseFolder_Click(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        var picker = new FolderPicker();
        picker.FileTypeFilter.Add("*");
        InitializeWithWindow.Initialize(picker,
            WindowNative.GetWindowHandle(App.Current.MainWindow!));
        var folder = await picker.PickSingleFolderAsync();
        if (folder is not null) ViewModel.CustomFolder = folder.Path;
    }
}
