use crate::scan::signatures::mime_to_category;

#[derive(Debug)]
pub struct FileTypeResult {
    pub mime_type: String,
    pub category: String,
}

/// Detect file type from raw bytes. Never trusts extensions.
pub fn detect_file_type(bytes: &[u8]) -> FileTypeResult {
    if bytes.is_empty() {
        return unknown();
    }

    // Primary: infer crate
    if let Some(t) = infer::get(bytes) {
        let mime = t.mime_type().to_string();

        // Disambiguate ZIP-based Office formats
        if mime == "application/zip" {
            if let Some(office_mime) = detect_office_format(bytes) {
                let category = mime_to_category(&office_mime).to_string();
                return FileTypeResult { mime_type: office_mime, category };
            }
        }

        let category = mime_to_category(&mime).to_string();
        return FileTypeResult { mime_type: mime, category };
    }

    // Fallback: MP4 detection (ftyp box at offset 4)
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        return FileTypeResult {
            mime_type: "video/mp4".to_string(),
            category: "Videos".to_string(),
        };
    }

    // Fallback: text detection
    if is_likely_text(bytes) {
        return FileTypeResult {
            mime_type: "text/plain".to_string(),
            category: "Documents".to_string(),
        };
    }

    unknown()
}

/// Detect DOCX / XLSX / PPTX from ZIP container
fn detect_office_format(bytes: &[u8]) -> Option<String> {
    let sample = &bytes[..bytes.len().min(2048)];
    let sample_str = String::from_utf8_lossy(sample);

    // Look for specific OOXML content type markers
    if sample_str.contains("wordprocessingml") || sample_str.contains("word/document") {
        return Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string());
    }
    if sample_str.contains("spreadsheetml") || sample_str.contains("xl/workbook") {
        return Some("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string());
    }
    if sample_str.contains("presentationml") || sample_str.contains("ppt/presentation") {
        return Some("application/vnd.openxmlformats-officedocument.presentationml.presentation".to_string());
    }
    None
}

fn is_likely_text(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(512)];
    let printable = sample.iter().filter(|&&b| b >= 0x20 || b == b'\n' || b == b'\r' || b == b'\t').count();
    printable * 100 / sample.len() >= 85
}

/// Fast extension-based detection for MFT scan phase (no disk I/O).
/// Returns confidence 75 (better than unknown=60, worse than bytes=87).
pub fn detect_file_type_from_name(filename: &str) -> FileTypeResult {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => ft("image/jpeg", "Images"),
        "png"          => ft("image/png", "Images"),
        "gif"          => ft("image/gif", "Images"),
        "bmp"          => ft("image/bmp", "Images"),
        "tiff" | "tif" => ft("image/tiff", "Images"),
        "webp"         => ft("image/webp", "Images"),
        "heic" | "heif"=> ft("image/heic", "Images"),
        "raw" | "cr2" | "nef" | "arw" | "dng" | "orf" | "rw2" => ft("image/x-raw", "Images"),
        "svg"          => ft("image/svg+xml", "Images"),

        "mp4" | "m4v"  => ft("video/mp4", "Videos"),
        "avi"          => ft("video/x-msvideo", "Videos"),
        "mov"          => ft("video/quicktime", "Videos"),
        "mkv"          => ft("video/x-matroska", "Videos"),
        "wmv"          => ft("video/x-ms-wmv", "Videos"),
        "flv"          => ft("video/x-flv", "Videos"),
        "webm"         => ft("video/webm", "Videos"),
        "3gp"          => ft("video/3gpp", "Videos"),
        "ts" | "mts"   => ft("video/mp2t", "Videos"),

        "mp3"          => ft("audio/mpeg", "Audio"),
        "wav"          => ft("audio/wav", "Audio"),
        "flac"         => ft("audio/flac", "Audio"),
        "aac"          => ft("audio/aac", "Audio"),
        "ogg"          => ft("audio/ogg", "Audio"),
        "wma"          => ft("audio/x-ms-wma", "Audio"),
        "m4a"          => ft("audio/mp4", "Audio"),
        "opus"         => ft("audio/opus", "Audio"),

        "pdf"          => ft("application/pdf", "Documents"),
        "doc"          => ft("application/msword", "Documents"),
        "docx"         => ft("application/vnd.openxmlformats-officedocument.wordprocessingml.document", "Documents"),
        "xls"          => ft("application/vnd.ms-excel", "Documents"),
        "xlsx"         => ft("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet", "Documents"),
        "ppt"          => ft("application/vnd.ms-powerpoint", "Documents"),
        "pptx"         => ft("application/vnd.openxmlformats-officedocument.presentationml.presentation", "Documents"),
        "txt" | "log"  => ft("text/plain", "Documents"),
        "html" | "htm" => ft("text/html", "Documents"),
        "csv"          => ft("text/csv", "Documents"),
        "rtf"          => ft("application/rtf", "Documents"),
        "odt" | "ods" | "odp" => ft("application/vnd.oasis.opendocument.text", "Documents"),

        "zip"          => ft("application/zip", "Archives"),
        "rar"          => ft("application/x-rar-compressed", "Archives"),
        "7z"           => ft("application/x-7z-compressed", "Archives"),
        "tar"          => ft("application/x-tar", "Archives"),
        "gz"           => ft("application/gzip", "Archives"),
        "bz2"          => ft("application/x-bzip2", "Archives"),
        "xz"           => ft("application/x-xz", "Archives"),

        _              => unknown(),
    }
}

fn ft(mime: &str, category: &str) -> FileTypeResult {
    FileTypeResult { mime_type: mime.to_string(), category: category.to_string() }
}

fn unknown() -> FileTypeResult {
    FileTypeResult {
        mime_type: "application/octet-stream".to_string(),
        category: "Other".to_string(),
    }
}
