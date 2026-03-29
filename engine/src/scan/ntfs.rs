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
    let total_sectors = u64::from_le_bytes(sector[40..48].try_into().ok().ok_or(EngineError::NotNtfs)?);
    let mft_lcn = i64::from_le_bytes(sector[48..56].try_into().ok().ok_or(EngineError::NotNtfs)?);

    Ok(NtfsBootSector {
        bytes_per_sector,
        sectors_per_cluster,
        bytes_per_cluster,
        mft_lcn,
        total_sectors,
    })
}

/// Apply NTFS update-sequence fixup to an MFT record buffer in-place.
/// NTFS replaces the last 2 bytes of every 512-byte sector with its sequence
/// number; the originals are stored in the fixup array. Without this, any
/// attribute whose length field straddles a sector boundary is read with
/// garbage bytes, silently breaking the entire attribute chain.
pub fn apply_fixup(data: &mut [u8]) {
    if data.len() < 8 { return; }
    let fixup_offset = u16::from_le_bytes([data[4], data[5]]) as usize;
    let fixup_count  = u16::from_le_bytes([data[6], data[7]]) as usize;
    // fixup_count = number_of_sectors + 1; a 1024-byte record has 3 entries
    if fixup_count < 2 || fixup_offset + fixup_count * 2 > data.len() { return; }
    for i in 1..fixup_count {
        let boundary = i * 512 - 2; // last 2 bytes of each 512-byte sector
        if boundary + 2 > data.len() { break; }
        let orig_lo = data[fixup_offset + i * 2];
        let orig_hi = data[fixup_offset + i * 2 + 1];
        data[boundary]     = orig_lo;
        data[boundary + 1] = orig_hi;
    }
}

// ── Run-list parsing ─────────────────────────────────────────────────────────

/// Parse a non-resident NTFS data-run list from a $DATA attribute.
/// Returns (lcn, cluster_count) pairs for each physical extent.
/// Sparse runs (no physical location) are skipped.
pub fn parse_data_runs(data_attr: &[u8]) -> Vec<(u64, u64)> {
    if data_attr.len() < 34 { return vec![]; }
    if data_attr[8] == 0 { return vec![]; } // resident attribute

    let run_offset = u16::from_le_bytes([data_attr[32], data_attr[33]]) as usize;
    if run_offset >= data_attr.len() { return vec![]; }

    let mut runs = Vec::new();
    let mut current_lcn: i64 = 0;
    let mut pos = run_offset;

    while pos < data_attr.len() {
        let header = data_attr[pos];
        if header == 0 { break; }

        let len_bytes = (header & 0x0F) as usize;
        let off_bytes = ((header >> 4) & 0x0F) as usize;

        if len_bytes == 0 { break; }
        if pos + 1 + len_bytes + off_bytes > data_attr.len() { break; }

        // Run length in clusters
        let mut run_length: u64 = 0;
        for i in 0..len_bytes {
            run_length |= (data_attr[pos + 1 + i] as u64) << (8 * i);
        }
        pos += 1 + len_bytes;

        if off_bytes > 0 {
            // LCN delta (sign-extended)
            let mut lcn_delta: i64 = 0;
            for i in 0..off_bytes {
                lcn_delta |= (data_attr[pos + i] as i64) << (8 * i);
            }
            if data_attr[pos + off_bytes - 1] & 0x80 != 0 {
                lcn_delta |= !((1i64 << (off_bytes * 8)) - 1);
            }
            current_lcn += lcn_delta;
            pos += off_bytes;

            if current_lcn > 0 && run_length > 0 {
                runs.push((current_lcn as u64, run_length));
            }
        } else {
            // off_bytes == 0 → sparse run, no physical clusters
        }
    }

    runs
}

