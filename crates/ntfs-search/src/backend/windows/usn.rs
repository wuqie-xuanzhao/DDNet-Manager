//! Windows USN backend（普通用户路径）。
//!
//! 通过 FSCTL_ENUM_USN_DATA 枚举整个 USN journal，每条 USN_RECORD V2 解析得到
//! 文件名 + parent_ref + 元数据。扫完后反向重建路径。
//!
//! 与 admin $MFT 路径相比：
//! - 不需要管理员权限
//! - 速度比 walkdir 快（顺序 IO），比 $MFT 慢
//! - created/accessed 时间拿不到（只有 modified）
//! - 文件 size 拿不到（FileEntry.size = 0）
//!
//! 性能预期：百万文件级盘，10-30 秒扫完。

use crate::backend::windows::volume::VolumeHandle;
use crate::error::ScanError;
use crate::options::{
    BackendKind, FileAttributes, FileEntry, NtfsScanOptions, ProgressEvent, ScanLimitKind,
};
use crate::rebuild_paths::{rebuild_path, with_drive_prefix, RecordInfo, NTFS_ROOT_FILE_REFERENCE};
use crate::ProgressSink;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio_util::sync::CancellationToken;
use windows::Win32::System::Ioctl::FSCTL_ENUM_USN_DATA;
use windows::Win32::System::IO::DeviceIoControl;

const USN_ENUM_BUFFER_SIZE: usize = 64 * 1024;
const USN_RECORD_V2_FIXED_HEADER: usize = 60;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

