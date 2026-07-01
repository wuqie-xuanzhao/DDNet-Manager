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

use super::filetime_to_system_time;

const USN_ENUM_BUFFER_SIZE: usize = 64 * 1024;
pub(crate) const USN_RECORD_V2_FIXED_HEADER: usize = 60;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
#[allow(dead_code)]
const FILE_REF_SEQ_MASK: u64 = 0xFFFF_0000_0000_0000;
const FILE_REF_RECORD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

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
        let mut limit_kind: Option<ScanLimitKind> = None;
        let mut limit_value: usize = 0;

        // USN_ENUM_DATA_V2 字节布局（手动构造，避免依赖 windows-rs 版本差异）：
        //   StartFileReferenceNumber: u64 (offset 0)
        //   LowUsn:  i64 (offset 8)
        //   HighUsn: i64 (offset 16)
        let mut enum_data: [u8; 24] = [0; 24];
        enum_data[8..16].copy_from_slice(&0i64.to_le_bytes());
        enum_data[16..24].copy_from_slice(&info.next_usn.to_le_bytes());

        let mut buf = vec![0u8; USN_ENUM_BUFFER_SIZE];

        loop {
            if cancel.is_cancelled() {
                break;
            }

            enum_data[0..8].copy_from_slice(&next_start.to_le_bytes());

            let mut bytes_returned: u32 = 0;
            // SAFETY: volume.raw_handle() 由 CreateFileW 返回；buffer 是有效的 64KB
            // Vec<u8>；enum_data 是 24 字节栈数组。DeviceIoControl 的 lpBuffer 类型是
            // *mut c_void，需要显式 cast 避免推断错位。
            let result = unsafe {
                DeviceIoControl(
                    volume.raw_handle(),
                    FSCTL_ENUM_USN_DATA,
                    Some(enum_data.as_ptr() as *const std::ffi::c_void),
                    enum_data.len() as u32,
                    Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
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

            // 处理整个 buffer（不在内部 max_results 提前返回，避免路径重建 stale）
            let delta_scanned =
                process_records_buffer(records_buf, &*matcher, &mut map, &mut matched, &cancel);
            scanned += delta_scanned;

            // 周期 emit
            if last_emit.elapsed() > Duration::from_millis(100) {
                progress.emit(ProgressEvent::EntriesFound {
                    found: matched.len(),
                });
                last_emit = Instant::now();
            }

            // 检查 max_results（整个 buffer 处理完后，map 完整）
            if let Some(max) = max_results {
                if matched.len() >= max {
                    matched.truncate(max);
                    limit_kind = Some(ScanLimitKind::Results);
                    limit_value = max;
                    break;
                }
            }

            // 检查 max_records_scanned
            if scanned >= max_records {
                limit_kind = Some(ScanLimitKind::RecordsScanned);
                limit_value = max_records;
                break;
            }

            next_start = new_start;
        }

        if let Some(kind) = limit_kind {
            progress.emit(ProgressEvent::ScanLimitHit {
                limit_kind: kind,
                limit: limit_value,
            });
        }

        finish_scan(map, matched, drive, progress.as_ref())
    })
    .await
    .map_err(|e| ScanError::Internal(format!("USN spawn_blocking join: {e}")))?
}

/// 解析单条 USN_RECORD V2 字节流。返回 None 表示 record 残缺 / 字段越界。
///
/// 此函数纯字节解析，不依赖 Windows API，可跨平台单元测试。
/// M3 接入 $MFT record 解析时会复用同一模式（fixture bytes → 字段提取）。
pub(crate) fn parse_one_record(record: &[u8]) -> Option<ParsedRecord> {
    if record.len() < USN_RECORD_V2_FIXED_HEADER {
        return None;
    }

    let record_len = u32::from_le_bytes(record[0..4].try_into().ok()?) as usize;
    if record_len < USN_RECORD_V2_FIXED_HEADER || record_len > record.len() {
        return None;
    }

    let major = u16::from_le_bytes(record[4..6].try_into().ok()?);
    let file_ref = u64::from_le_bytes(record[8..16].try_into().ok()?);
    let parent_ref = u64::from_le_bytes(record[16..24].try_into().ok()?);
    let timestamp = u64::from_le_bytes(record[32..40].try_into().ok()?);
    let attrs = u32::from_le_bytes(record[52..56].try_into().ok()?);
    let name_len = u16::from_le_bytes(record[56..58].try_into().ok()?) as usize;
    let name_offset = u16::from_le_bytes(record[58..60].try_into().ok()?) as usize;

    if name_offset + name_len > record_len {
        return None;
    }

    let name_utf16: Vec<u16> = record[name_offset..name_offset + name_len]
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let name = String::from_utf16_lossy(&name_utf16);

    Some(ParsedRecord {
        record_len,
        major,
        file_ref,
        parent_ref,
        timestamp,
        attributes: attrs,
        name,
    })
}

