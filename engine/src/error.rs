// engine/src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EngineError {
    #[error("Volume access denied: {0}")]
    VolumeAccessDenied(String),
    #[error("Volume not found: {0}")]
    VolumeNotFound(String),
    #[error("Not an NTFS volume")]
    NotNtfs,
    #[error("Corrupt MFT record at offset {offset}: {reason}")]
    CorruptMftRecord { offset: u64, reason: String },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("Destination is same volume as source — refusing to recover")]
    SameVolumeDenied,
    #[error("Destination path not found: {0}")]
    DestinationNotFound(String),
    #[cfg(target_os = "windows")]
    #[error("Windows API error: {0}")]
    WindowsApi(#[from] windows::core::Error),
}

pub type Result<T> = std::result::Result<T, EngineError>;
