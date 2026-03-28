use recoverer_engine::filetype::{detect_file_type, FileTypeResult};

fn jpeg_bytes() -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0];
    b.extend_from_slice(b"JFIF");
    b.extend(vec![0u8; 100]);
    b
}

fn png_bytes() -> Vec<u8> {
    vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0]
}

fn pdf_bytes() -> Vec<u8> {
    let mut b = b"%PDF-1.7\n".to_vec();
    b.extend(vec![0u8; 100]);
    b
}

fn zip_bytes() -> Vec<u8> {
    vec![0x50, 0x4B, 0x03, 0x04, 0, 0, 0, 0, 0, 0]
}

fn mp4_bytes() -> Vec<u8> {
    // ftyp box at offset 4
    let mut b = vec![0u8; 4]; // size
    b.extend_from_slice(b"ftyp");
    b.extend_from_slice(b"mp42");
    b.extend(vec![0u8; 50]);
    b
}

fn mp3_bytes() -> Vec<u8> {
    let mut b = b"ID3".to_vec();
    b.extend(vec![0u8; 100]);
    b
}

fn exe_bytes() -> Vec<u8> {
    let mut b = vec![0x4D, 0x5A]; // MZ
    b.extend(vec![0u8; 100]);
    b
}

fn unknown_bytes() -> Vec<u8> {
    vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05]
}

#[test]
fn detect_jpeg() {
    let r = detect_file_type(&jpeg_bytes());
    assert_eq!(r.mime_type, "image/jpeg");
    assert_eq!(r.category, "Images");
}

#[test]
fn detect_png() {
    let r = detect_file_type(&png_bytes());
    assert_eq!(r.mime_type, "image/png");
    assert_eq!(r.category, "Images");
}

#[test]
fn detect_pdf() {
    let r = detect_file_type(&pdf_bytes());
    assert_eq!(r.mime_type, "application/pdf");
    assert_eq!(r.category, "Documents");
}

#[test]
fn detect_zip() {
    let r = detect_file_type(&zip_bytes());
    assert_eq!(r.mime_type, "application/zip");
    assert_eq!(r.category, "Archives");
}

#[test]
fn detect_mp4() {
    let r = detect_file_type(&mp4_bytes());
    assert_eq!(r.mime_type, "video/mp4");
    assert_eq!(r.category, "Videos");
}

#[test]
fn detect_mp3() {
    let r = detect_file_type(&mp3_bytes());
    assert_eq!(r.mime_type, "audio/mpeg");
    assert_eq!(r.category, "Audio");
}

#[test]
fn detect_exe() {
    let r = detect_file_type(&exe_bytes());
    assert_eq!(r.category, "Other");
}

#[test]
fn detect_unknown_returns_other() {
    let r = detect_file_type(&unknown_bytes());
    assert_eq!(r.category, "Other");
    assert_eq!(r.mime_type, "application/octet-stream");
}

#[test]
fn empty_bytes_returns_other() {
    let r = detect_file_type(&[]);
    assert_eq!(r.category, "Other");
}
