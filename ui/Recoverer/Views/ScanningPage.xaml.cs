using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using Recoverer.ViewModels;

namespace Recoverer.Views;

public sealed partial class ScanningPage : Page
{
    public ScanningViewModel ViewModel { get; }

    public ScanningPage()
    {
        ViewModel = new ScanningViewModel(App.Current.ViewModel.Pipe);
        InitializeComponent();
    }

    protected override void OnNavigatedFrom(NavigationEventArgs e)
    {
        base.OnNavigatedFrom(e);
        ViewModel.Detach();
    }
}
