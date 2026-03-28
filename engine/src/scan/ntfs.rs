// engine/src/scan/ntfs.rs
use crate::error::{EngineError, Result};

#[derive(Debug)]
pub struct NtfsBootSector {
    pub bytes_per_sector: u32,
    pub sectors_per_cluster: u32,
    pub bytes_per_cluster: u32,
    pub mft_lcn: i64,
    pub total_sectors: u64,
}

#[derive(Debug, Clone)]
pub struct MftRecord {
    pub record_number: u32,
    pub in_use: bool,
    pub is_directory: bool,
    pub filename: Option<String>,
    pub parent_ref: Option<u64>,
    pub file_size: u64,
    pub allocated_size: u64,
    pub created_at: Option<i64>,
    pub modified_at: Option<i64>,
    pub first_data_cluster: Option<u64>,
}

pub fn parse_boot_sector(sector: &[u8]) -> Result<NtfsBootSector> {
    if sector.len() < 512 {
        return Err(EngineError::NotNtfs);
    }
    if &sector[3..7] != b"NTFS" {
        return Err(EngineError::NotNtfs);
    }

    let bytes_per_sector = u16::from_le_bytes([sector[11], sector[12]]) as u32;
    let sectors_per_cluster = sector[13] as u32;
    let bytes_per_cluster = bytes_per_sector * sectors_per_cluster;
    let total_sectors = u64::from_le_bytes(sector[40..48].try_into().unwrap());
    let mft_lcn = i64::from_le_bytes(sector[48..56].try_into().unwrap());

    Ok(NtfsBootSector {
        bytes_per_sector,
        sectors_per_cluster,
        bytes_per_cluster,
        mft_lcn,
        total_sectors,
    })
}

/// Parse one MFT record (1024 bytes). Returns None if not a valid FILE record.
/// Errors in individual attributes are silently skipped (defensive parsing).
pub fn parse_mft_record(data: &[u8], record_number: u32) -> Option<MftRecord> {
    if data.len() < 42 {
        return None;
    }
    // FILE signature
    if &data[0..4] != b"FILE" {
        return None;
    }

    let flags = u16::from_le_bytes([data[22], data[23]]);
    let in_use = flags & 0x01 != 0;
    let is_directory = flags & 0x02 != 0;

    let first_attr_offset = u16::from_le_bytes([data[20], data[21]]) as usize;
    if first_attr_offset >= data.len() {
        return Some(MftRecord {
            record_number, in_use, is_directory,
            filename: None, parent_ref: None,
            file_size: 0, allocated_size: 0,
            created_at: None, modified_at: None,
            first_data_cluster: None,
        });
    }

    let mut filename = None;
    let mut parent_ref = None;
    let mut file_size: u64 = 0;
    let mut allocated_size: u64 = 0;
    let mut created_at = None;
    let mut modified_at = None;
    let mut first_data_cluster = None;

    let mut offset = first_attr_offset;
    let max_offset = data.len().saturating_sub(4);

    while offset < max_offset {
        let attr_type = u32::from_le_bytes(
            match data[offset..offset + 4].try_into() {
                Ok(b) => b,
                Err(_) => break,
            }
        );

        if attr_type == 0xFFFFFFFF {
            break;
        }

        if offset + 8 > data.len() { break; }
        let attr_len = u32::from_le_bytes(
            data[offset + 4..offset + 8].try_into().unwrap_or([0; 4])
        ) as usize;

        if attr_len == 0 || offset + attr_len > data.len() {
            break;
        }

        match attr_type {
            0x30 => {
                // $FILE_NAME attribute — pass full remaining slice so content is not truncated
                // by a potentially under-counted attr_len in the record header.
                if let Some((fname, pref, fs, als, cr, mo)) = parse_filename_attr(&data[offset..]) {
                    if filename.is_none() {
                        filename = Some(fname);
                        parent_ref = Some(pref);
                        file_size = fs;
                        allocated_size = als;
                        created_at = cr;
                        modified_at = mo;
                    }
                }
            }
            0x80 => {
                // $DATA attribute — try to get first VCN/LCN (first cluster)
                if let Some(cluster) = parse_data_attr_first_cluster(&data[offset..offset + attr_len]) {
                    first_data_cluster = Some(cluster);
                }
            }
            _ => {}
        }

        offset += attr_len;
    }

    Some(MftRecord {
        record_number, in_use, is_directory,
        filename, parent_ref, file_size, allocated_size,
        created_at, modified_at, first_data_cluster,
    })
}

fn parse_filename_attr(attr: &[u8]) -> Option<(String, u64, u64, u64, Option<i64>, Option<i64>)> {
    if attr.len() < 26 { return None; }

    let non_resident = attr[8];
    let content_offset = u16::from_le_bytes([attr[20], attr[21]]) as usize;

    let content = if non_resident == 0 {
        &attr[content_offset..]
    } else {
        return None; // $FILE_NAME is always resident — if not, skip
    };

    if content.len() < 66 { return None; }

    let parent_ref = u64::from_le_bytes(content[0..8].try_into().ok()?) & 0x0000FFFFFFFFFFFF;
    let created_raw = i64::from_le_bytes(content[8..16].try_into().unwrap_or([0; 8]));
    let modified_raw = i64::from_le_bytes(content[16..24].try_into().unwrap_or([0; 8]));
    let allocated_size = u64::from_le_bytes(content[40..48].try_into().unwrap_or([0; 8]));
    let real_size = u64::from_le_bytes(content[48..56].try_into().unwrap_or([0; 8]));
    let name_len = content[64] as usize;

    if content.len() < 66 + name_len * 2 { return None; }

    let name_u16: Vec<u16> = (0..name_len)
        .map(|i| u16::from_le_bytes([content[66 + i * 2], content[66 + i * 2 + 1]]))
        .collect();

    let filename = String::from_utf16_lossy(&name_u16).to_string();

    // Convert Windows FILETIME (100ns intervals since 1601-01-01) to Unix timestamp
    let created = if created_raw != 0 {
        Some((created_raw / 10_000_000) - 11_644_473_600)
    } else { None };
    let modified = if modified_raw != 0 {
        Some((modified_raw / 10_000_000) - 11_644_473_600)
    } else { None };

    Some((filename, parent_ref, real_size, allocated_size, created, modified))
}

fn parse_data_attr_first_cluster(attr: &[u8]) -> Option<u64> {
    if attr.len() < 9 { return None; }
    let non_resident = attr[8];
    if non_resident == 0 { return None; } // Resident data, no cluster

    if attr.len() < 64 { return None; }
    let data_run_offset = u16::from_le_bytes([attr[32], attr[33]]) as usize;
    if data_run_offset >= attr.len() { return None; }

    // Parse first data run: header byte encodes length_bytes and offset_bytes
    let header = attr[data_run_offset];
    if header == 0 { return None; }

    let len_bytes = (header & 0x0F) as usize;
    let off_bytes = ((header >> 4) & 0x0F) as usize;

    if data_run_offset + 1 + len_bytes + off_bytes > attr.len() { return None; }

    // Extract LCN (cluster offset) from the data run
    let off_start = data_run_offset + 1 + len_bytes;
    let mut lcn: i64 = 0;
    for i in 0..off_bytes {
        lcn |= (attr[off_start + i] as i64) << (8 * i);
    }
    // Sign-extend
    if off_bytes > 0 && attr[off_start + off_bytes - 1] & 0x80 != 0 {
        lcn |= !((1i64 << (off_bytes * 8)) - 1);
    }

    if lcn > 0 { Some(lcn as u64) } else { None }
}
