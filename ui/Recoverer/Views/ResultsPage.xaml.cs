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
        ViewModel = App.Current.ViewModel.Results;
        InitializeComponent();
    }

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ViewModel.ResetAndReload();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        _searchDebounce?.Cancel();
        _searchDebounce?.Dispose();
        _searchDebounce = null;
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
        var win = App.Current.MainWindow!;
        win.NavigateToRecovery(ViewModel.SelectedFiles.ToList());
    }

    private async void RecoverAllButton_Click(object sender, Microsoft.UI.Xaml.RoutedEventArgs e)
    {
        var all = await ViewModel.FetchAllFilteredAsync();
        if (all.Count == 0) return;
        App.Current.MainWindow!.NavigateToRecovery(all);
    }
}
