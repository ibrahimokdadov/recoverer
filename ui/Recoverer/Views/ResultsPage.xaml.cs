using System.Threading;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using Recoverer.ViewModels;

namespace Recoverer.Views;

public sealed partial class ResultsPage : Page
{
    public ResultsViewModel ViewModel { get; }
    private CancellationTokenSource? _searchDebounce;

    public ResultsPage()
    {
        ViewModel = new ResultsViewModel(App.Current.ViewModel.Pipe);
        InitializeComponent();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        _searchDebounce?.Cancel();
        _searchDebounce?.Dispose();
        _searchDebounce = null;
        ViewModel.Detach();
    }

    private async void SearchBox_TextChanged(object sender, TextChangedEventArgs e)
    {
        _searchDebounce?.Cancel();
        _searchDebounce?.Dispose();
        _searchDebounce = new CancellationTokenSource();
        try
        {
            await Task.Delay(200, _searchDebounce.Token);
            ViewModel.ApplySearch(SearchBox.Text);
        }
        catch (OperationCanceledException) { }
    }

    private void FileList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        ViewModel.SelectedFiles.Clear();
        foreach (FileRecord f in FileList.SelectedItems)
            ViewModel.SelectedFiles.Add(f);

        ViewModel.SetPreview(FileList.SelectedItems.Count == 1
            ? (FileRecord)FileList.SelectedItems[0]
            : null);
    }

    private void RecoverButton_Click(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        // Navigate to Recovery screen and pass selected files
        var win = App.Current.MainWindow!;
        win.NavigateToRecovery(ViewModel.SelectedFiles.ToList());
    }
}