/// USN 后端扫描入口。spawn_blocking 包同步 DeviceIoControl 循环。
pub(crate) async fn scan(
    volume: VolumeHandle,
    opts: &NtfsScanOptions,
    progress: Arc<dyn ProgressSink>,
    cancel: CancellationToken,
) -> Result<Vec<FileEntry>, ScanError> {
    let info = volume.query_usn_journal()?;
    let drive = volume.drive_letter();

    let matcher = Arc::clone(&opts.matcher);
    let max_results = opts.max_results;
    let max_records = opts.max_records_scanned.unwrap_or(usize::MAX);

    tokio::task::spawn_blocking(move || {
        let mut map: HashMap<u64, RecordInfo> = HashMap::new();
        let mut matched: Vec<MatchedRecord> = Vec::new();
        let mut scanned = 0usize;
        let mut next_start: u64 = 0;
        let mut last_emit = Instant::now();

        // USN_ENUM_DATA_V2: { StartFileReferenceNumber (u64), LowUsn (i64), HighUsn (i64) }
        let mut enum_data: [u8; 24] = [0; 24];
        enum_data[0..8].copy_from_slice(&0u64.to_le_bytes());
        enum_data[8..16].copy_from_slice(&0i64.to_le_bytes());
        enum_data[16..24].copy_from_slice(&info.next_usn.to_le_bytes());

        let mut buf = vec![0u8; USN_ENUM_BUFFER_SIZE];

        loop {
            if cancel.is_cancelled() {
                break;
            }

            // 更新 StartFileReferenceNumber
            enum_data[0..8].copy_from_slice(&next_start.to_le_bytes());

            let mut bytes_returned: u32 = 0;
            // SAFETY: volume.raw_handle() 由 CreateFileW 返回；buffer 是有效的 64KB
            // Vec<u8>；enum_data 是 24 字节栈数组。
            let result = unsafe {
                DeviceIoControl(
                    volume.raw_handle(),
                    FSCTL_ENUM_USN_DATA,
                    Some(enum_data.as_ptr() as *const _),
                    enum_data.len() as u32,
                    Some(buf.as_mut_ptr() as *mut _),
                    buf.len() as u32,
                    Some(&mut bytes_returned),
                    None,
                )
            };

            if result.is_err() {
                return Err(ScanError::UsnEnumFailed {
                    root: format!("{}:", drive),
                    detail: format!("FSCTL_ENUM_USN_DATA returned error: {:?}", result.err()),
                });
            }

            if bytes_returned < 8 {
                break; // 无更多数据
            }

            // buffer 前 8 字节是下次 StartFileReference
            let new_start = u64::from_le_bytes(buf[0..8].try_into().unwrap_or([0u8; 8]));
            let records_buf = &buf[8..bytes_returned as usize];

            if records_buf.is_empty() || new_start == next_start {
                break; // 无进展，防死循环
            }

            // 解析变长 USN_RECORD V2 链
            let mut offset = 0usize;
            while offset + USN_RECORD_V2_FIXED_HEADER <= records_buf.len() {
                if cancel.is_cancelled() {
                    break;
                }

                let record = &records_buf[offset..];
                let record_len =
                    u32::from_le_bytes(record[0..4].try_into().unwrap_or([0u8; 4])) as usize;
                if record_len < USN_RECORD_V2_FIXED_HEADER
                    || offset + record_len > records_buf.len()
                {
                    break; // record 残缺
                }

                let major = u16::from_le_bytes(record[4..6].try_into().unwrap_or([0u8; 2]));
                if major != 2 {
                    // V3/V4 跳过（v0.1 不支持）
                    offset += record_len;
                    continue;
                }

                let file_ref = u64::from_le_bytes(record[8..16].try_into().unwrap_or([0u8; 8]));
                let parent_ref = u64::from_le_bytes(record[16..24].try_into().unwrap_or([0u8; 8]));
                let timestamp = u64::from_le_bytes(record[32..40].try_into().unwrap_or([0u8; 8]));
                let attrs = u32::from_le_bytes(record[52..56].try_into().unwrap_or([0u8; 4]));
                let name_len =
                    u16::from_le_bytes(record[56..58].try_into().unwrap_or([0u8; 2])) as usize;
                let name_offset =
                    u16::from_le_bytes(record[58..60].try_into().unwrap_or([0u8; 2])) as usize;

                if name_offset + name_len > record_len {
                    offset += record_len;
                    continue; // 字段越界
                }

                let name_utf16: Vec<u16> = record[name_offset..name_offset + name_len]
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                let name = String::from_utf16_lossy(&name_utf16);

                // FileRef 低 48 位为 record 序号、高 16 位为 sequence
                let file_ref_id = file_ref & 0x0000_FFFF_FFFF_FFFF;
                let parent_ref_id = parent_ref & 0x0000_FFFF_FFFF_FFFF;

                if (matcher)(&name) {
                    matched.push(MatchedRecord {
                        file_ref: file_ref_id,
                        timestamp,
                        attributes: attrs,
                    });
                    if let Some(max) = max_results {
                        if matched.len() >= max {
                            progress.emit(ProgressEvent::ScanLimitHit {
                                kind: ScanLimitKind::Results,
                                limit: max,
                            });
                            // 提前退出（即使 record map 不完整，路径重建会跳过 stale ref）
                            return finish_scan(map, matched, drive, progress.as_ref(), scanned);
                        }
                    }
                }

                // 全部 record 都加入 map（路径重建需要祖先链）
                // 但根节点（FileRef 5）不应该覆盖
                if file_ref_id != NTFS_ROOT_FILE_REFERENCE {
                    map.insert(
                        file_ref_id,
                        RecordInfo {
                            file_name: name,
                            parent_reference: parent_ref_id,
                        },
                    );
                }

                scanned += 1;
                offset += record_len;

                // 周期 emit + 上限保护
                if scanned % 4096 == 0 && last_emit.elapsed() > Duration::from_millis(100) {
                    progress.emit(ProgressEvent::EntriesFound {
                        found: matched.len(),
                    });
                    last_emit = Instant::now();
                }
                if scanned >= max_records {
                    progress.emit(ProgressEvent::ScanLimitHit {
                        kind: ScanLimitKind::RecordsScanned,
                        limit: max_records,
                    });
                    return finish_scan(map, matched, drive, progress.as_ref(), scanned);
                }
            }

            next_start = new_start;
        }

        finish_scan(map, matched, drive, progress.as_ref(), scanned)
    })
    .await
    .map_err(|e| ScanError::Internal(format!("USN spawn_blocking join: {e}")))?
}

struct MatchedRecord {
    file_ref: u64,
    timestamp: u64,
    attributes: u32,
}

