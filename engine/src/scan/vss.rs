// engine/src/scan/vss.rs
/// VSS (Volume Shadow Copy) enumeration.
/// Lists available shadow copies for a volume and can recover files from them.
/// This is the fastest recovery path — no raw disk access needed.

pub struct VssCandidate {
    pub original_path: String,
    pub shadow_path: String,
    pub size_bytes: u64,
    pub modified_at: Option<i64>,
}

/// List VSS shadow copies for a given drive letter (e.g., "C:").
/// Returns an empty vec on non-Windows or if VSS is unavailable.
pub fn list_shadow_copies(_drive: &str) -> Vec<String> {
    // TODO: implement via Win32 VSS API (IVssBackupComponents)
    // For v1, return empty — VSS integration is post-launch
    vec![]
}

/// Enumerate deleted files visible in VSS shadow copies.
pub fn enumerate_deleted_in_vss(_drive: &str) -> Vec<VssCandidate> {
    // TODO: mount shadow copy, walk filesystem, compare against live filesystem
    vec![]
}
