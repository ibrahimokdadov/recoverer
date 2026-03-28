/// Represents a file signature for carving
#[derive(Debug, Clone)]
pub struct Signature {
    pub mime_type: &'static str,
    pub header: &'static [u8],
    pub header_offset: usize,
    /// If Some, scanning can look for this footer to determine file end
    pub footer: Option<&'static [u8]>,
    /// Maximum expected file size in bytes (for carving bounds)
    pub max_size: u64,
}

/// The master signature database.
pub static SIGNATURES: &[Signature] = &[
    // Images
    Signature { mime_type: "image/jpeg",
        header: &[0xFF, 0xD8, 0xFF], header_offset: 0,
        footer: Some(&[0xFF, 0xD9]), max_size: 50 * 1024 * 1024 },
    Signature { mime_type: "image/png",
        header: &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A], header_offset: 0,
        footer: Some(&[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82]), max_size: 100 * 1024 * 1024 },
    Signature { mime_type: "image/gif",
        header: b"GIF89a", header_offset: 0,
        footer: Some(&[0x00, 0x3B]), max_size: 50 * 1024 * 1024 },
    Signature { mime_type: "image/gif",
        header: b"GIF87a", header_offset: 0,
        footer: Some(&[0x00, 0x3B]), max_size: 50 * 1024 * 1024 },
    Signature { mime_type: "image/webp",
        header: b"WEBP", header_offset: 8,
        footer: None, max_size: 50 * 1024 * 1024 },
    Signature { mime_type: "image/bmp",
        header: &[0x42, 0x4D], header_offset: 0,
        footer: None, max_size: 50 * 1024 * 1024 },
    // RAW formats — CR2 must come before TIFF LE (same first 4 bytes, CR2 has more specific 10-byte header)
    Signature { mime_type: "image/x-canon-cr2",
        header: &[0x49, 0x49, 0x2A, 0x00, 0x10, 0x00, 0x00, 0x00, 0x43, 0x52], header_offset: 0,
        footer: None, max_size: 50 * 1024 * 1024 },
    Signature { mime_type: "image/tiff",
        header: &[0x49, 0x49, 0x2A, 0x00], header_offset: 0,
        footer: None, max_size: 200 * 1024 * 1024 },
    Signature { mime_type: "image/tiff",
        header: &[0x4D, 0x4D, 0x00, 0x2A], header_offset: 0,
        footer: None, max_size: 200 * 1024 * 1024 },
    // NOTE: NEF (Nikon) uses the same big-endian TIFF header [0x4D, 0x4D, 0x00, 0x2A] as TIFF BE;
    // no further discrimination is possible at the signature level, so NEF is omitted here.
    // Videos
    Signature { mime_type: "video/mp4",
        header: b"ftyp", header_offset: 4,
        footer: None, max_size: 10 * 1024 * 1024 * 1024 },
    // WAV: match on "WAVE" marker at bytes 8-11 (before AVI/RIFF entry so dedup picks WAV first)
    Signature { mime_type: "audio/wav",
        header: b"WAVE", header_offset: 8,
        footer: None, max_size: 2 * 1024 * 1024 * 1024 },
    Signature { mime_type: "video/x-msvideo",
        header: b"RIFF", header_offset: 0,
        footer: None, max_size: 5 * 1024 * 1024 * 1024 },
    Signature { mime_type: "video/x-matroska",
        header: &[0x1A, 0x45, 0xDF, 0xA3], header_offset: 0,
        footer: None, max_size: 50 * 1024 * 1024 * 1024 },
    Signature { mime_type: "video/x-ms-wmv",
        header: &[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11], header_offset: 0,
        footer: None, max_size: 10 * 1024 * 1024 * 1024 },
    // Audio
    Signature { mime_type: "audio/mpeg",
        header: b"ID3", header_offset: 0,
        footer: None, max_size: 500 * 1024 * 1024 },
    Signature { mime_type: "audio/mpeg",
        header: &[0xFF, 0xFB], header_offset: 0,
        footer: None, max_size: 500 * 1024 * 1024 },
    Signature { mime_type: "audio/flac",
        header: b"fLaC", header_offset: 0,
        footer: None, max_size: 2 * 1024 * 1024 * 1024 },
    Signature { mime_type: "audio/ogg",
        header: b"OggS", header_offset: 0,
        footer: None, max_size: 500 * 1024 * 1024 },
    // Documents
    Signature { mime_type: "application/pdf",
        header: b"%PDF", header_offset: 0,
        footer: Some(b"%%EOF"), max_size: 500 * 1024 * 1024 },
    Signature { mime_type: "application/msword",
        header: &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1], header_offset: 0,
        footer: None, max_size: 100 * 1024 * 1024 },
    // Office 2007+ (DOCX/XLSX/PPTX are all ZIP)
    Signature { mime_type: "application/zip",
        header: &[0x50, 0x4B, 0x03, 0x04], header_offset: 0,
        footer: Some(&[0x50, 0x4B, 0x05, 0x06]), max_size: 5 * 1024 * 1024 * 1024 },
    // Archives
    Signature { mime_type: "application/x-rar-compressed",
        header: &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07], header_offset: 0,
        footer: None, max_size: 5 * 1024 * 1024 * 1024 },
    Signature { mime_type: "application/x-7z-compressed",
        header: &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C], header_offset: 0,
        footer: None, max_size: 5 * 1024 * 1024 * 1024 },
    Signature { mime_type: "application/gzip",
        header: &[0x1F, 0x8B, 0x08], header_offset: 0,  // 0x08 = DEFLATE compression method (always present)
        footer: None, max_size: 5 * 1024 * 1024 * 1024 },
];

/// Map MIME type to user-facing category
pub fn mime_to_category(mime: &str) -> &'static str {
    match mime {
        m if m.starts_with("image/") => "Images",
        m if m.starts_with("video/") => "Videos",
        m if m.starts_with("audio/") => "Audio",
        "application/pdf" | "application/msword"
        | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "application/vnd.ms-excel"
        | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        | "application/vnd.ms-powerpoint"
        | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        | "text/plain" | "text/html" | "text/csv" => "Documents",
        "application/zip" | "application/x-rar-compressed"
        | "application/x-7z-compressed" | "application/gzip"
        | "application/x-tar" => "Archives",
        _ => "Other",
    }
}
