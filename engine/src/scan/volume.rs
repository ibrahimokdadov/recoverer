// engine/src/scan/volume.rs
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Storage::FileSystem::{
        CreateFileW, FILE_BEGIN, FILE_FLAGS_AND_ATTRIBUTES,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, SetFilePointerEx,
    },
    System::IO::DeviceIoControl,
    System::Ioctl::{
        GET_LENGTH_INFORMATION, IOCTL_DISK_GET_LENGTH_INFO, IOCTL_STORAGE_QUERY_PROPERTY,
        PropertyStandardQuery, STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
        StorageAccessAlignmentProperty,
    },
};
#[cfg(target_os = "windows")]
use windows::core::PCWSTR;
use crate::error::{EngineError, Result};

pub struct VolumeReader {
    #[cfg(target_os = "windows")]
    handle: HANDLE,
    pub bytes_per_sector: u32,
    pub total_sectors: u64,
}

#[cfg(target_os = "windows")]
impl Drop for VolumeReader {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

impl VolumeReader {
    #[cfg(target_os = "windows")]
    pub fn open(drive: &str) -> Result<Self> {
        // Normalize: "C:" or "C:\" -> "\\.\C:"
        let letter = drive.trim_end_matches('\\').trim_end_matches(':');
        let path: Vec<u16> = format!("\\\\.\\{}:", letter)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                PCWSTR(path.as_ptr()),
                0x80000000u32, // GENERIC_READ
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0), // no FILE_FLAG_NO_BUFFERING: compatible with all drive types incl. USB
                None,
            )
        }
        .map_err(|_| EngineError::VolumeAccessDenied(drive.to_string()))?;

        // No INVALID_HANDLE_VALUE check needed — windows-rs returns Err on failure

        let bytes_per_sector = query_sector_size(handle).unwrap_or_else(|| {
            log::warn!("Could not query sector size for {} — falling back to 512 bytes; may fail on 4Kn drives", drive);
            512
        });
        let total_sectors = query_total_sectors(handle, bytes_per_sector);

        Ok(Self {
            handle,
            bytes_per_sector,
            total_sectors,
        })
    }

    #[cfg(not(target_os = "windows"))]
    pub fn open(_drive: &str) -> Result<Self> {
        Err(EngineError::VolumeNotFound(
            "Only supported on Windows".to_string(),
        ))
    }

    pub fn read_sector(&self, lba: u64) -> Result<Vec<u8>> {
        self.read_sectors(lba, 1)
    }

    #[cfg(target_os = "windows")]
    pub fn read_sectors(&self, lba: u64, count: u32) -> Result<Vec<u8>> {
        let sector_size = self.bytes_per_sector as usize;
        let byte_offset = (lba * self.bytes_per_sector as u64) as i64;
        let buf_size = sector_size
            .checked_mul(count as usize)
            .ok_or_else(|| EngineError::Io(std::io::Error::other("read size overflow")))?;

        let mut buf = vec![0u8; buf_size];

        // Position the file pointer
        let seek_ok = unsafe { SetFilePointerEx(self.handle, byte_offset, None, FILE_BEGIN) };
        if let Err(e) = seek_ok {
            return Err(EngineError::Io(std::io::Error::from_raw_os_error(e.code().0)));
        }

        // Synchronous ReadFile
        let mut bytes_read = 0u32;
        let read_ok = unsafe {
            ReadFile(self.handle, Some(buf.as_mut_slice()), Some(&mut bytes_read), None)
        };

        if read_ok.is_ok() && bytes_read as usize == buf_size {
            Ok(buf)
        } else {
            Err(EngineError::Io(std::io::Error::last_os_error()))
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn read_sectors(&self, _lba: u64, _count: u32) -> Result<Vec<u8>> {
        Err(EngineError::VolumeNotFound(
            "Only supported on Windows".to_string(),
        ))
    }
}

#[cfg(target_os = "windows")]
fn query_sector_size(handle: HANDLE) -> Option<u32> {
    let query = STORAGE_PROPERTY_QUERY {
        PropertyId: StorageAccessAlignmentProperty,
        QueryType: PropertyStandardQuery,
        ..Default::default()
    };
    let mut desc = STORAGE_ACCESS_ALIGNMENT_DESCRIPTOR::default();
    let mut bytes_returned = 0u32;

    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_STORAGE_QUERY_PROPERTY,
            Some(&query as *const _ as *const std::ffi::c_void),
            std::mem::size_of_val(&query) as u32,
            Some(&mut desc as *mut _ as *mut std::ffi::c_void),
            std::mem::size_of_val(&desc) as u32,
            Some(&mut bytes_returned),
            None,
        )
    };

    if ok.is_ok() {
        Some(desc.BytesPerLogicalSector)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
fn query_total_sectors(handle: HANDLE, bytes_per_sector: u32) -> u64 {
    let mut info = GET_LENGTH_INFORMATION::default();
    let mut bytes_ret = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            IOCTL_DISK_GET_LENGTH_INFO,
            None,
            0,
            Some(&mut info as *mut _ as *mut std::ffi::c_void),
            std::mem::size_of_val(&info) as u32,
            Some(&mut bytes_ret),
            None,
        )
    };
    if ok.is_ok() {
        (info.Length as u64) / bytes_per_sector as u64
    } else {
        log::warn!("IOCTL_DISK_GET_LENGTH_INFO failed — total_sectors will be 0; scan coverage unknown");
        0
    }
}
