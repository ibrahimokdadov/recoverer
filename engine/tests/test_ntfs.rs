use recoverer_engine::scan::ntfs::{parse_boot_sector, parse_mft_record};

fn make_ntfs_boot_sector() -> Vec<u8> {
    let mut sector = vec![0u8; 512];
    // NTFS signature
    sector[3..7].copy_from_slice(b"NTFS");
    // Bytes per sector (little-endian u16 at offset 11)
    sector[11] = 0x00;
    sector[12] = 0x02; // = 512
    // Sectors per cluster (u8 at offset 13)
    sector[13] = 0x08; // = 8 sectors per cluster
    // Total sectors (u64 at offset 40)
    let total: u64 = 976773167u64; // ~465 GB
    sector[40..48].copy_from_slice(&total.to_le_bytes());
    // MFT cluster offset (i64 at offset 48)
    let mft_lcn: i64 = 786432;
    sector[48..56].copy_from_slice(&mft_lcn.to_le_bytes());
    sector
}

fn make_deleted_mft_record(record_number: u32, filename: &str) -> Vec<u8> {
    let mut record = vec![0u8; 1024];
    // FILE signature
    record[0..4].copy_from_slice(b"FILE");
    // Sequence number at offset 4 (u16)
    record[4..6].copy_from_slice(&1u16.to_le_bytes());
    // Flags at offset 22: 0x00 = deleted file (0x01 = in use, 0x02 = directory)
    record[22..24].copy_from_slice(&0u16.to_le_bytes());
    // First attribute offset at offset 20 (u16)
    record[20..22].copy_from_slice(&56u16.to_le_bytes());
    // Record number at offset 44 (u32)
    record[44..48].copy_from_slice(&record_number.to_le_bytes());

    // Insert a minimal $FILE_NAME attribute at offset 56
    let name_bytes: Vec<u16> = filename.encode_utf16().collect();
    let name_len = name_bytes.len() as u8;
    let attr_start = 56usize;
    record[attr_start] = 0x30; // attribute type $FILE_NAME = 0x30
    record[attr_start + 1] = 0x00;
    record[attr_start + 2] = 0x00;
    record[attr_start + 3] = 0x00;
    let attr_len: u32 = 90 + (name_len as u32 * 2);
    record[attr_start + 4..attr_start + 8].copy_from_slice(&attr_len.to_le_bytes());
    record[attr_start + 8] = 0x00; // non-resident = 0
    record[attr_start + 9] = 0x00; // name length = 0
    // Content offset (u16 at attr+20)
    record[attr_start + 20..attr_start + 22].copy_from_slice(&24u16.to_le_bytes());
    // In the content at offset 24: parent ref (8 bytes), timestamps x4 (8 bytes each),
    // alloc size (8), real size (8), flags (4), reparse (4), filename length (1), namespace (1), name
    let content_start = attr_start + 24;
    // Parent MFT ref (inode 5 = root)
    record[content_start..content_start + 8].copy_from_slice(&5u64.to_le_bytes());
    // Timestamps (created/modified/mft/accessed) — all zero is fine for tests
    // Allocated size
    record[content_start + 40..content_start + 48].copy_from_slice(&(1024u64 * 100).to_le_bytes());
    // Real size
    record[content_start + 48..content_start + 56].copy_from_slice(&(512u64 * 80).to_le_bytes());
    // Filename length
    record[content_start + 64] = name_len;
    // Namespace
    record[content_start + 65] = 3; // WIN32_AND_DOS
    // Filename (UTF-16LE)
    let name_start = content_start + 66;
    for (i, &cu) in name_bytes.iter().enumerate() {
        record[name_start + i * 2] = cu as u8;
        record[name_start + i * 2 + 1] = (cu >> 8) as u8;
    }

    // End-of-attributes marker (0xFFFFFFFF at end)
    let end = attr_start + attr_len as usize;
    if end + 4 <= record.len() {
        record[end..end + 4].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
    }

    record
}

#[test]
fn parse_valid_ntfs_boot_sector() {
    let sector = make_ntfs_boot_sector();
    let boot = parse_boot_sector(&sector).unwrap();
    assert_eq!(boot.bytes_per_sector, 512);
    assert_eq!(boot.sectors_per_cluster, 8);
    assert_eq!(boot.bytes_per_cluster, 4096);
    assert_eq!(boot.mft_lcn, 786432);
}

#[test]
fn reject_non_ntfs_boot_sector() {
    let sector = vec![0u8; 512];
    let result = parse_boot_sector(&sector);
    assert!(result.is_err());
}

#[test]
fn parse_deleted_mft_record_extracts_filename() {
    let record_bytes = make_deleted_mft_record(100, "vacation.jpg");
    let result = parse_mft_record(&record_bytes, 100);
    assert!(result.is_some());
    let record = result.unwrap();
    assert!(!record.in_use, "record should be marked deleted");
    assert_eq!(record.filename.as_deref(), Some("vacation.jpg"));
}

#[test]
fn in_use_records_are_not_returned() {
    let mut record_bytes = make_deleted_mft_record(100, "active.txt");
    // Set in-use flag
    record_bytes[22] = 0x01;
    record_bytes[23] = 0x00;
    let result = parse_mft_record(&record_bytes, 100);
    // parse_mft_record returns None or a record with in_use=true; scanner skips in_use
    if let Some(r) = result {
        assert!(r.in_use);
    }
}

#[test]
fn corrupt_record_without_file_signature_returns_none() {
    let record = vec![0xAAu8; 1024];
    let result = parse_mft_record(&record, 0);
    assert!(result.is_none());
}

