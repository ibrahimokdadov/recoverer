use recoverer_engine::store::{Store, NewFile};
use tempfile::tempdir;

fn make_store() -> Store {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    Store::open(db_path.to_str().unwrap()).unwrap()
}

fn jpeg_file(filename: &str, confidence: u8) -> NewFile {
    NewFile {
        filename: Some(filename.to_string()),
        original_path: Some("C:\\Users\\test\\Pictures".to_string()),
        mime_type: "image/jpeg".to_string(),
        category: "Images".to_string(),
        size_bytes: 1024 * 1024,
        first_cluster: Some(12345),
        confidence,
        source: "mft".to_string(),
        mft_record_number: Some(100),
        created_at: None,
        modified_at: Some(1700000000),
        deleted_at: None,
    }
}

#[test]
fn insert_and_retrieve_file() {
    let store = make_store();
    let id = store.insert_file(&jpeg_file("photo.jpg", 95)).unwrap();
    assert!(id > 0);

    let files = store.query_files(None, None, None, 0, 10).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].filename.as_deref(), Some("photo.jpg"));
}

#[test]
fn filter_by_category() {
    let store = make_store();
    store.insert_file(&jpeg_file("photo.jpg", 95)).unwrap();
    store.insert_file(&NewFile {
        filename: Some("doc.pdf".to_string()),
        mime_type: "application/pdf".to_string(),
        category: "Documents".to_string(),
        confidence: 90,
        source: "mft".to_string(),
        ..jpeg_file("doc.pdf", 90)
    }).unwrap();

    let images = store.query_files(Some("Images"), None, None, 0, 10).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].category, "Images");
}

#[test]
fn filter_by_min_confidence() {
    let store = make_store();
    store.insert_file(&jpeg_file("high.jpg", 90)).unwrap();
    store.insert_file(&jpeg_file("low.jpg", 40)).unwrap();

    let high = store.query_files(None, Some(80), None, 0, 10).unwrap();
    assert_eq!(high.len(), 1);
    assert_eq!(high[0].filename.as_deref(), Some("high.jpg"));
}

#[test]
fn filter_by_name_contains() {
    let store = make_store();
    store.insert_file(&jpeg_file("vacation_2023.jpg", 90)).unwrap();
    store.insert_file(&jpeg_file("work_report.jpg", 90)).unwrap();

    let results = store.query_files(None, None, Some("vacation"), 0, 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].filename.as_deref(), Some("vacation_2023.jpg"));
}

#[test]
fn total_count_matches() {
    let store = make_store();
    for i in 0..5 {
        store.insert_file(&jpeg_file(&format!("photo{i}.jpg"), 90)).unwrap();
    }
    let count = store.total_count(None, None, None).unwrap();
    assert_eq!(count, 5);
}

#[test]
fn update_recovery_status() {
    let store = make_store();
    let id = store.insert_file(&jpeg_file("photo.jpg", 95)).unwrap();
    store.update_recovery_status(id, "recovered").unwrap();

    let files = store.query_files(None, None, None, 0, 10).unwrap();
    assert_eq!(files[0].recovery_status, recoverer_engine::events::RecoveryStatus::Recovered);
}
