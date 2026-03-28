// engine/src/scan/volume.rs
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        CreateFileW, ReadFile, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_NO_BUFFERING,
        FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    },
    System::IO::{DeviceIoControl, OVERLAPPED},
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
                FILE_FLAGS_AND_ATTRIBUTES(FILE_FLAG_NO_BUFFERING.0),
                None,
            )
        }
        .map_err(|_| EngineError::VolumeAccessDenied(drive.to_string()))?;

        if handle == INVALID_HANDLE_VALUE {
            return Err(EngineError::VolumeAccessDenied(drive.to_string()));
        }

        let bytes_per_sector = query_sector_size(handle).unwrap_or(512);
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
        let byte_offset = lba * self.bytes_per_sector as u64;
        let buf_size = sector_size * count as usize;

        // Allocate sector-aligned buffer (required for FILE_FLAG_NO_BUFFERING)
        let layout = std::alloc::Layout::from_size_align(buf_size, sector_size)
            .map_err(|_| EngineError::Io(std::io::Error::other("alignment error")))?;
        let buf_ptr = unsafe { std::alloc::alloc(layout) };
        if buf_ptr.is_null() {
            return Err(EngineError::Io(std::io::Error::other("allocation failed")));
        }

        let mut overlapped = OVERLAPPED::default();
        overlapped.Anonymous.Anonymous.Offset = byte_offset as u32;
        overlapped.Anonymous.Anonymous.OffsetHigh = (byte_offset >> 32) as u32;

        let mut bytes_read = 0u32;
        let buf_slice =
            unsafe { std::slice::from_raw_parts_mut(buf_ptr, buf_size) };

        let ok = unsafe {
            ReadFile(
                self.handle,
                Some(buf_slice),
                Some(&mut bytes_read),
                Some(&mut overlapped),
            )
        };

        let result = if ok.is_ok() && bytes_read as usize == buf_size {
            let mut out = vec![0u8; buf_size];
            unsafe {
                std::ptr::copy_nonoverlapping(buf_ptr, out.as_mut_ptr(), buf_size);
            }
            Ok(out)
        } else {
            Err(EngineError::Io(std::io::Error::last_os_error()))
        };

        unsafe { std::alloc::dealloc(buf_ptr, layout) };
        result
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
        0
    }
}