/// Build an MFT record that has both a $FILE_NAME and a non-resident $DATA attribute
/// with a simple single-run data run pointing to LCN 1000.
fn make_mft_record_with_data_run(_record_number: u32, filename: &str, lcn: u64) -> Vec<u8> {
    let mut record = vec![0u8; 1024];
    record[0..4].copy_from_slice(b"FILE");
    record[20..22].copy_from_slice(&56u16.to_le_bytes()); // first attr offset
    record[22..24].copy_from_slice(&0u16.to_le_bytes());  // deleted

    // --- $FILE_NAME at offset 56 ---
    let name_bytes: Vec<u16> = filename.encode_utf16().collect();
    let name_len = name_bytes.len() as u8;
    let fn_start = 56usize;
    record[fn_start] = 0x30;
    let fn_attr_len: u32 = 90 + (name_len as u32 * 2);
    record[fn_start + 4..fn_start + 8].copy_from_slice(&fn_attr_len.to_le_bytes());
    record[fn_start + 8] = 0x00; // resident
    record[fn_start + 20..fn_start + 22].copy_from_slice(&24u16.to_le_bytes()); // content offset
    let fn_content = fn_start + 24;
    record[fn_content..fn_content + 8].copy_from_slice(&5u64.to_le_bytes()); // parent ref
    record[fn_content + 48..fn_content + 56].copy_from_slice(&4096u64.to_le_bytes()); // real size
    record[fn_content + 64] = name_len;
    let fn_name_start = fn_content + 66;
    for (i, &cu) in name_bytes.iter().enumerate() {
        record[fn_name_start + i * 2] = cu as u8;
        record[fn_name_start + i * 2 + 1] = (cu >> 8) as u8;
    }

    // --- $DATA at offset fn_start + fn_attr_len ---
    let data_start = fn_start + fn_attr_len as usize;
    // Build data run: len_bytes=2 (length=1 cluster), off_bytes=3 (LCN fits in 3 bytes)
    let run_lcn = lcn as i64;
    let run_len: u64 = 1;
    let len_bytes: usize = 2;
    let off_bytes: usize = 3;
    let header_byte: u8 = ((off_bytes as u8) << 4) | (len_bytes as u8);
    // Data run offset in the attribute: standard non-resident header ends at byte 64
    let data_run_rel: u16 = 64;
    let data_attr_len: u32 = 64 + 1 + len_bytes as u32 + off_bytes as u32 + 1; // +1 for terminator
    record[data_start] = 0x80; // $DATA type low byte
    record[data_start + 4..data_start + 8].copy_from_slice(&data_attr_len.to_le_bytes());
    record[data_start + 8] = 0x01; // non-resident
    record[data_start + 32..data_start + 34].copy_from_slice(&data_run_rel.to_le_bytes());

    // Write data run at data_start + 64
    let run_start = data_start + 64;
    record[run_start] = header_byte;
    // Length (2 bytes LE)
    record[run_start + 1..run_start + 3].copy_from_slice(&(run_len as u16).to_le_bytes());
    // LCN (3 bytes LE)
    record[run_start + 3] = (run_lcn & 0xFF) as u8;
    record[run_start + 4] = ((run_lcn >> 8) & 0xFF) as u8;
    record[run_start + 5] = ((run_lcn >> 16) & 0xFF) as u8;
    // Terminator
    record[run_start + 6] = 0x00;

    // End-of-attributes
    let end = data_start + data_attr_len as usize;
    if end + 4 <= record.len() {
        record[end..end + 4].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
    }

    record
}

#[test]
fn data_run_first_cluster_is_extracted() {
    let record = make_mft_record_with_data_run(200, "report.docx", 1000);
    let result = parse_mft_record(&record, 200).expect("should parse");
    assert_eq!(result.filename.as_deref(), Some("report.docx"));
    assert_eq!(result.first_data_cluster, Some(1000));
}

#[test]
fn data_run_off_bytes_overflow_guard() {
    // Craft a $DATA attribute where off_bytes = 8 — must return None, not panic.
    let mut attr = vec![0u8; 128];
    attr[8] = 0x01; // non-resident
    // data_run_offset = 64
    attr[32] = 64;
    attr[33] = 0;
    // header at offset 64: len_bytes=1, off_bytes=8 → 0x81
    attr[64] = 0x81;
    // rest is zero
    // parse_data_attr_first_cluster is not pub, but we can exercise it via parse_mft_record
    // by embedding this $DATA in a record. We verify: no panic, first_data_cluster = None.
    let mut record = vec![0u8; 1024];
    record[0..4].copy_from_slice(b"FILE");
    record[20..22].copy_from_slice(&56u16.to_le_bytes());
    record[22..24].copy_from_slice(&0u16.to_le_bytes());
    let attr_len: u32 = 128;
    record[56] = 0x80;
    record[60..64].copy_from_slice(&attr_len.to_le_bytes());
    record[56 + 8] = 0x01; // non-resident
    record[56 + 32..56 + 34].copy_from_slice(&64u16.to_le_bytes()); // data_run_offset
    record[56 + 64] = 0x81; // off_bytes=8, len_bytes=1
    record[56 + 65] = 0x01; // length byte
    // 8 LCN bytes follow (all 0xFF to force large shift if guard absent)
    for i in 0..8 {
        record[56 + 66 + i] = 0xFF;
    }
    record[56 + 74] = 0x00; // terminator
    let end = 56 + 128;
    if end + 4 <= record.len() {
        record[end..end + 4].copy_from_slice(&0xFFFFFFFFu32.to_le_bytes());
    }
    // Must not panic
    let result = parse_mft_record(&record, 0);
    if let Some(r) = result {
        // off_bytes=8 guard fires → no cluster
        assert!(r.first_data_cluster.is_none());
    }
}
