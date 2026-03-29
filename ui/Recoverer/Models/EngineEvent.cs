using System.Text.Json;
using System.Text.Json.Serialization;

namespace Recoverer;

// ── Discriminated event union ──────────────────────────────────────────────

public abstract class EngineEvent
{
    private static readonly JsonSerializerOptions _opts = new()
    {
        PropertyNameCaseInsensitive = true,
        NumberHandling = JsonNumberHandling.AllowReadingFromString,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.SnakeCaseLower) }
    };

    public static EngineEvent? Deserialize(string line)
    {
        try
        {
            using var doc = JsonDocument.Parse(line);
            var root = doc.RootElement;
            if (!root.TryGetProperty("event", out var tagProp)) return null;
            var raw = root.GetRawText();
            return tagProp.GetString() switch
            {
                "Pong"             => new PongEvent(),
                "Progress"         => JsonSerializer.Deserialize<ProgressEvent>(raw, _opts),
                "PhaseChange"      => JsonSerializer.Deserialize<PhaseChangeEvent>(raw, _opts),
                "FileFound"        => JsonSerializer.Deserialize<FileFoundEvent>(raw, _opts),
                "ScanComplete"     => JsonSerializer.Deserialize<ScanCompleteEvent>(raw, _opts),
                "RecoveryProgress" => JsonSerializer.Deserialize<RecoveryProgressEvent>(raw, _opts),
                "RecoveryComplete" => JsonSerializer.Deserialize<RecoveryCompleteEvent>(raw, _opts),
                "Error"            => JsonSerializer.Deserialize<ErrorEvent>(raw, _opts),
                "FilesPage"        => JsonSerializer.Deserialize<FilesPageEvent>(raw, _opts),
                "SessionsList"     => JsonSerializer.Deserialize<SessionsListEvent>(raw, _opts),
                _                  => null
            };
        }
        catch { return null; }
    }
}

// ── Concrete event types ───────────────────────────────────────────────────

public sealed class PongEvent : EngineEvent { }

public sealed class ProgressEvent : EngineEvent
{
    [JsonPropertyName("phase")]       public string Phase       { get; init; } = "";
    [JsonPropertyName("pct")]         public byte   Pct         { get; init; }
    [JsonPropertyName("files_found")] public ulong  FilesFound  { get; init; }
    [JsonPropertyName("eta_secs")]    public ulong? EtaSecs     { get; init; }
}

public sealed class PhaseChangeEvent : EngineEvent
{
    [JsonPropertyName("new_phase")] public string NewPhase { get; init; } = "";
}

/// <summary>
/// Emitted during scan when a file is discovered. Does not include recovery_status
/// (which is only meaningful after recovery). Use <see cref="FileRecord"/> for
/// post-scan file state from a FilesPage event.
/// </summary>
public sealed class FileFoundEvent : EngineEvent
{
    [JsonPropertyName("id")]            public long    Id           { get; init; }
    [JsonPropertyName("filename")]      public string? Filename     { get; init; }
    [JsonPropertyName("original_path")] public string? OriginalPath { get; init; }
    [JsonPropertyName("size_bytes")]    public ulong   SizeBytes    { get; init; }
    [JsonPropertyName("mime_type")]     public string  MimeType     { get; init; } = "";
    [JsonPropertyName("category")]      public string  Category     { get; init; } = "";
    [JsonPropertyName("confidence")]    public byte    Confidence   { get; init; }
    [JsonPropertyName("source")]        public string  Source       { get; init; } = "";
}

public sealed class ScanCompleteEvent : EngineEvent
{
    [JsonPropertyName("total_found")]   public ulong TotalFound   { get; init; }
    [JsonPropertyName("duration_secs")] public ulong DurationSecs { get; init; }
}

public sealed class RecoveryProgressEvent : EngineEvent
{
    [JsonPropertyName("recovered")] public ulong Recovered { get; init; }
    [JsonPropertyName("warnings")]  public ulong Warnings  { get; init; }
    [JsonPropertyName("failed")]    public ulong Failed    { get; init; }
    [JsonPropertyName("total")]     public ulong Total     { get; init; }
}

public sealed class RecoveryCompleteEvent : EngineEvent
{
    [JsonPropertyName("recovered")] public ulong Recovered { get; init; }
    [JsonPropertyName("warnings")]  public ulong Warnings  { get; init; }
    [JsonPropertyName("failed")]    public ulong Failed    { get; init; }
}

public sealed class ErrorEvent : EngineEvent
{
    [JsonPropertyName("code")]    public string Code    { get; init; } = "";
    [JsonPropertyName("message")] public string Message { get; init; } = "";
    [JsonPropertyName("fatal")]   public bool   Fatal   { get; init; }
}

public sealed class FilesPageEvent : EngineEvent
{
    [JsonPropertyName("files")]       public FileRecord[] Files      { get; init; } = [];
    [JsonPropertyName("total_count")] public long         TotalCount { get; init; }
}

public sealed class SessionsListEvent : EngineEvent
{
    [JsonPropertyName("sessions")] public ScanSession[] Sessions { get; init; } = [];
}

public sealed class ScanSession
{
    [JsonPropertyName("id")]          public long   Id         { get; init; }
    [JsonPropertyName("name")]        public string Name       { get; init; } = "";
    [JsonPropertyName("drive")]       public string Drive      { get; init; } = "";
    [JsonPropertyName("db_path")]     public string DbPath     { get; init; } = "";
    [JsonPropertyName("created_at")]  public long   CreatedAt  { get; init; }
    [JsonPropertyName("total_files")] public long   TotalFiles { get; init; }

    public string DisplayDate =>
        DateTimeOffset.FromUnixTimeSeconds(CreatedAt).LocalDateTime.ToString("MMM d, yyyy  HH:mm");

    public string DisplayName =>
        $"{Drive} drive  ·  {TotalFiles:N0} files  ·  {DisplayDate}";
}

// ── FileRecord ─────────────────────────────────────────────────────────────

public enum RecoveryStatus { Pending, Recovered, Failed, Skipped }

public sealed class FileRecord
{
    [JsonPropertyName("id")]              public long           Id             { get; init; }
    [JsonPropertyName("filename")]        public string?        Filename       { get; init; }
    [JsonPropertyName("original_path")]   public string?        OriginalPath   { get; init; }
    [JsonPropertyName("mime_type")]       public string         MimeType       { get; init; } = "";
    [JsonPropertyName("category")]        public string         Category       { get; init; } = "";
    [JsonPropertyName("size_bytes")]      public ulong          SizeBytes      { get; init; }
    [JsonPropertyName("confidence")]      public byte           Confidence     { get; init; }
    [JsonPropertyName("source")]          public string         Source         { get; init; } = "";
    [JsonPropertyName("recovery_status")]   public RecoveryStatus RecoveryStatus  { get; init; }
    [JsonPropertyName("modified_at")]       public long?          ModifiedAt      { get; init; }
    [JsonPropertyName("fragment_group_id")] public long           FragmentGroupId { get; init; }

    public string DisplayName    => Filename ?? $"[carved file #{Id}]";
    public bool   IsFragment     => FragmentGroupId > 0;
    public string StatusDisplay  => RecoveryStatus == RecoveryStatus.Recovered
        ? "✓ Recovered"
        : IsFragment ? $"chain #{FragmentGroupId}" : Source;
}
