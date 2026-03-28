using Microsoft.UI.Xaml;
using Microsoft.UI.Dispatching;
using Recoverer.ViewModels;

namespace Recoverer;

public partial class App : Application
{
    public MainWindow? MainWindow { get; private set; }
    public MainViewModel ViewModel { get; private set; } = null!;

    public static new App Current => (App)Application.Current;

    public App() => InitializeComponent();

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        ViewModel = new MainViewModel(DispatcherQueue.GetForCurrentThread());
        MainWindow = new MainWindow(ViewModel);
        MainWindow.Activate();

        // Start the engine asynchronously after the window is shown
        _ = ViewModel.StartEngineAsync();
    }
}
