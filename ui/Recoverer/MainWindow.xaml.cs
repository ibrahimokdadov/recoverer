using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Recoverer.ViewModels;

namespace Recoverer;

public sealed partial class MainWindow : Window
{
    private readonly MainViewModel _vm;

    public MainWindow(MainViewModel vm)
    {
        InitializeComponent();
        _vm = vm;
        _vm.PropertyChanged += (_, e) =>
        {
            if (e.PropertyName == nameof(MainViewModel.Phase))
                UnlockNav(_vm.Phase);
        };
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

    private void UnlockNav(AppPhase phase)
    {
        NavScanning.IsEnabled = phase >= AppPhase.Scanning;
        NavResults.IsEnabled  = phase >= AppPhase.Results;
        NavRecovery.IsEnabled = phase >= AppPhase.Recovery;

        // Auto-navigate to the new phase's screen
        if (phase == AppPhase.Scanning)
        {
            NavView.SelectedItem = NavScanning;
            ContentFrame.Navigate(typeof(Views.ScanningPage));
        }
        else if (phase == AppPhase.Results)
        {
            NavView.SelectedItem = NavResults;
            ContentFrame.Navigate(typeof(Views.ResultsPage));
        }
    }

    public void NavigateToRecovery(IReadOnlyList<FileRecord> files)
    {
        NavRecovery.IsEnabled = true;
        NavView.SelectedItem = NavRecovery;
        ContentFrame.Navigate(typeof(Views.RecoveryPage), files);
    }
}

public enum AppPhase { Setup, Scanning, Results, Recovery }
