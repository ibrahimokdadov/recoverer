use recoverer_engine::recovery::{is_same_volume, build_destination_path, RecoveryOptions};
use tempfile::tempdir;

#[test]
fn same_volume_detection_same_drive() {
    let source = "C:\\";
    let dest = "C:\\Users\\test\\Recovered";
    assert!(is_same_volume(source, dest));
}

#[test]
fn same_volume_detection_different_drive() {
    let source = "C:\\";
    let dest = "D:\\Recovered";
    assert!(!is_same_volume(source, dest));
}

#[test]
fn build_path_with_structure() {
    let opts = RecoveryOptions {
        destination: "D:\\Recovered".to_string(),
        recreate_structure: true,
        on_conflict: recoverer_engine::recovery::ConflictMode::AddSuffix,
    };
    let path = build_destination_path(
        &opts,
        Some("vacation.jpg"),
        Some("C:\\Users\\test\\Pictures"),
        "image/jpeg",
        0,
    );
    let s = path.to_string_lossy();
    // Destination root is present and original path components are reconstructed under it
    assert!(s.contains("Recovered"), "path missing destination: {s}");
    assert!(s.contains("Users"), "path missing original dir structure: {s}");
    assert!(s.ends_with("vacation.jpg"), "path missing filename: {s}");
}

#[test]
fn build_path_flat_mode() {
    let opts = RecoveryOptions {
        destination: "D:\\Recovered".to_string(),
        recreate_structure: false,
        on_conflict: recoverer_engine::recovery::ConflictMode::AddSuffix,
    };
    let path = build_destination_path(
        &opts,
        Some("vacation.jpg"),
        Some("C:\\Users\\test\\Pictures"),
        "image/jpeg",
        0,
    );
    assert_eq!(path.parent().unwrap().to_string_lossy(), "D:\\Recovered");
}

#[test]
fn build_path_generates_name_for_carved_file() {
    let opts = RecoveryOptions {
        destination: "D:\\Recovered".to_string(),
        recreate_structure: false,
        on_conflict: recoverer_engine::recovery::ConflictMode::AddSuffix,
    };
    let path = build_destination_path(&opts, None, None, "image/jpeg", 12345);
    let name = path.file_name().unwrap().to_string_lossy().to_string();
    assert!(name.ends_with(".jpg"), "expected .jpg extension, got: {}", name);
    assert!(name.contains("recovered"), "expected 'recovered' in name, got: {}", name);
}

#[test]
fn copy_file_to_destination() {
    use recoverer_engine::recovery::copy_file_content;

    let src_dir = tempdir().unwrap();
    let dst_dir = tempdir().unwrap();

    let src_path = src_dir.path().join("source.txt");
    std::fs::write(&src_path, b"hello recovery world").unwrap();

    let dst_path = dst_dir.path().join("recovered.txt");
    copy_file_content(&src_path, &dst_path).unwrap();

    let content = std::fs::read(&dst_path).unwrap();
    assert_eq!(content, b"hello recovery world");
}
