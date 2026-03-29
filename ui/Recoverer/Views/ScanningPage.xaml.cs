using Microsoft.UI.Xaml.Controls;
using Recoverer.ViewModels;

namespace Recoverer.Views;

public sealed partial class ScanningPage : Page
{
    public ScanningViewModel ViewModel { get; }

    public ScanningPage()
    {
        ViewModel = App.Current.ViewModel.Scanning;
        InitializeComponent();
    }
}
