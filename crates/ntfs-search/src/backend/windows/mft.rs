//! Windows $MFT raw record backend（admin 路径）。
//!
//! 通过 CreateFileW(r"\\.\C:$MFT") 直接读 $MFT 文件 $DATA 流，按 BytesPerFileRecordSegment
//! 偏移顺序读，调 mft_record::parse_mft_record 解析。
//!
//! 与 USN 路径相比：
//! - **需要**管理员权限（或 SeBackupPrivilege）打开 $MFT 句柄
//! - 速度最快（顺序读 $MFT，几秒扫 C 盘）
//! - 拿到完整 timestamps（created/modified/accessed）+ 真实文件 size
//!
//! 探测：CreateFileW($MFT) 失败说明无 admin → fallback USN → fallback Walkdir。

use crate::backend::walkdir::WalkdirBackend;
use crate::backend::windows::mft_record::parse_mft_record;
use crate::backend::windows::volume::VolumeHandle;
use crate::backend::Backend;
use crate::error::ScanError;
use crate::options::{
    BackendKind, FileAttributes, FileEntry, NtfsScanOptions, ProgressEvent, ScanLimitKind,
};
use crate::rebuild_paths::{rebuild_path, with_drive_prefix, RecordInfo};
use crate::ProgressSink;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio_util::sync::CancellationToken;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{FSCTL_GET_NTFS_VOLUME_DATA, NTFS_VOLUME_DATA_BUFFER};
use windows::Win32::System::IO::DeviceIoControl;

const MFT_READ_BUFFER_SIZE: usize = 64 * 1024;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

/// MFT backend：尝试用 admin 权限直接读 $MFT 文件。失败时 fallback USN/Walkdir。
pub(crate) struct MftBackend {
    drive_letter: char,
}

impl MftBackend {
    pub(crate) fn new(drive_letter: char) -> Self {
        Self { drive_letter }
    }
}

#[async_trait]
impl Backend for MftBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Mft
    }

    async fn scan_root(
        &self,
        root: &Path,
        opts: &NtfsScanOptions,
        progress: Arc<dyn ProgressSink>,
        cancel: CancellationToken,
    ) -> Result<Vec<FileEntry>, ScanError> {
        // 子树路径直接走 Walkdir（同 UsnBackend 语义）
        if !super::is_whole_drive_root(root, self.drive_letter) {
            tracing::debug!(
                root = %root.display(),
                drive = %self.drive_letter,
                "MFT backend got subtree path; using walkdir directly"
            );
            return WalkdirBackend.scan_root(root, opts, progress, cancel).await;
        }

        // 尝试打开 $MFT 句柄
        let mft_handle = match open_mft_file(self.drive_letter) {
            Ok(h) => h,
            Err(e) => {
                return fallback_to_usn_or_walkdir(
                    root,
                    opts,
                    progress,
                    cancel,
                    &format!("open $MFT failed: {e}"),
                )
                .await;
            }
        };

        // 拿 volume metadata（BytesPerFileRecordSegment）
        let vol_meta = match VolumeHandle::open(self.drive_letter) {
            Ok(v) => match query_volume_data(&v) {
                Ok(m) => m,
                Err(e) => {
                    return fallback_to_usn_or_walkdir(
                        root,
                        opts,
                        progress,
                        cancel,
                        &format!("query NTFS volume data failed: {e}"),
                    )
                    .await;
                }
            },
            Err(e) => {
                return fallback_to_usn_or_walkdir(
                    root,
                    opts,
                    progress,
                    cancel,
                    &format!("open volume for metadata failed: {e}"),
                )
                .await;
            }
        };

        // 扫描
        let result = scan_via_mft(
            mft_handle,
            vol_meta.bytes_per_record,
            vol_meta.mft_valid_data_length,
            self.drive_letter,
            opts,
            Arc::clone(&progress),
            cancel.clone(),
        )
        .await;

        match result {
            Ok(entries) => Ok(entries),
            Err(e) => {
                fallback_to_usn_or_walkdir(
                    root,
                    opts,
                    progress,
                    cancel,
                    &format!("MFT scan failed: {e}"),
                )
                .await
            }
        }
    }
}

