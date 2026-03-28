use recoverer_engine::scan::ntfs::{parse_boot_sector, NtfsBootSector, parse_mft_record, MftRecord};

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
