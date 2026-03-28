// engine/tests/test_volume.rs
// These tests require Windows + admin rights. Run with:
// cargo test test_volume -- --ignored
// (on a Windows machine with admin privileges)

#[cfg(target_os = "windows")]
mod windows_tests {
    use recoverer_engine::scan::volume::VolumeReader;

    #[test]
    #[ignore]
    fn open_c_drive_and_read_boot_sector() {
        // Requires admin
        let reader = VolumeReader::open("C:").expect("Failed to open C: — run as admin");
        let sector = reader.read_sector(0).expect("Failed to read sector 0");
        assert_eq!(sector.len(), 512);
        // NTFS signature at offset 3
        assert_eq!(&sector[3..7], b"NTFS");
    }

    #[test]
    #[ignore]
    fn read_aligned_sectors() {
        let reader = VolumeReader::open("C:").unwrap();
        let sectors = reader.read_sectors(0, 8).unwrap();
        assert_eq!(sectors.len(), 512 * 8);
    }
}