/// NTFS 卷元数据（从 FSCTL_GET_NTFS_VOLUME_DATA 提取）。
struct VolumeMeta {
    bytes_per_record: u64,
    mft_valid_data_length: u64,
}

fn query_volume_data(vol: &VolumeHandle) -> Result<VolumeMeta, ScanError> {
    let mut data = NTFS_VOLUME_DATA_BUFFER::default();
    let mut bytes_returned: u32 = 0;

    // SAFETY: handle 由 CreateFileW 返回；data 是有效栈地址；显式 c_void 匹配 DeviceIoControl。
    let result = unsafe {
        DeviceIoControl(
            vol.raw_handle(),
            FSCTL_GET_NTFS_VOLUME_DATA,
            None,
            0,
            Some(&mut data as *mut NTFS_VOLUME_DATA_BUFFER as *mut std::ffi::c_void),
            std::mem::size_of::<NTFS_VOLUME_DATA_BUFFER>() as u32,
            Some(&mut bytes_returned),
            None,
        )
    };

    if result.is_err() {
        return Err(ScanError::MftReadFailed {
            root: format!("{}:", vol.drive_letter()),
            detail: format!("FSCTL_GET_NTFS_VOLUME_DATA failed: {:?}", result.err()),
        });
    }

    Ok(VolumeMeta {
        bytes_per_record: data.BytesPerFileRecordSegment as u64,
        mft_valid_data_length: data.MftValidDataLength as u64,
    })
}

/// $MFT 文件句柄。RAII：drop 时 CloseHandle。
struct MftFileHandle {
    handle: HANDLE,
    drive_letter: char,
}

// SAFETY: 同 VolumeHandle——HANDLE 跨 spawn_blocking 边界需要 Send。
// 前提：所有 ReadFile 调用必须同步（lpOverlapped = NULL）。
unsafe impl Send for MftFileHandle {}

impl MftFileHandle {
    /// 流式读 $MFT，从 offset 开始填 buffer。
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, ScanError> {
        // 先 SeekToEnd via SetFilePointerEx
        use windows::Win32::Storage::FileSystem::SetFilePointerEx;
        let distance: i64 = offset as i64;

        // SAFETY: handle 由 CreateFileW 返回；distance 是值类型。
        let seek_result = unsafe {
            SetFilePointerEx(
                self.handle,
                distance,
                None,
                windows::Win32::Storage::FileSystem::FILE_BEGIN,
            )
        };
        if seek_result.is_err() {
            return Err(ScanError::MftReadFailed {
                root: format!("{}:", self.drive_letter),
                detail: format!("SetFilePointerEx failed: {:?}", seek_result.err()),
            });
        }

        let mut total_read = 0usize;
        while total_read < buf.len() {
            let mut bytes_read: u32 = 0;
            let buf_slice = &mut buf[total_read..];

            // SAFETY: handle 由 CreateFileW 返回；buf_slice 是有效可变借用；同步读。
            let result =
                unsafe { ReadFile(self.handle, Some(buf_slice), Some(&mut bytes_read), None) };
            if result.is_err() {
                return Err(ScanError::MftReadFailed {
                    root: format!("{}:", self.drive_letter),
                    detail: format!("ReadFile failed: {:?}", result.err()),
                });
            }
            if bytes_read == 0 {
                break; // EOF
            }
            total_read += bytes_read as usize;
        }

        Ok(total_read)
    }
}

impl Drop for MftFileHandle {
    fn drop(&mut self) {
        // SAFETY: 句柄独占持有；同步关闭。
        if let Err(e) = unsafe { CloseHandle(self.handle) } {
            tracing::error!(
                drive = %self.drive_letter,
                error = %e,
                "CloseHandle failed during MftFileHandle drop"
            );
        }
    }
}

/// 打开 `\\.\C:$MFT` 句柄。需要 admin 或 SeBackupPrivilege。
fn open_mft_file(drive_letter: char) -> Result<MftFileHandle, ScanError> {
    let drive = drive_letter.to_ascii_uppercase();
    let path = format!("\\\\.\\{}:$MFT", drive);
    let mut wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();

    // SAFETY: CreateFileW 是 Win32 API；PCWSTR 来自 wide buffer（含 NUL）。
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
        root: format!("{}:$MFT", drive),
        source: std::io::Error::from(e),
    })?;

    Ok(MftFileHandle {
        handle,
        drive_letter: drive,
    })
}

