using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Windows.UI;

namespace Recoverer.Controls;

public sealed partial class ConfidenceDots : UserControl
{
    public static readonly DependencyProperty ValueProperty =
        DependencyProperty.Register(nameof(Value), typeof(byte), typeof(ConfidenceDots),
            new PropertyMetadata((byte)0, OnValueChanged));

    public byte Value
    {
        get => (byte)GetValue(ValueProperty);
        set => SetValue(ValueProperty, value);
    }

    public ConfidenceDots() => InitializeComponent();

    private static void OnValueChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        => ((ConfidenceDots)d).Refresh();

    private void Refresh()
    {
        if (Dot1 is null) return;

        // 5-dot scale:  ●●●●● 95-100 green | ●●●●○ 75-94 teal | ●●●○○ 50-74 amber | ●●○○○ 25-49 orange | ●○○○○ 0-24 red
        var (filled, color) = Value switch
        {
            >= 95 => (5, Color.FromArgb(255, 61,  184, 122)),   // green  #3DB87A
            >= 75 => (4, Color.FromArgb(255, 58,  159, 219)),   // teal   #3A9FDB
            >= 50 => (3, Color.FromArgb(255, 224, 160, 32)),    // amber  #E0A020
            >= 25 => (2, Color.FromArgb(255, 220, 120, 40)),    // orange
            _     => (1, Color.FromArgb(255, 217, 88,  88)),    // red    #D95858
        };
        var inactive = Color.FromArgb(255, 60, 60, 60);

        var dots = new[] { Dot1, Dot2, Dot3, Dot4, Dot5 };
        for (int i = 0; i < 5; i++)
            dots[i].Fill = new SolidColorBrush(i < filled ? color : inactive);
    }
}
