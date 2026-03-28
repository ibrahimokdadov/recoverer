using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;

namespace Recoverer;

public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
        ContentFrame.Navigate(typeof(Views.SetupPage));
        NavView.SelectedItem = NavSetup;
    }

    private void NavView_SelectionChanged(NavigationView sender,
        NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItem is NavigationViewItem item)
        {
            var page = (item.Tag as string) switch
            {
                "Setup"    => typeof(Views.SetupPage),
                "Scanning" => typeof(Views.ScanningPage),
                "Results"  => typeof(Views.ResultsPage),
                "Recovery" => typeof(Views.RecoveryPage),
                _          => null
            };
            if (page is not null)
                ContentFrame.Navigate(page);
        }
    }

    /// <summary>Enable nav items as the scan progresses through phases.</summary>
    public void UnlockNav(AppPhase phase)
    {
        NavScanning.IsEnabled = phase >= AppPhase.Scanning;
        NavResults.IsEnabled  = phase >= AppPhase.Results;
        NavRecovery.IsEnabled = phase >= AppPhase.Recovery;
    }
}

public enum AppPhase { Setup, Scanning, Results, Recovery }
