use crate::scan::signatures::{SIGNATURES, mime_to_category};

#[derive(Debug, Clone)]
pub struct CarvingResult {
    pub byte_offset: u64,
    pub mime_type: String,
    pub category: String,
    pub estimated_size: Option<u64>,
}

/// Scan a byte buffer for known file signatures.
/// `base_offset` is the byte offset of the buffer start on the volume,
/// so returned `byte_offset` values are absolute volume positions.
pub fn carve_buffer(buf: &[u8], base_offset: u64) -> Vec<CarvingResult> {
    let mut results = Vec::new();

    for sig in SIGNATURES {
        let header = sig.header;
        let hoff = sig.header_offset;

        if header.len() + hoff > buf.len() {
            continue;
        }

        // Advance in 512-byte sector steps — all NTFS cluster allocations
        // are sector-aligned, so file headers never start mid-sector.
        // This gives ~512x fewer comparisons with zero loss of recall.
        const SECTOR: usize = 512;
        let mut pos = 0;
        while pos + hoff + header.len() <= buf.len() {
            if buf[pos + hoff..pos + hoff + header.len()] == *header {
                let estimated_size = sig.footer.and_then(|footer| {
                    find_footer(buf, pos, footer, sig.max_size)
                        .map(|end| (end - pos) as u64)
                });
                results.push(CarvingResult {
                    byte_offset: base_offset + pos as u64,
                    mime_type: sig.mime_type.to_string(),
                    category: mime_to_category(sig.mime_type).to_string(),
                    estimated_size,
                });
            }
            pos += SECTOR;
        }
    }

    // sort_by_key is a stable sort — equal-offset entries retain their SIGNATURES insertion order,
    // so dedup_by_key keeps the highest-priority (first in SIGNATURES) match at each offset.
    results.sort_by_key(|r| r.byte_offset);
    results.dedup_by_key(|r| r.byte_offset);
    results
}

fn find_footer(buf: &[u8], start: usize, footer: &[u8], max_size: u64) -> Option<usize> {
    if footer.is_empty() { return None; }
    let search_start = start + 1;
    let search_end = std::cmp::min(buf.len(), start.saturating_add(max_size as usize));
    if search_start + footer.len() > search_end { return None; }

    for i in search_start..=(search_end - footer.len()) {
        if &buf[i..i + footer.len()] == footer {
            return Some(i + footer.len());
        }
    }
    None
}
