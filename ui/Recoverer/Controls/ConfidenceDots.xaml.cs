using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
namespace Recoverer.Controls;
public sealed partial class ConfidenceDots : UserControl
{
    public static readonly DependencyProperty ValueProperty =
        DependencyProperty.Register(nameof(Value), typeof(byte), typeof(ConfidenceDots),
            new PropertyMetadata((byte)0, (d, _) => ((ConfidenceDots)d).ValueText.Text = ((ConfidenceDots)d).Value + "%"));
    public byte Value { get => (byte)GetValue(ValueProperty); set => SetValue(ValueProperty, value); }
    public ConfidenceDots() => InitializeComponent();
}
