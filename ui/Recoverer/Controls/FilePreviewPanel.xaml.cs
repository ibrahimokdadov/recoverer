using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
namespace Recoverer.Controls;
public sealed partial class FilePreviewPanel : UserControl
{
    public static readonly DependencyProperty FileProperty =
        DependencyProperty.Register(nameof(File), typeof(FileRecord), typeof(FilePreviewPanel),
            new PropertyMetadata(null));
    public FileRecord? File { get => (FileRecord?)GetValue(FileProperty); set => SetValue(FileProperty, value); }
    public FilePreviewPanel() => InitializeComponent();
}