/// 处理一个 buffer（已去掉前 8 字节 start ref）中的所有 record。
///
/// 返回该 buffer 内成功解析的 record 总数（含被 matcher 过滤的）。
/// 不在此函数内做 max_results 提前返回——让外层 scan 处理，避免 map 不完整导致 stale。
pub(crate) fn process_records_buffer(
    records_buf: &[u8],
    matcher: &dyn Fn(&str) -> bool,
    map: &mut HashMap<u64, RecordInfo>,
    matched: &mut Vec<MatchedRecord>,
    cancel: &CancellationToken,
) -> usize {
    let mut offset = 0usize;
    let mut count = 0usize;

    while offset + USN_RECORD_V2_FIXED_HEADER <= records_buf.len() {
        if cancel.is_cancelled() {
            break;
        }

        let Some(parsed) = parse_one_record(&records_buf[offset..]) else {
            break; // record 残缺，无法继续顺序解析
        };

        let record_len = parsed.record_len;

        // V3/V4 跳过（v0.1 不支持）
        if parsed.major == 2 {
            let file_ref_id = parsed.file_ref & FILE_REF_RECORD_MASK;
            let parent_ref_id = parsed.parent_ref & FILE_REF_RECORD_MASK;

            if matcher(&parsed.name) {
                matched.push(MatchedRecord {
                    file_ref: file_ref_id,
                    timestamp: parsed.timestamp,
                    attributes: parsed.attributes,
                });
            }

            // 全部 record 都加入 map（路径重建需要祖先链）
            // 根节点（FileRef=5）本身不存 name，跳过
            if file_ref_id != NTFS_ROOT_FILE_REFERENCE {
                map.insert(
                    file_ref_id,
                    RecordInfo {
                        file_name: parsed.name,
                        parent_reference: parent_ref_id,
                    },
                );
            }
        }

        count += 1;
        offset += record_len;
    }

    count
}

pub(crate) struct ParsedRecord {
    pub record_len: usize,
    pub major: u16,
    pub file_ref: u64,
    pub parent_ref: u64,
    pub timestamp: u64,
    pub attributes: u32,
    pub name: String,
}

pub(crate) struct MatchedRecord {
    pub file_ref: u64,
    pub timestamp: u64,
    pub attributes: u32,
}

