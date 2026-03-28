// engine/tests/test_signatures.rs
use recoverer_engine::scan::signatures::{SIGNATURES, mime_to_category};

#[test]
fn all_signature_mimes_have_known_category() {
    for sig in SIGNATURES {
        let cat = mime_to_category(sig.mime_type);
        assert_ne!(cat, "Other",
            "Signature for {} has category 'Other' via mime_to_category — add it to the match",
            sig.mime_type);
    }
}

#[test]
fn no_duplicate_mime_at_same_offset_and_header() {
    for i in 0..SIGNATURES.len() {
        for j in (i + 1)..SIGNATURES.len() {
            let a = &SIGNATURES[i];
            let b = &SIGNATURES[j];
            if a.header == b.header && a.header_offset == b.header_offset {
                panic!(
                    "Duplicate signature: '{}' and '{}' share header {:?} at offset {}",
                    a.mime_type, b.mime_type, a.header, a.header_offset
                );
            }
        }
    }
}

#[test]
fn cr2_entry_appears_before_tiff_le() {
    let cr2_pos = SIGNATURES.iter().position(|s| s.mime_type == "image/x-canon-cr2").unwrap();
    let tiff_le_pos = SIGNATURES.iter().position(|s|
        s.mime_type == "image/tiff" && s.header == &[0x49u8, 0x49, 0x2A, 0x00]
    ).unwrap();
    assert!(cr2_pos < tiff_le_pos,
        "CR2 (pos {}) must come before TIFF LE (pos {}) — CR2 has the more specific header",
        cr2_pos, tiff_le_pos);
}

#[test]
fn mime_to_category_covers_all_standard_types() {
    assert_eq!(mime_to_category("image/jpeg"), "Images");
    assert_eq!(mime_to_category("video/mp4"), "Videos");
    assert_eq!(mime_to_category("audio/mpeg"), "Audio");
    assert_eq!(mime_to_category("application/pdf"), "Documents");
    assert_eq!(mime_to_category("application/zip"), "Archives");
    assert_eq!(mime_to_category("application/x-unknown"), "Other");
}
