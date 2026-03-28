use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ScanDepth {
    Quick,
    Deep,
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
    },
    Ping,
}
