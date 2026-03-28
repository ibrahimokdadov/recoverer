// engine/src/scan/fat.rs
/// FAT32/exFAT directory entry scanner for deleted files.
/// Deleted entries have 0xE5 as the first byte of the filename.
use crate::error::Result;
use crate::scan::volume::VolumeReader;

pub struct FatDeletedEntry {
    pub original_name: Option<String>,
    pub first_cluster: u32,
    pub file_size: u32,
}

/// Check if the volume is FAT32 or exFAT (not NTFS).
pub fn is_fat_volume(reader: &VolumeReader) -> bool {
    // Read boot sector
    let Ok(sector) = reader.read_sector(0) else { return false; };
    if sector.len() < 82 { return false; }
    // FAT32 has "FAT32   " at offset 82, exFAT has "EXFAT   " at offset 3
    &sector[3..8] == b"EXFAT" || &sector[82..87] == b"FAT32"
}

/// Scan FAT32/exFAT directory entries for deleted files (first byte = 0xE5).
/// Stub for v1 — returns empty.
pub fn scan_fat_deleted(_reader: &VolumeReader) -> Result<Vec<FatDeletedEntry>> {
    // TODO: parse FAT BPB, locate root directory clusters, walk directory entries
    Ok(vec![])
}
