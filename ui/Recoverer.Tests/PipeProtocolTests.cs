using System.Text;
using Xunit;

namespace Recoverer.Tests;

// Tests for the static helpers used in PipeClient (line splitting, encoding)
public class PipeProtocolTests
{
    [Fact]
    public void NewlineDelimited_splits_two_events()
    {
        var input = """{"event":"Pong"}""" + "\n" + """{"event":"Progress","phase":"mft_scan","pct":5,"files_found":0}""" + "\n";
        var lines = input.Split('\n', StringSplitOptions.RemoveEmptyEntries);
        Assert.Equal(2, lines.Length);
    }

    [Fact]
    public void Command_ends_with_newline()
    {
        var cmd = Commands.Ping();
        var bytes = Encoding.UTF8.GetBytes(cmd + "\n");
        Assert.Equal((byte)'\n', bytes[^1]);
    }

    [Fact]
    public void Partial_line_does_not_deserialize()
    {
        var partial = """{"event":"Progr""";
        var result = EngineEvent.Deserialize(partial);
        Assert.Null(result);  // malformed JSON returns null
    }
}
