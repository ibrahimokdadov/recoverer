using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Imaging;

namespace Recoverer.Controls;

public sealed partial class FilePreviewPanel : UserControl
{
    public static readonly DependencyProperty FileProperty =
        DependencyProperty.Register(nameof(File), typeof(FileRecord), typeof(FilePreviewPanel),
            new PropertyMetadata(null, OnFileChanged));

    public FileRecord? File
    {
        get => (FileRecord?)GetValue(FileProperty);
        set => SetValue(FileProperty, value);
    }

    public FilePreviewPanel() => InitializeComponent();

    private static void OnFileChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        => ((FilePreviewPanel)d).UpdatePreview((FileRecord?)e.NewValue);

    private void UpdatePreview(FileRecord? file)
    {
        if (file is null) return;

        FileNameText.Text = file.DisplayName;
        MetaText.Text     = $"{FormatSize(file.SizeBytes)}  ·  {file.MimeType}  ·  {file.Source}";
        ConfidenceIndicator.Value = file.Confidence;
        ConfidenceLabel.Text = file.Confidence switch
        {
            >= 95 => "Excellent — full recovery expected",
            >= 75 => "Good — minor data loss possible",
            >= 50 => "Fair — file may be partially corrupt",
            >= 25 => "Poor — significant data loss likely",
            _     => "Low — file mostly overwritten"
        };
        LowConfidenceWarning.IsOpen = file.Confidence < 50;

        // Image preview (mime type check)
        ImagePreview.Visibility      = Visibility.Collapsed;
        TextPreviewScroller.Visibility = Visibility.Collapsed;

        if (file.MimeType.StartsWith("image/"))
        {
            // Real preview requires reading raw cluster data from engine — show placeholder
            ImagePreview.Visibility = Visibility.Visible;
        }
        else if (file.MimeType is "text/plain" or "application/pdf")
        {
            TextPreview.Text = $"[{file.MimeType} — {FormatSize(file.SizeBytes)}]";
            TextPreviewScroller.Visibility = Visibility.Visible;
        }
    }

    private static string FormatSize(ulong bytes) => bytes switch
    {
        < 1024          => $"{bytes} B",
        < 1_048_576     => $"{bytes / 1024.0:F1} KB",
        < 1_073_741_824 => $"{bytes / 1_048_576.0:F1} MB",
        _               => $"{bytes / 1_073_741_824.0:F1} GB",
    };
}
