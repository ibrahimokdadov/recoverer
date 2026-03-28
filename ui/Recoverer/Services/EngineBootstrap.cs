using Microsoft.UI.Xaml.Controls;

namespace Recoverer.Services;

/// <summary>
/// Checks preconditions and shows friendly errors before connecting.
/// </summary>
public static class EngineBootstrap
{
    public static async Task<bool> StartWithErrorHandlingAsync(
        EngineProcess engine, PipeClient pipe, MainWindow window)
    {
        try
        {
            engine.Start();
        }
        catch (System.IO.FileNotFoundException ex)
        {
            await ShowError(window,
                "Engine not found",
                $"recoverer-engine.exe was not found.\n\n{ex.Message}\n\nBuild the Rust engine first: cargo build --release -p recoverer-engine");
            return false;
        }

        await Task.Delay(1500);

        try
        {
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(8));
            await pipe.ConnectAsync(cts.Token);
            return true;
        }
        catch (Exception ex)
        {
            await ShowError(window, "Connection failed",
                $"Could not connect to the scan engine.\n\n{ex.Message}");
            return false;
        }
    }

    private static async Task ShowError(MainWindow window, string title, string message)
    {
        var dialog = new ContentDialog
        {
            Title = title,
            Content = message,
            CloseButtonText = "OK",
            XamlRoot = window.Content.XamlRoot
        };
        await dialog.ShowAsync();
    }
}
