using Microsoft.UI.Xaml.Data;

namespace Recoverer;

public class FileSizeConverter : IValueConverter
{
    public object Convert(object v, Type t, object p, string l)
    {
        if (v is not ulong bytes) return "";
        return bytes switch
        {
            < 1024            => $"{bytes} B",
            < 1_048_576       => $"{bytes / 1024.0:F1} KB",
            < 1_073_741_824   => $"{bytes / 1_048_576.0:F1} MB",
            _                 => $"{bytes / 1_073_741_824.0:F1} GB",
        };
    }
    public object ConvertBack(object v, Type t, object p, string l) =>
        throw new NotSupportedException();
}
