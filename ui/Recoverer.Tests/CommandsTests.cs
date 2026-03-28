using System.Text.Json;
using Xunit;

namespace Recoverer.Tests;

public class CommandsTests
{
    private static JsonElement Parse(string json) =>
        JsonDocument.Parse(json).RootElement;

    [Fact]
    public void Ping_has_correct_type()
    {
        var e = Parse(Commands.Ping());
        Assert.Equal("Ping", e.GetProperty("type").GetString());
    }

    [Fact]
    public void StartScan_serializes_all_fields()
    {
        var e = Parse(Commands.StartScan("C:\\", ScanDepth.Deep, ["Images", "Videos"]));
        Assert.Equal("StartScan", e.GetProperty("type").GetString());
        Assert.Equal("C:\\", e.GetProperty("drive").GetString());
        Assert.Equal("deep", e.GetProperty("depth").GetString());
        Assert.Equal(2, e.GetProperty("categories").GetArrayLength());
    }

    [Fact]
    public void QueryFiles_serializes_snake_case()
    {
        var e = Parse(Commands.QueryFiles("Images", 80, "vacation", 0, 50));
        Assert.Equal("QueryFiles", e.GetProperty("type").GetString());
        Assert.Equal("Images", e.GetProperty("category").GetString());
        Assert.Equal(80, e.GetProperty("min_confidence").GetInt32());
        Assert.Equal("vacation", e.GetProperty("name_contains").GetString());
    }

    [Fact]
    public void RecoverFiles_serializes_snake_case()
    {
        var e = Parse(Commands.RecoverFiles([1L, 2L, 3L], "D:\\Recovered", true));
        Assert.Equal("RecoverFiles", e.GetProperty("type").GetString());
        Assert.Equal(3, e.GetProperty("file_ids").GetArrayLength());
        Assert.True(e.GetProperty("recreate_structure").GetBoolean());
    }
}