/// 主扫描：spawn_blocking 内流式读 $MFT，按 record 解析。
async fn scan_via_mft(
    mft: MftFileHandle,
    bytes_per_record: u64,
    mft_valid_data_length: u64,
    drive: char,
    opts: &NtfsScanOptions,
    progress: Arc<dyn ProgressSink>,
    cancel: CancellationToken,
) -> Result<Vec<FileEntry>, ScanError> {
    let matcher = Arc::clone(&opts.matcher);
    let max_results = opts.max_results;
    let max_records = opts.max_records_scanned.unwrap_or(usize::MAX);

    if bytes_per_record == 0 {
        return Err(ScanError::MftReadFailed {
            root: format!("{}:", drive),
            detail: "BytesPerFileRecordSegment is zero".to_string(),
        });
    }

    // 读 buffer：每次读 MFT_READ_BUFFER_SIZE / bytes_per_record 条 record
    let records_per_buf = (MFT_READ_BUFFER_SIZE as u64 / bytes_per_record).max(1);
    let buf_size = (records_per_buf * bytes_per_record) as usize;

    tokio::task::spawn_blocking(move || {
        let mut map: HashMap<u64, RecordInfo> = HashMap::new();
        let mut matched: Vec<MatchedMft> = Vec::new();
        let mut scanned = 0usize;
        let mut last_emit = Instant::now();
        let mut limit_kind: Option<ScanLimitKind> = None;
        let mut limit_value: usize = 0;

        let mut buf = vec![0u8; buf_size];
        let total_records = mft_valid_data_length / bytes_per_record;
        let mut record_index: u64 = 0;

        while record_index < total_records {
            if cancel.is_cancelled() {
                break;
            }

            let offset = record_index * bytes_per_record;
            let n = mft.read_at(offset, &mut buf)?;
            if n < bytes_per_record as usize {
                break; // EOF 或短读
            }

            let records_in_buf = (n as u64) / bytes_per_record;
            for i in 0..records_in_buf {
                if cancel.is_cancelled() {
                    break;
                }

                let start = (i * bytes_per_record) as usize;
                let end = start + bytes_per_record as usize;
                let record_bytes = &buf[start..end];

                let Some(parsed) = parse_mft_record(record_bytes) else {
                    continue; // 跳过无法解析的 record（如未使用 slot）
                };

                let file_ref_id = record_index + i;
                scanned += 1;

                if (matcher)(&parsed.file_name) {
                    matched.push(MatchedMft {
                        file_ref: file_ref_id,
                        created: parsed.created,
                        modified: parsed.modified,
                        accessed: parsed.accessed,
                        attributes: parsed.attributes,
                        size: parsed.size,
                    });
                }

                map.insert(
                    file_ref_id,
                    RecordInfo {
                        file_name: parsed.file_name,
                        parent_reference: parsed.parent_reference,
                    },
                );

                if scanned % 4096 == 0 && last_emit.elapsed() > Duration::from_millis(100) {
                    progress.emit(ProgressEvent::EntriesFound {
                        found: matched.len(),
                    });
                    last_emit = Instant::now();
                }
            }

            record_index += records_in_buf;

            // 检查 max_results（buffer 处理完后，避免 stale ref）
            if let Some(max) = max_results {
                if matched.len() >= max {
                    matched.truncate(max);
                    limit_kind = Some(ScanLimitKind::Results);
                    limit_value = max;
                    break;
                }
            }

            if scanned >= max_records {
                limit_kind = Some(ScanLimitKind::RecordsScanned);
                limit_value = max_records;
                break;
            }
        }

        if let Some(kind) = limit_kind {
            progress.emit(ProgressEvent::ScanLimitHit {
                kind,
                limit: limit_value,
            });
        }

        // 路径重建 + FileEntry 构造
        let mut entries: Vec<FileEntry> = Vec::with_capacity(matched.len());
        for m in matched {
            match rebuild_path(&map, m.file_ref) {
                Ok(rel) => {
                    let full = with_drive_prefix(&rel, drive);
                    entries.push(FileEntry {
                        path: full,
                        size: m.size,
                        created: filetime_to_system_time(m.created),
                        modified: filetime_to_system_time(m.modified),
                        accessed: filetime_to_system_time(m.accessed),
                        attributes: FileAttributes::from_bits_truncate(m.attributes),
                        is_directory: (m.attributes & FILE_ATTRIBUTE_DIRECTORY) != 0,
                        backend: BackendKind::Mft,
                        file_reference: Some(m.file_ref),
                    });
                }
                Err(e) => {
                    progress.emit(ProgressEvent::EntryError {
                        path: None,
                        error: format!("rebuild_path for ref {} failed: {:?}", m.file_ref, e),
                    });
                }
            }
        }

        Ok(entries)
    })
    .await
    .map_err(|e| ScanError::Internal(format!("MFT spawn_blocking join: {e}")))?
}

