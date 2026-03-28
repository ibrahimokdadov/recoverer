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

        let mut pos = 0;
        while pos + hoff + header.len() <= buf.len() {
            if buf[pos + hoff..pos + hoff + header.len()] == *header {
                let estimated_size = sig.footer.and_then(|footer| {
                    find_footer(buf, pos, footer)
                        .map(|end| (end - pos) as u64)
                });

                results.push(CarvingResult {
                    byte_offset: base_offset + pos as u64,
                    mime_type: sig.mime_type.to_string(),
                    category: mime_to_category(sig.mime_type).to_string(),
                    estimated_size,
                });

                // Advance past this match to avoid re-matching the same position
                pos += header.len() + hoff;
            } else {
                pos += 1;
            }
        }
    }

    // Sort by offset, deduplicate same-offset matches (keep highest-priority signature)
    results.sort_by_key(|r| r.byte_offset);
    results.dedup_by_key(|r| r.byte_offset);
    results
}

fn find_footer(buf: &[u8], start: usize, footer: &[u8]) -> Option<usize> {
    if footer.is_empty() { return None; }
    let search_start = start + 1;
    if search_start + footer.len() > buf.len() { return None; }

    for i in search_start..=(buf.len() - footer.len()) {
        if &buf[i..i + footer.len()] == footer {
            return Some(i + footer.len());
        }
    }
    None
}
