using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Recoverer.Controls;

public sealed partial class CategoryRow : UserControl
{
    public static readonly DependencyProperty LabelProperty =
        DependencyProperty.Register(nameof(Label), typeof(string), typeof(CategoryRow),
            new PropertyMetadata("", OnChanged));
    public static readonly DependencyProperty CountProperty =
        DependencyProperty.Register(nameof(Count), typeof(ulong), typeof(CategoryRow),
            new PropertyMetadata(0UL, OnChanged));
    public static readonly DependencyProperty TotalProperty =
        DependencyProperty.Register(nameof(Total), typeof(ulong), typeof(CategoryRow),
            new PropertyMetadata(0UL, OnChanged));

    public string Label { get => (string)GetValue(LabelProperty); set => SetValue(LabelProperty, value); }
    public ulong  Count { get => (ulong)GetValue(CountProperty);  set => SetValue(CountProperty, value); }
    public ulong  Total { get => (ulong)GetValue(TotalProperty);  set => SetValue(TotalProperty, value); }

    public CategoryRow() => InitializeComponent();

    private static void OnChanged(DependencyObject d, DependencyPropertyChangedEventArgs _)
    {
        var self = (CategoryRow)d;
        self.LabelText.Text  = self.Label;
        self.CountText.Text  = self.Count.ToString("N0");
        self.Bar.Value       = self.Total > 0 ? (double)self.Count / self.Total * 100.0 : 0;
    }
}