fn finish_scan(
    map: HashMap<u64, RecordInfo>,
    matched: Vec<MatchedRecord>,
    drive: char,
    progress: &dyn ProgressSink,
    scanned: usize,
) -> Result<Vec<FileEntry>, ScanError> {
    let mut entries: Vec<FileEntry> = Vec::with_capacity(matched.len());

    for m in matched {
        let path_result = rebuild_path(&map, m.file_ref);
        match path_result {
            Ok(rel) => {
                let full = with_drive_prefix(&rel, drive);
                entries.push(FileEntry {
                    path: full,
                    size: 0,                         // USN V2 不含 size
                    created: SystemTime::UNIX_EPOCH, // USN V2 不含 created
                    modified: filetime_to_system_time(m.timestamp),
                    accessed: SystemTime::UNIX_EPOCH, // USN V2 不含 accessed
                    attributes: FileAttributes::from_bits_truncate(m.attributes),
                    is_directory: (m.attributes & FILE_ATTRIBUTE_DIRECTORY) != 0,
                    backend: BackendKind::Usn,
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

    let _ = scanned; // 已 emit 过 EntriesFound
    Ok(entries)
}

/// Windows FILETIME（自 1601-01-01 的 100ns 单位）转 SystemTime。
fn filetime_to_system_time(filetime: u64) -> SystemTime {
    if filetime == 0 {
        return SystemTime::UNIX_EPOCH;
    }
    // FILETIME: 100ns 间隔 since 1601-01-01
    // UNIX epoch: 1970-01-01 = 11644473600 秒 since 1601
    const FILETIME_UNIX_OFFSET: u64 = 116_444_736_000_000_000; // 100ns 单位
    let unix_100ns = filetime.saturating_sub(FILETIME_UNIX_OFFSET);
    let secs = unix_100ns / 10_000_000;
    let nanos = ((unix_100ns % 10_000_000) * 100) as u32;
    SystemTime::UNIX_EPOCH + Duration::new(secs, nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetime_zero_is_unix_epoch() {
        assert_eq!(filetime_to_system_time(0), SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn filetime_known_value() {
        // 2024-01-01 00:00:00 UTC = Unix epoch 后 1704067200 秒
        // FILETIME = (1704067200 + 11644473600) * 10^7 = 133485408000000000
        let ft = 133_485_408_000_000_000u64;
        let st = filetime_to_system_time(ft);
        let duration_since_epoch = st
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("should be after epoch");
        assert_eq!(duration_since_epoch.as_secs(), 1_704_067_200);
    }

    #[test]
    fn filetime_before_unix_epoch_falls_back_to_epoch() {
        // 1601-01-01 = 0；介于 1601 和 1970 之间的值 → saturating 到 0 → UNIX_EPOCH
        let ft = 100_000_000u64; // 远小于 116_444_736_000_000_000
        let st = filetime_to_system_time(ft);
        assert_eq!(st, SystemTime::UNIX_EPOCH);
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "needs real C: drive; run with --ignored"]
    async fn real_usn_scan_or_fallback_finds_notepad() {
        // 在 Win11 默认安全策略下，普通用户打开 raw volume handle 会 Access Denied，
        // UsnBackend 自动降级到 WalkdirBackend（emit BackendDowngraded）。
        // 这个测试验证：无论走 USN 还是 fallback，都能扫到 notepad.exe。
        use crate::backend::windows::UsnBackend;
        use crate::backend::Backend;
        use crate::options::NtfsScanOptions;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let downgrades = Arc::new(AtomicUsize::new(0));
        let downgrades_clone = Arc::clone(&downgrades);
        let sink = crate::sink_from(move |ev| {
            if matches!(ev, crate::ProgressEvent::BackendDowngraded { .. }) {
                downgrades_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        let backend = UsnBackend::new('C');
        let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("notepad.exe"))
            .with_root(std::path::PathBuf::from("C:\\Windows\\System32"));
        let entries = backend
            .scan_root(
                std::path::Path::new("C:\\Windows\\System32"),
                &opts,
                sink,
                CancellationToken::new(),
            )
            .await
            .expect("scan_root");

        eprintln!(
            "found {} notepad.exe copies (downgrades = {})",
            entries.len(),
            downgrades.load(Ordering::Relaxed)
        );
        assert!(
            entries
                .iter()
                .any(|e| e.path.to_string_lossy().contains("System32")),
            "should find System32\\notepad.exe (via USN or walkdir fallback)"
        );
    }
}
