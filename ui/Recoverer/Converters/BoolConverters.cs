using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Data;

namespace Recoverer;  // in root namespace so x:Key can find them in App.xaml

public class InverseBoolConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) => v is bool b && !b;
    public object ConvertBack(object v, Type t, object p, string l) => v is bool b && !b;
}

public class InverseBoolToVisibilityConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) =>
        v is bool b && !b ? Visibility.Visible : Visibility.Collapsed;
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}

public class BoolToVisibilityConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) =>
        v is bool b && b ? Visibility.Visible : Visibility.Collapsed;
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}

public class NullToVisibilityConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) =>
        v is null ? Visibility.Collapsed : Visibility.Visible;
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}

public class PauseResumeTextConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) =>
        v is bool paused && paused ? "Resume" : "Pause";
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}

public class PercentConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) => $"{v}%";
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}

public class CountToBoolConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) => v is int n && n > 0;
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}

public class CountToVisibilityConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) =>
        v is int n && n > 0 ? Visibility.Visible : Visibility.Collapsed;
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}

public class InverseCountToVisibilityConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) =>
        v is int n && n == 0 ? Visibility.Visible : Visibility.Collapsed;
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}

public class CategoryToGlyphConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l) => (v as string) switch
    {
        "Images"    => "\uEB9F",   // Photo
        "Videos"    => "\uE714",   // Video
        "Documents" => "\uE8A5",   // Document
        "Audio"     => "\uE8D6",   // MusicInfo
        "Archives"  => "\uE8B7",   // ZipFolder
        _           => "\uE8CF",   // Globe / Other
    };
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}
