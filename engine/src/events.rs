use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScanSession {
    pub id:          i64,
    pub name:        String,
    pub drive:       String,
    pub db_path:     String,
    pub created_at:  i64,
    pub total_files: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryStatus {
    Pending,
    Recovered,
    Failed,
    Skipped,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "event")]
pub enum Event {
    Pong,
    Progress {
        phase: String,
        pct: u8,
        files_found: u64,
        eta_secs: Option<u64>,
    },
    PhaseChange {
        new_phase: String,
    },
    FileFound {
        id: i64,
        filename: Option<String>,
        original_path: Option<String>,
        size_bytes: u64,
        mime_type: String,
        category: String,
        confidence: u8,
        source: String,
    },
    ScanComplete {
        total_found: u64,
        duration_secs: u64,
    },
    RecoveryProgress {
        recovered: u64,
        warnings: u64,
        failed: u64,
        total: u64,
    },
    RecoveryComplete {
        recovered: u64,
        warnings: u64,
        failed: u64,
    },
    Error {
        code: String,
        message: String,
        fatal: bool,
    },
    FilesPage {
        files: Vec<FileRecord>,
        total_count: i64,
    },
    SessionsList {
        sessions: Vec<ScanSession>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileRecord {
    pub id: i64,
    pub filename: Option<String>,
    pub original_path: Option<String>,
    pub mime_type: String,
    pub category: String,
    pub size_bytes: u64,
    pub confidence: u8,
    pub source: String,
    pub recovery_status:   RecoveryStatus,
    /// Unix timestamp (seconds since 1970-01-01 UTC) of the file's last modification.
    pub modified_at:       Option<i64>,
    /// Non-zero means this file is part of a fragment chain (same video split across extents).
    pub fragment_group_id: i64,
}
