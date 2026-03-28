use recoverer_engine::scan::carver::{carve_buffer, CarvingResult};

fn buffer_with_jpeg_at(offset: usize) -> Vec<u8> {
    let mut buf = vec![0xAAu8; 4096];
    // JPEG header
    buf[offset] = 0xFF;
    buf[offset + 1] = 0xD8;
    buf[offset + 2] = 0xFF;
    buf[offset + 3] = 0xE0;
    // JPEG footer at offset + 100
    if offset + 102 <= buf.len() {
        buf[offset + 100] = 0xFF;
        buf[offset + 101] = 0xD9;
    }
    buf
}

fn buffer_with_pdf_at(offset: usize) -> Vec<u8> {
    let mut buf = vec![0x00u8; 4096];
    buf[offset..offset + 4].copy_from_slice(b"%PDF");
    buf
}

fn buffer_with_multiple_signatures() -> Vec<u8> {
    let mut buf = vec![0x00u8; 8192];
    // JPEG at 512
    buf[512] = 0xFF; buf[513] = 0xD8; buf[514] = 0xFF;
    // PNG at 2048
    buf[2048..2056].copy_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    buf
}

#[test]
fn find_jpeg_at_buffer_start() {
    let buf = buffer_with_jpeg_at(0);
    let results = carve_buffer(&buf, 0);
    assert!(!results.is_empty());
    let jpeg = results.iter().find(|r| r.mime_type == "image/jpeg");
    assert!(jpeg.is_some());
    assert_eq!(jpeg.unwrap().byte_offset, 0);
}

#[test]
fn find_jpeg_at_midpoint() {
    let buf = buffer_with_jpeg_at(512);
    let results = carve_buffer(&buf, 0);
    let jpeg = results.iter().find(|r| r.mime_type == "image/jpeg");
    assert!(jpeg.is_some());
    assert_eq!(jpeg.unwrap().byte_offset, 512);
}

#[test]
fn find_pdf_signature() {
    let buf = buffer_with_pdf_at(0);
    let results = carve_buffer(&buf, 0);
    let pdf = results.iter().find(|r| r.mime_type == "application/pdf");
    assert!(pdf.is_some());
}

#[test]
fn find_multiple_different_signatures() {
    let buf = buffer_with_multiple_signatures();
    let results = carve_buffer(&buf, 0);
    let mimes: Vec<&str> = results.iter().map(|r| r.mime_type.as_str()).collect();
    assert!(mimes.contains(&"image/jpeg"));
    assert!(mimes.contains(&"image/png"));
}

#[test]
fn empty_buffer_returns_no_results() {
    let results = carve_buffer(&[], 0);
    assert!(results.is_empty());
}

#[test]
fn buffer_with_no_signatures_returns_no_results() {
    let buf = vec![0x00u8; 4096];
    let results = carve_buffer(&buf, 0);
    assert!(results.is_empty());
}

#[test]
fn byte_offset_includes_buffer_base_offset() {
    let buf = buffer_with_jpeg_at(0);
    let base_offset: u64 = 1_000_000;
    let results = carve_buffer(&buf, base_offset);
    let jpeg = results.iter().find(|r| r.mime_type == "image/jpeg").unwrap();
    assert_eq!(jpeg.byte_offset, base_offset);
}

fn buffer_with_webp_at(offset: usize) -> Vec<u8> {
    let mut buf = vec![0x00u8; 4096];
    // RIFF at offset (4 bytes)
    buf[offset..offset + 4].copy_from_slice(b"RIFF");
    // file size (4 bytes) — doesn't matter for carving
    buf[offset + 4..offset + 8].copy_from_slice(&[0x00, 0x10, 0x00, 0x00]);
    // WEBP at offset + 8
    buf[offset + 8..offset + 12].copy_from_slice(b"WEBP");
    buf
}

#[test]
fn find_webp_with_nonzero_header_offset() {
    let buf = buffer_with_webp_at(0);
    let results = carve_buffer(&buf, 0);
    let webp = results.iter().find(|r| r.mime_type == "image/webp");
    assert!(webp.is_some(), "WEBP at header_offset=8 should be found");
    assert_eq!(webp.unwrap().byte_offset, 0);
}