/// Parse MFT record 0 (already fixup-applied) to discover the complete MFT
/// extent list. Returns (sector_runs, total_record_count) where each run is
/// (start_sector, length_in_sectors). Falls back to empty vec on any failure.
pub fn parse_mft_extents(record0: &[u8], boot: &NtfsBootSector) -> (Vec<(u64, u64)>, u64) {
    if record0.len() < 42 || &record0[0..4] != b"FILE" {
        return (vec![], 0);
    }

    let first_attr_offset = u16::from_le_bytes([record0[20], record0[21]]) as usize;
    if first_attr_offset >= record0.len() { return (vec![], 0); }

    let mut offset = first_attr_offset;

    while offset + 8 <= record0.len() {
        let attr_type = u32::from_le_bytes(record0[offset..offset + 4].try_into().unwrap_or([0xFF; 4]));
        if attr_type == 0xFFFFFFFF { break; }

        let attr_len = u32::from_le_bytes(record0[offset + 4..offset + 8].try_into().unwrap_or([0; 4])) as usize;
        if attr_len == 0 || offset + attr_len > record0.len() { break; }

        if attr_type == 0x80 {
            let attr = &record0[offset..offset + attr_len];
            let lcn_runs = parse_data_runs(attr);
            if lcn_runs.is_empty() { break; }

            let spc = boot.sectors_per_cluster as u64;
            let sector_runs: Vec<(u64, u64)> = lcn_runs.iter()
                .map(|&(lcn, len)| (lcn * spc, len * spc))
                .collect();

            // Exact record count from data_size field (non-resident attr, offset 48)
            let total_records = if attr.len() >= 56 && attr[8] != 0 {
                let data_size = u64::from_le_bytes(attr[48..56].try_into().unwrap_or([0; 8]));
                (data_size / 1024).max(1)
            } else {
                let total_sectors: u64 = sector_runs.iter().map(|(_, l)| l).sum();
                let record_size_sectors = (1024u64 / boot.bytes_per_sector as u64).max(1);
                (total_sectors / record_size_sectors).max(1)
            };

            log::info!("MFT: {} extent(s), {} total records", sector_runs.len(), total_records);
            for (i, &(s, l)) in sector_runs.iter().enumerate() {
                let rec_size_s = (1024u64 / boot.bytes_per_sector as u64).max(1);
                log::info!("  extent[{}]: start_sector={} length_sectors={} (~{} records)",
                    i, s, l, l / rec_size_s);
            }

            return (sector_runs, total_records);
        }

        offset += attr_len;
    }

    (vec![], 0)
}

// ── MFT record parsing ───────────────────────────────────────────────────────

