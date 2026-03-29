use serde::{Deserialize, Serialize};

fn default_true() -> bool { true }

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ScanDepth {
    Quick,
    Deep,
    CarveOnly,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Command {
    StartScan {
        drive: String,
        depth: ScanDepth,
        categories: Vec<String>,
    },
    PauseScan,
    ResumeScan,
    CancelScan,
    RecoverFiles {
        file_ids: Vec<i64>,
        destination: String,
        recreate_structure: bool,
    },
    QueryFiles {
        category: Option<String>,
        min_confidence: Option<i32>,
        name_contains: Option<String>,
        offset: u64,
        limit: u64,
        #[serde(default)]
        exclude_recovered: bool,
        #[serde(default = "default_true")]
        collapse_fragments: bool,
    },
    ListSessions,
    SwitchSession {
        session_id: i64,
    },
    /// After a new scan completes: cross-reference files in the active session DB
    /// against clusters recovered in any previous scan of the same drive and
    /// pre-mark them as 'recovered'. Sends Pong when done.
    ApplyScanHistory,
    Ping,
}
