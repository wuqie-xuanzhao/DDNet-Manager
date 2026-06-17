//! Windows 卷句柄封装。提供 safe Rust 接口给上层 backend。
//!
//! 安全铁律（见设计稿 §10）：
//! - 所有句柄用 `GENERIC_READ` 打开
//! - `FILE_SHARE_READ | FILE_SHARE_WRITE` 共享，允许其他进程并发
//! - 只调用只读 NTFS IOCTL（FSCTL_GET_NTFS_VOLUME_DATA / FSCTL_QUERY_USN_JOURNAL /
//!   FSCTL_ENUM_USN_DATA / FSCTL_GET_NTFS_FILE_RECORD 等）

use crate::error::ScanError;
use std::path::Path;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{FSCTL_QUERY_USN_JOURNAL, USN_JOURNAL_DATA_V0};
use windows::Win32::System::IO::DeviceIoControl;

/// 卷信息：从 FSCTL_QUERY_USN_JOURNAL 拿到的关键字段。
#[derive(Debug, Clone, Copy)]
pub(crate) struct JournalInfo {
    #[allow(dead_code)]
    pub(crate) journal_id: u64,
    #[allow(dead_code)]
    pub(crate) first_usn: i64,
    pub(crate) next_usn: i64,
}

/// 只读卷句柄。RAII：drop 时 CloseHandle。
pub(crate) struct VolumeHandle {
    handle: HANDLE,
    drive_letter: char,
}

// SAFETY: HANDLE 是 Win32 句柄（void*），但本结构对它的访问全部在持有期间
// 单线程同步完成。tokio::spawn_blocking 的 future 跨线程通过需要 Send。
// HANDLE 本身在 Win32 中是进程级而非线程级，可以跨线程传递使用。
unsafe impl Send for VolumeHandle {}

impl VolumeHandle {
    /// 打开 `\\.\C:` 形式的卷句柄。
    ///
    /// 共享模式：`FILE_SHARE_READ | FILE_SHARE_WRITE`，允许 Listary/Everything/chkdsk
    /// 同时持有同卷的 read handle。NTFS 多读单写语义保证无锁冲突。
    pub(crate) fn open(drive_letter: char) -> Result<Self, ScanError> {
        let drive = drive_letter.to_ascii_uppercase();
        let path = format!("\\\\.\\{}:", drive);
        let mut wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

        // SAFETY: CreateFileW 是 Win32 API；PCWSTR 来自我们维护的 wide buffer（含 NUL）。
        let handle = unsafe {
            CreateFileW(
                PCWSTR::from_raw(wide.as_mut_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|e| ScanError::VolumeOpenFailed {
            root: format!("{}:", drive),
            source: std::io::Error::from(e),
        })?;

        Ok(Self {
            handle,
            drive_letter: drive,
        })
    }

    pub(crate) fn drive_letter(&self) -> char {
        self.drive_letter
    }

    /// 查询 USN journal 元信息。失败说明该卷不支持 USN（FAT32/exFAT/ReFS 未启用等）。
    pub(crate) fn query_usn_journal(&self) -> Result<JournalInfo, ScanError> {
        let mut data = USN_JOURNAL_DATA_V0::default();
        let mut bytes_returned = 0u32;

        // SAFETY: handle 由 CreateFileW 返回；data 是有效栈地址。
        let success = unsafe {
            DeviceIoControl(
                self.handle,
                FSCTL_QUERY_USN_JOURNAL,
                None,
                0,
                Some(&mut data as *mut _ as *mut _),
                std::mem::size_of::<USN_JOURNAL_DATA_V0>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        };

        if success.is_err() || bytes_returned < std::mem::size_of::<USN_JOURNAL_DATA_V0>() as u32 {
            return Err(ScanError::UsnEnumFailed {
                root: format!("{}:", self.drive_letter),
                detail: "FSCTL_QUERY_USN_JOURNAL failed (volume may not support USN)".to_string(),
            });
        }

        Ok(JournalInfo {
            journal_id: data.UsnJournalID,
            first_usn: data.FirstUsn,
            next_usn: data.NextUsn,
        })
    }

    /// 原始 DeviceIoControl 调用，由 usn.rs/mft.rs 直接使用 unsafe 句柄。
    ///
    /// 安全保证：调用方负责 buffer 对齐与生命周期；本函数仅暴露 handle。
    pub(crate) fn raw_handle(&self) -> HANDLE {
        self.handle
    }
}

impl Drop for VolumeHandle {
    fn drop(&mut self) {
        // SAFETY: 句柄由我们独占持有，drop 时不会再有其他操作。
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// 从 Path 提取单字母盘符。如 `C:\Windows` → `Some('C')`。
pub(crate) fn path_to_drive_letter(path: &Path) -> Option<char> {
    let s = path.to_str()?;
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic() {
        Some(bytes[0] as char)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_drive_letter_extracts_c() {
        assert_eq!(path_to_drive_letter(Path::new("C:\\Windows")), Some('C'));
        assert_eq!(path_to_drive_letter(Path::new("d:/games")), Some('d'));
    }

    #[test]
    fn path_to_drive_letter_returns_none_for_unc() {
        assert_eq!(path_to_drive_letter(Path::new("\\\\server\\share")), None);
        assert_eq!(path_to_drive_letter(Path::new("/usr/local")), None);
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "needs real C: drive; run with --ignored"]
    async fn open_volume_c_succeeds_on_windows() {
        let v = VolumeHandle::open('C').expect("open C:");
        let info = v.query_usn_journal().expect("query journal");
        eprintln!(
            "C: journal id = {}, first = {}, next = {}",
            info.journal_id, info.first_usn, info.next_usn
        );
    }
}
