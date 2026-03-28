// engine/src/recovery.rs
use std::path::{Path, PathBuf};
use crate::error::Result;

#[derive(Debug, Clone)]
pub enum ConflictMode {
    AddSuffix,
    Skip,
    Overwrite,
}

#[derive(Debug, Clone)]
pub struct RecoveryOptions {
    pub destination: String,
    pub recreate_structure: bool,
    pub on_conflict: ConflictMode,
}

/// Returns true if source drive letter == destination drive letter (Windows).
/// On non-Windows, compares first character as a best-effort check.
pub fn is_same_volume(source_drive: &str, destination: &str) -> bool {
    let src_letter = source_drive.chars().next().map(|c| c.to_ascii_uppercase());
    let dst_letter = destination.chars().next().map(|c| c.to_ascii_uppercase());
    match (src_letter, dst_letter) {
        (Some(s), Some(d)) => s == d,
        _ => false,
    }
}

/// Build the destination file path for a recovered file.
/// Generates a name for carved files (no original filename).
pub fn build_destination_path(
    opts: &RecoveryOptions,
    filename: Option<&str>,
    original_path: Option<&str>,
    mime_type: &str,
    file_id: i64,
) -> PathBuf {
    let dest_base = Path::new(&opts.destination);

    let extension = mime_to_extension(mime_type);
    let base_name = match filename {
        Some(f) => f.to_string(),
        None => format!("recovered_{}{}", file_id, extension),
    };

    if opts.recreate_structure {
        if let Some(orig_path) = original_path {
            let stripped = strip_drive_root(orig_path);
            let full = dest_base.join(stripped).join(&base_name);
            return full;
        }
    }

    dest_base.join(&base_name)
}

/// Resolve naming conflicts by adding a numeric suffix.
pub fn resolve_conflict(path: &Path, mode: &ConflictMode) -> Option<PathBuf> {
    if !path.exists() {
        return Some(path.to_path_buf());
    }
    match mode {
        ConflictMode::Skip => None,
        ConflictMode::Overwrite => Some(path.to_path_buf()),
        ConflictMode::AddSuffix => {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            let parent = path.parent().unwrap_or(Path::new("."));

            for i in 1..=999 {
                let candidate = parent.join(format!("{}_{}{}", stem, i, ext));
                if !candidate.exists() {
                    return Some(candidate);
                }
            }
            None
        }
    }
}

/// Copy a file from `src` to `dst`, creating parent directories as needed.
pub fn copy_file_content(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(src, dst)?;
    Ok(())
}

fn mime_to_extension(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => ".jpg",
        "image/png" => ".png",
        "image/gif" => ".gif",
        "image/bmp" => ".bmp",
        "image/tiff" => ".tif",
        "image/webp" => ".webp",
        "video/mp4" => ".mp4",
        "video/quicktime" => ".mov",
        "video/x-msvideo" => ".avi",
        "video/x-matroska" => ".mkv",
        "audio/mpeg" => ".mp3",
        "audio/flac" => ".flac",
        "audio/wav" => ".wav",
        "audio/ogg" => ".ogg",
        "application/pdf" => ".pdf",
        "application/zip" => ".zip",
        "application/x-rar-compressed" => ".rar",
        "application/x-7z-compressed" => ".7z",
        "application/msword" => ".doc",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => ".docx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => ".xlsx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => ".pptx",
        _ => ".bin",
    }
}

fn strip_drive_root(path: &str) -> &str {
    // "C:\Users\test\Pictures" -> "Users\test\Pictures"
    let p = path.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    let p = p.trim_start_matches(':');
    let p = p.trim_start_matches(['\\', '/']);
    p
}