/// MFT scan 期间的临时匹配 record 缓存（路径重建前）。
struct MatchedMft {
    file_ref: u64,
    created: u64,
    modified: u64,
    accessed: u64,
    attributes: u32,
    size: u64,
}

/// Fallback：先尝试 USN，USN 失败再走 Walkdir。
async fn fallback_to_usn_or_walkdir(
    root: &Path,
    opts: &NtfsScanOptions,
    progress: Arc<dyn ProgressSink>,
    cancel: CancellationToken,
    reason: &str,
) -> Result<Vec<FileEntry>, ScanError> {
    progress.emit(ProgressEvent::BackendDowngraded {
        root: root.to_path_buf(),
        from: BackendKind::Mft,
        to: BackendKind::Usn,
        reason: reason.to_string(),
    });

    // 复用 UsnBackend 的失败自动 fallback Walkdir 逻辑
    let drive = super::volume::path_to_drive_letter(root).unwrap_or('C');
    let usn = super::UsnBackend::new(drive);
    usn.scan_root(root, opts, progress, cancel).await
}

fn filetime_to_system_time(filetime: u64) -> SystemTime {
    if filetime == 0 {
        return SystemTime::UNIX_EPOCH;
    }
    const FILETIME_UNIX_OFFSET: u64 = 116_444_736_000_000_000;
    let unix_100ns = filetime.saturating_sub(FILETIME_UNIX_OFFSET);
    let secs = unix_100ns / 10_000_000;
    let nanos = ((unix_100ns % 10_000_000) * 100) as u32;
    SystemTime::UNIX_EPOCH + Duration::new(secs, nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_zero_is_epoch() {
        assert_eq!(filetime_to_system_time(0), SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn filetime_known_value() {
        let ft = 133_485_408_000_000_000u64;
        let st = filetime_to_system_time(ft);
        assert_eq!(
            st.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
            1_704_067_200
        );
    }

    #[test]
    fn filetime_pre_epoch_falls_back() {
        let st = filetime_to_system_time(100);
        assert_eq!(st, SystemTime::UNIX_EPOCH);
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "needs admin to open $MFT; run with --ignored"]
    async fn real_mft_scan_finds_notepad_on_admin() {
        use crate::backend::Backend;
        use crate::options::NtfsScanOptions;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let downgrades = Arc::new(AtomicUsize::new(0));
        let downgrades_clone = Arc::clone(&downgrades);
        let sink = crate::sink_from(move |ev| {
            if matches!(ev, crate::ProgressEvent::BackendDowngraded { .. }) {
                downgrades_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        let backend = MftBackend::new('C');
        let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("notepad.exe"))
            .with_root(PathBuf::from("C:\\"));
        let entries = backend
            .scan_root(Path::new("C:\\"), &opts, sink, CancellationToken::new())
            .await
            .expect("scan_root");

        eprintln!(
            "found {} notepad.exe (downgrades = {})",
            entries.len(),
            downgrades.load(Ordering::Relaxed)
        );
        // admin 环境：应在 System32 找到 notepad.exe（backend = Mft）
        // 普通环境：自动 fallback Usn → Walkdir，仍能找到
        assert!(
            entries
                .iter()
                .any(|e| e.path.to_string_lossy().contains("System32")),
            "should find System32\\notepad.exe"
        );
    }
}
