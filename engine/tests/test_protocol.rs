use recoverer_engine::commands::Command;
use recoverer_engine::events::Event;

#[test]
fn deserialize_start_scan_command() {
    let json = r#"{"type":"StartScan","drive":"C:\\","depth":"deep","categories":["Images","Videos"]}"#;
    let cmd: Command = serde_json::from_str(json).unwrap();
    match cmd {
        Command::StartScan { drive, depth, categories } => {
            assert_eq!(drive, "C:\\");
            assert_eq!(depth, "deep");
            assert_eq!(categories, vec!["Images", "Videos"]);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn deserialize_recover_command() {
    let json = r#"{"type":"RecoverFiles","file_ids":[1,2,3],"destination":"D:\\Recovered","recreate_structure":true}"#;
    let cmd: Command = serde_json::from_str(json).unwrap();
    match cmd {
        Command::RecoverFiles { file_ids, destination, recreate_structure } => {
            assert_eq!(file_ids, vec![1, 2, 3]);
            assert_eq!(destination, "D:\\Recovered");
            assert!(recreate_structure);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn serialize_file_found_event() {
    let event = Event::FileFound {
        id: 42,
        filename: Some("photo.jpg".to_string()),
        original_path: Some("C:\\Users\\test\\Pictures".to_string()),
        size_bytes: 1024 * 1024,
        mime_type: "image/jpeg".to_string(),
        category: "Images".to_string(),
        confidence: 95,
        source: "mft".to_string(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"event\":\"FileFound\""));
    assert!(json.contains("\"filename\":\"photo.jpg\""));
}

#[test]
fn serialize_progress_event() {
    let event = Event::Progress {
        phase: "mft_scan".to_string(),
        pct: 47,
        files_found: 1247,
        eta_secs: Some(840),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"pct\":47"));
}
