using System.Text.Json;
using Xunit;

namespace Recoverer.Tests;

public class EngineEventTests
{
    [Fact]
    public void Pong_deserializes()
    {
        var e = EngineEvent.Deserialize("""{"event":"Pong"}""");
        Assert.IsType<PongEvent>(e);
    }

    [Fact]
    public void Progress_deserializes()
    {
        var e = EngineEvent.Deserialize(
            """{"event":"Progress","phase":"mft_scan","pct":23,"files_found":1247,"eta_secs":840}""");
        var p = Assert.IsType<ProgressEvent>(e);
        Assert.Equal("mft_scan", p.Phase);
        Assert.Equal(23, p.Pct);
        Assert.Equal(1247UL, p.FilesFound);
        Assert.Equal(840UL, p.EtaSecs);
    }

    [Fact]
    public void FileFound_deserializes()
    {
        var e = EngineEvent.Deserialize(
            """{"event":"FileFound","id":42,"filename":"photo.jpg","original_path":"C:\\Pictures","size_bytes":524288,"mime_type":"image/jpeg","category":"Images","confidence":87,"source":"mft"}""");
        var f = Assert.IsType<FileFoundEvent>(e);
        Assert.Equal(42L, f.Id);
        Assert.Equal("photo.jpg", f.Filename);
        Assert.Equal(87, f.Confidence);
    }

    [Fact]
    public void ScanComplete_deserializes()
    {
        var e = EngineEvent.Deserialize(
            """{"event":"ScanComplete","total_found":4218,"duration_secs":1247}""");
        var s = Assert.IsType<ScanCompleteEvent>(e);
        Assert.Equal(4218UL, s.TotalFound);
    }

    [Fact]
    public void Error_deserializes()
    {
        var e = EngineEvent.Deserialize(
            """{"event":"Error","code":"VOLUME_ACCESS_DENIED","message":"Access denied","fatal":false}""");
        var err = Assert.IsType<ErrorEvent>(e);
        Assert.Equal("VOLUME_ACCESS_DENIED", err.Code);
        Assert.False(err.Fatal);
    }

    [Fact]
    public void FilesPage_deserializes_file_records()
    {
        var e = EngineEvent.Deserialize(
            """{"event":"FilesPage","files":[{"id":1,"filename":"doc.pdf","original_path":null,"mime_type":"application/pdf","category":"Documents","size_bytes":102400,"confidence":90,"source":"mft","recovery_status":"pending","modified_at":1711574400}],"total_count":1}""");
        var fp = Assert.IsType<FilesPageEvent>(e);
        Assert.Single(fp.Files);
        Assert.Equal(90, fp.Files[0].Confidence);
        Assert.Equal(RecoveryStatus.Pending, fp.Files[0].RecoveryStatus);
    }

    [Fact]
    public void Unknown_event_returns_null()
    {
        var e = EngineEvent.Deserialize("""{"event":"SomeFutureEvent","data":"x"}""");
        Assert.Null(e);
    }
}