/// Parse one MFT record (1024 bytes). Returns None if not a valid FILE record.
pub fn parse_mft_record(data: &[u8], record_number: u32) -> Option<MftRecord> {
    if data.len() < 42 { return None; }
    if &data[0..4] != b"FILE" { return None; }

    // Apply fixup before parsing — restores bytes at sector boundaries
    // (offsets 510-511 and 1022-1023) that NTFS clobbers with its sequence number.
    let mut owned = data.to_vec();
    apply_fixup(&mut owned);
    let data = owned.as_slice();

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

    let mut filename: Option<String> = None;
    let mut filename_ns: u8 = 255;   // 255 = not set
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
                Ok(b) => b, Err(_) => break,
            }
        );
        if attr_type == 0xFFFFFFFF { break; }

        if offset + 8 > data.len() { break; }
        let attr_len = u32::from_le_bytes(
            data[offset + 4..offset + 8].try_into().unwrap_or([0; 4])
        ) as usize;
        if attr_len == 0 || offset + attr_len > data.len() { break; }

        match attr_type {
            0x30 => {
                // $FILE_NAME — prefer Win32/Win32&DOS (ns=1,3) over DOS/8.3 (ns=2) or POSIX (ns=0)
                if let Some((fname, pref, fs, als, cr, mo, ns)) =
                    parse_filename_attr(&data[offset..offset + attr_len])
                {
                    let priority = ns_priority(ns);
                    if filename.is_none() || priority > ns_priority(filename_ns) {
                        filename = Some(fname);
                        filename_ns = ns;
                        parent_ref = Some(pref);
                        file_size = fs;
                        allocated_size = als;
                        created_at = cr;
                        modified_at = mo;
                    }
                }
            }
            0x80 => {
                // $DATA — get first cluster of the first run
                if first_data_cluster.is_none() {
                    if let Some(cluster) = parse_data_attr_first_cluster(&data[offset..offset + attr_len]) {
                        first_data_cluster = Some(cluster);
                    }
                }
            }
            0x20 => {
                // $ATTRIBUTE_LIST — signals this file has extension records.
                // For deleted files this mainly matters when the $DATA attribute
                // itself was moved to an extension record.  We flag it so the
                // filter below does not silently drop these records.
                // Full attribute-list resolution would require reading extra MFT
                // records; mark cluster as 0 (unknown) so the file is still
                // listed, and recovery can attempt a best-effort read.
                if first_data_cluster.is_none() {
                    first_data_cluster = Some(0); // placeholder — means "attr list present"
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

// Higher return value = preferred namespace
fn ns_priority(ns: u8) -> u8 {
    match ns {
        3 | 1 => 2, // Win32&DOS or Win32 — full long name
        0     => 1, // POSIX — case-sensitive variant
        _     => 0, // DOS/8.3 short name — last resort
    }
}

fn parse_filename_attr(attr: &[u8]) -> Option<(String, u64, u64, u64, Option<i64>, Option<i64>, u8)> {
    if attr.len() < 26 { return None; }

    let non_resident = attr[8];
    let content_offset = u16::from_le_bytes([attr[20], attr[21]]) as usize;

    if content_offset > attr.len() { return None; }
    let content = if non_resident == 0 {
        &attr[content_offset..]
    } else {
        return None; // $FILE_NAME is always resident
    };

    if content.len() < 66 { return None; }

    let parent_ref = u64::from_le_bytes(content[0..8].try_into().ok()?) & 0x0000FFFFFFFFFFFF;
    let created_raw  = i64::from_le_bytes(content[8..16].try_into().unwrap_or([0; 8]));
    let modified_raw = i64::from_le_bytes(content[16..24].try_into().unwrap_or([0; 8]));
    let allocated_size = u64::from_le_bytes(content[40..48].try_into().unwrap_or([0; 8]));
    let real_size      = u64::from_le_bytes(content[48..56].try_into().unwrap_or([0; 8]));
    let name_len  = content[64] as usize;
    let namespace = content[65]; // 0=POSIX 1=Win32 2=DOS 3=Win32&DOS

    if content.len() < 66 + name_len * 2 { return None; }

    let name_u16: Vec<u16> = (0..name_len)
        .map(|i| u16::from_le_bytes([content[66 + i * 2], content[66 + i * 2 + 1]]))
        .collect();
    let filename = String::from_utf16_lossy(&name_u16).to_string();

    let created = if created_raw != 0 {
        Some((created_raw / 10_000_000) - 11_644_473_600)
    } else { None };
    let modified = if modified_raw != 0 {
        Some((modified_raw / 10_000_000) - 11_644_473_600)
    } else { None };

    Some((filename, parent_ref, real_size, allocated_size, created, modified, namespace))
}

fn parse_data_attr_first_cluster(attr: &[u8]) -> Option<u64> {
    if attr.len() < 9 { return None; }
    let non_resident = attr[8];
    if non_resident == 0 { return None; }

    if attr.len() < 34 { return None; }
    let data_run_offset = u16::from_le_bytes([attr[32], attr[33]]) as usize;
    if data_run_offset >= attr.len() { return None; }

    let header = attr[data_run_offset];
    if header == 0 { return None; }

    let len_bytes = (header & 0x0F) as usize;
    let off_bytes = ((header >> 4) & 0x0F) as usize;

    if off_bytes > 7 { return None; }
    if data_run_offset + 1 + len_bytes + off_bytes > attr.len() { return None; }

    let off_start = data_run_offset + 1 + len_bytes;
    let mut lcn: i64 = 0;
    for i in 0..off_bytes {
        lcn |= (attr[off_start + i] as i64) << (8 * i);
    }
    if off_bytes > 0 && attr[off_start + off_bytes - 1] & 0x80 != 0 {
        lcn |= !((1i64 << (off_bytes * 8)) - 1);
    }

    if lcn > 0 { Some(lcn as u64) } else { None }
}