fn finish_scan(
    map: HashMap<u64, RecordInfo>,
    matched: Vec<MatchedRecord>,
    drive: char,
    progress: &dyn ProgressSink,
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

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一条 USN_RECORD V2 字节流。
    fn build_v2_record(
        file_ref: u64,
        parent_ref: u64,
        timestamp: u64,
        attrs: u32,
        name: &str,
    ) -> Vec<u8> {
        let name_utf16: Vec<u16> = name.encode_utf16().collect();
        let name_bytes: Vec<u8> = name_utf16.iter().flat_map(|c| c.to_le_bytes()).collect();
        let name_len = name_bytes.len();
        let name_offset = USN_RECORD_V2_FIXED_HEADER;
        let record_len = name_offset + name_len;

        let mut buf = vec![0u8; record_len];
        buf[0..4].copy_from_slice(&(record_len as u32).to_le_bytes());
        buf[4..6].copy_from_slice(&2u16.to_le_bytes()); // MajorVersion = 2
        buf[6..8].copy_from_slice(&0u16.to_le_bytes()); // MinorVersion = 0
        buf[8..16].copy_from_slice(&file_ref.to_le_bytes());
        buf[16..24].copy_from_slice(&parent_ref.to_le_bytes());
        buf[24..32].copy_from_slice(&0u64.to_le_bytes()); // Usn
        buf[32..40].copy_from_slice(&timestamp.to_le_bytes());
        buf[40..44].copy_from_slice(&0u32.to_le_bytes()); // Reason
        buf[44..48].copy_from_slice(&0u32.to_le_bytes()); // SourceInfo
        buf[48..52].copy_from_slice(&0u32.to_le_bytes()); // SecurityId
        buf[52..56].copy_from_slice(&attrs.to_le_bytes());
        buf[56..58].copy_from_slice(&(name_len as u16).to_le_bytes());
        buf[58..60].copy_from_slice(&(name_offset as u16).to_le_bytes());
        buf[60..].copy_from_slice(&name_bytes);
        buf
    }

    /// 构造一条非 V2（如 V3）record，用于测试跳过逻辑。
    fn build_non_v2_record(major: u16, record_len: usize) -> Vec<u8> {
        let mut buf = vec![0u8; record_len];
        buf[0..4].copy_from_slice(&(record_len as u32).to_le_bytes());
        buf[4..6].copy_from_slice(&major.to_le_bytes());
        buf
    }

    #[test]
    fn parse_v2_record_extracts_all_fields() {
        let bytes = build_v2_record(
            0x0000_0000_0000_0042,
            0x0000_0000_0000_0005,
            133_485_408_000_000_000,
            0x20,
            "test.txt",
        );
        let parsed = parse_one_record(&bytes).expect("should parse");

        assert_eq!(parsed.record_len, bytes.len());
        assert_eq!(parsed.major, 2);
        assert_eq!(parsed.file_ref, 0x42);
        assert_eq!(parsed.parent_ref, 0x5);
        assert_eq!(parsed.timestamp, 133_485_408_000_000_000);
        assert_eq!(parsed.attributes, 0x20);
        assert_eq!(parsed.name, "test.txt");
    }

    #[test]
    fn parse_v2_record_with_utf16_chinese_name() {
        let bytes = build_v2_record(100, 5, 0, 0, "客户端.exe");
        let parsed = parse_one_record(&bytes).expect("should parse");
        assert_eq!(parsed.name, "客户端.exe");
    }

    #[test]
    fn parse_non_v2_record_returns_some_with_major_field() {
        // V3 record：major=3，应能解析但 major != 2（外层会 skip）
        let bytes = build_non_v2_record(3, 80);
        let parsed = parse_one_record(&bytes).expect("should parse structurally");
        assert_eq!(parsed.major, 3);
    }

    #[test]
    fn parse_short_record_returns_none() {
        // 不足 60 字节固定头 → None
        let bytes = vec![0u8; 50];
        assert!(parse_one_record(&bytes).is_none());
    }

    #[test]
    fn parse_record_with_record_len_smaller_than_header_returns_none() {
        // record_len 字段值 < 60 → None
        let mut bytes = vec![0u8; 80];
        bytes[0..4].copy_from_slice(&40u32.to_le_bytes()); // record_len = 40 < 60
        assert!(parse_one_record(&bytes).is_none());
    }

    #[test]
    fn parse_record_with_name_out_of_bounds_returns_none() {
        // name_offset + name_len > record_len → None
        let mut bytes = build_v2_record(1, 5, 0, 0, "ok");
        // 把 name_len 改成超大值
        bytes[56..58].copy_from_slice(&1000u16.to_le_bytes());
        assert!(parse_one_record(&bytes).is_none());
    }

    #[test]
    fn process_buffer_collects_matched_and_map() {
        // 三条 record：file1 (parent 5)、sub_dir (parent 5, dir)、file2 (parent sub_dir)
        let r1 = build_v2_record(100, 5, 0, 0x20, "DDNet.exe");
        let r2 = build_v2_record(101, 5, 0, 0x10, "sub"); // 0x10 = DIRECTORY
        let r3 = build_v2_record(102, 101, 0, 0x20, "nested.exe");
        let mut buf = Vec::new();
        buf.extend_from_slice(&r1);
        buf.extend_from_slice(&r2);
        buf.extend_from_slice(&r3);

        let mut map = HashMap::new();
        let mut matched = Vec::new();
        let cancel = CancellationToken::new();
        let count = process_records_buffer(
            &buf,
            &|n| n.eq_ignore_ascii_case("DDNet.exe"),
            &mut map,
            &mut matched,
            &cancel,
        );

        assert_eq!(count, 3, "all 3 records scanned");
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].file_ref, 100);
        assert_eq!(map.len(), 3); // 3 个非根节点 record 入 map
        assert!(map.contains_key(&100));
        assert!(map.contains_key(&101));
        assert!(map.contains_key(&102));
    }

    #[test]
    fn process_buffer_skips_non_v2_records() {
        let r1 = build_v2_record(100, 5, 0, 0x20, "v2.txt");
        let r2 = build_non_v2_record(3, 80); // V3，应被 skip（不入 map/matched）
        let r3 = build_v2_record(101, 5, 0, 0x20, "v2_also.txt");
        let mut buf = Vec::new();
        buf.extend_from_slice(&r1);
        buf.extend_from_slice(&r2);
        buf.extend_from_slice(&r3);

        let mut map = HashMap::new();
        let mut matched = Vec::new();
        let cancel = CancellationToken::new();
        let count = process_records_buffer(&buf, &|_| true, &mut map, &mut matched, &cancel);

        assert_eq!(count, 3, "all 3 records iterated (V3 counted but skipped)");
        assert_eq!(matched.len(), 2, "only V2 records matched");
        assert_eq!(map.len(), 2, "only V2 records in map");
    }

    #[test]
    fn process_buffer_respects_cancel_mid_way() {
        let mut buf = Vec::new();
        for i in 0..10 {
            let r = build_v2_record(100 + i, 5, 0, 0x20, &format!("f{i}.txt"));
            buf.extend_from_slice(&r);
        }

        let mut map = HashMap::new();
        let mut matched = Vec::new();
        let cancel = CancellationToken::new();
        cancel.cancel();

        let count = process_records_buffer(&buf, &|_| true, &mut map, &mut matched, &cancel);
        assert_eq!(count, 0, "cancel before first record");
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn file_ref_seq_mask_isolates_record_id() {
        // 高 16 位是 sequence number，低 48 位是 record id
        let full_ref = 0x0001_0000_0000_0042u64; // seq=1, record=0x42
        assert_eq!(full_ref & FILE_REF_RECORD_MASK, 0x42);
        assert_eq!(full_ref & FILE_REF_SEQ_MASK, 0x0001_0000_0000_0000);
    }

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
        let ft = 100_000_000u64;
        let st = filetime_to_system_time(ft);
        assert_eq!(st, SystemTime::UNIX_EPOCH);
    }

    /// 真盘测试：UsnBackend 扫 C:\ 找 notepad.exe。
    ///
    /// 运行方式：`cargo test -p ntfs-search --lib -- --ignored real_usn_scan`
    ///
    /// 期望行为（elevation-aware）：
    /// - admin 进程：VolumeHandle::open 成功 → USN 主路径，downgrades=0
    /// - 普通进程：open volume 拒绝访问 → 自动 fallback Walkdir，downgrades=1
    ///   仍能在 System32 找到 notepad.exe
    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "needs real C: drive; admin verifies USN, non-admin verifies fallback (run with --ignored)"]
    async fn real_usn_scan_or_fallback_finds_notepad() {
        use crate::backend::windows::UsnBackend;
        use crate::backend::Backend;
        use crate::options::NtfsScanOptions;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let is_elevated = super::super::elevation::is_process_elevated();
        eprintln!("process elevated = {}", is_elevated);

        let downgrades = Arc::new(AtomicUsize::new(0));
        let downgrades_clone = Arc::clone(&downgrades);
        let sink = crate::sink_from(move |ev| {
            if matches!(ev, crate::ProgressEvent::BackendDowngraded { .. }) {
                downgrades_clone.fetch_add(1, Ordering::Relaxed);
            }
        });

        let backend = UsnBackend::new('C');
        let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("notepad.exe"))
            .with_root(std::path::PathBuf::from("C:\\"));
        let entries = backend
            .scan_root(
                std::path::Path::new("C:\\"),
                &opts,
                sink,
                CancellationToken::new(),
            )
            .await
            .expect("scan_root");

        let downgrade_count = downgrades.load(Ordering::Relaxed);
        eprintln!(
            "found {} notepad.exe copies (downgrades = {})",
            entries.len(),
            downgrade_count
        );

        // admin 应走 USN 主路径（downgrades=0）；普通用户 fallback Walkdir（downgrades=1）
        if is_elevated {
            assert_eq!(
                downgrade_count, 0,
                "elevated process should use USN directly"
            );
        } else {
            assert!(downgrade_count >= 1, "non-elevated process should fallback");
        }

        assert!(
            entries
                .iter()
                .any(|e| e.path.to_string_lossy().contains("System32")),
            "should find System32\\notepad.exe (via USN or walkdir fallback)"
        );
    }
}
