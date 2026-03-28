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

fn unknown() -> FileTypeResult {
    FileTypeResult {
        mime_type: "application/octet-stream".to_string(),
        category: "Other".to_string(),
    }
}
