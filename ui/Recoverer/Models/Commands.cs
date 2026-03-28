using System.Text.Json;
using System.Text.Json.Serialization;

namespace Recoverer;

public enum ScanDepth { Quick, Deep }

public static class Commands
{
    private static readonly JsonSerializerOptions _opts = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.SnakeCaseLower) }
    };

    public static string Ping() => """{"type":"Ping"}""";
    public static string PauseScan() => """{"type":"PauseScan"}""";
    public static string ResumeScan() => """{"type":"ResumeScan"}""";
    public static string CancelScan() => """{"type":"CancelScan"}""";

    public static string StartScan(string drive, ScanDepth depth, IEnumerable<string> categories) =>
        JsonSerializer.Serialize(new StartScanPayload(drive, depth, categories.ToArray()), _opts);

    public static string QueryFiles(
        string? category, int? minConfidence, string? nameContains,
        ulong offset, ulong limit) =>
        JsonSerializer.Serialize(
            new QueryFilesPayload(category, minConfidence, nameContains, offset, limit), _opts);

    public static string RecoverFiles(
        IEnumerable<long> fileIds, string destination, bool recreateStructure) =>
        JsonSerializer.Serialize(
            new RecoverFilesPayload(fileIds.ToArray(), destination, recreateStructure), _opts);

    // ── Private payload records (PropertyNamingPolicy converts PascalCase → snake_case) ──

    private sealed record StartScanPayload(
        [property: JsonPropertyName("type")] string Type,
        string Drive,
        ScanDepth Depth,
        string[] Categories)
    {
        public StartScanPayload(string drive, ScanDepth depth, string[] categories)
            : this("StartScan", drive, depth, categories) { }
    }

    private sealed record QueryFilesPayload(
        [property: JsonPropertyName("type")] string Type,
        string? Category,
        int? MinConfidence,
        string? NameContains,
        ulong Offset,
        ulong Limit)
    {
        public QueryFilesPayload(string? category, int? minConfidence, string? nameContains,
            ulong offset, ulong limit)
            : this("QueryFiles", category, minConfidence, nameContains, offset, limit) { }
    }

    private sealed record RecoverFilesPayload(
        [property: JsonPropertyName("type")] string Type,
        long[] FileIds,
        string Destination,
        bool RecreateStructure)
    {
        public RecoverFilesPayload(long[] fileIds, string destination, bool recreateStructure)
            : this("RecoverFiles", fileIds, destination, recreateStructure) { }
    }
}
