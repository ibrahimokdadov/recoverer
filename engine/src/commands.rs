use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Command {
    StartScan {
        drive: String,
        depth: String,
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
        offset: i64,
        limit: i64,
    },
    Ping,
}
