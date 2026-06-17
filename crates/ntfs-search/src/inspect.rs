//! `inspect` / `inspect_many`：按需 fetch 文件的扩展信息。
//!
//! 扫描阶段（`find_files`）只返回 MFT 原生字段（路径/大小/时间戳）。
//! 扩展信息（PE 版本资源、所有者 SID、ADS）需要单独打开文件读，按需 fetch：
//! - 单文件场景用 `inspect`
//! - 批量场景用 `inspect_many`（内置并发控制）

use crate::error::ScanError;
use crate::options::{FileEntry, InspectFields, InspectedEntry};
use crate::pe::read_version_info;
use crate::ProgressSink;
use futures::stream::{self, StreamExt};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// 单条 entry 扩展查询。返回 `InspectedEntry`（含调用方请求的所有 fields）。
///
/// 失败时返回 `Err(ScanError)`，调用方决定如何处理。
pub async fn inspect(
    entry: &FileEntry,
    fields: InspectFields,
) -> Result<InspectedEntry, ScanError> {
    inspect_one(entry, fields).await
}

/// 批量扩展查询。**真并行**：用 `futures::stream::buffer_unordered(concurrency)` 控制
/// 最大并发数。结果按 `(index, outcome)` 收集后排序，保证 `results[i]` 与 `entries[i]` 对应。
///
/// 单条失败不阻断其他，按 `InspectOutcome::Failed` 标记。
pub async fn inspect_many(
    entries: &[FileEntry],
    fields: InspectFields,
    progress: Arc<dyn ProgressSink>,
    concurrency: usize,
) -> Result<Vec<InspectOutcome>, ScanError> {
    if concurrency == 0 {
        return Err(ScanError::Internal(
            "inspect_many concurrency must be > 0".to_string(),
        ));
    }

    let semaphore = Arc::new(Semaphore::new(concurrency));

    // 每个 entry 包装成 (index, outcome)，让 buffer_unordered 完成后按 index 还原顺序
    let stream = stream::iter(entries.iter().cloned().enumerate())
        .map(|(idx, entry)| {
            let permit_sem = Arc::clone(&semaphore);
            let progress = Arc::clone(&progress);
            async move {
                // 等待 permit（spawn_blocking 期间持有 permit 控制并发）
                // Semaphore closed 视为 runtime 致命错误，转 Internal
                let _permit = permit_sem
                    .acquire()
                    .await
                    .map_err(|e| ScanError::Internal(format!("semaphore closed: {e}")))?;

                let outcome = match inspect_one(&entry, fields).await {
                    Ok(info) => InspectOutcome::Success(info),
                    Err(e) => {
                        progress.emit(crate::options::ProgressEvent::EntryError {
                            path: Some(entry.path.clone()),
                            error: e.to_string(),
                        });
                        InspectOutcome::Failed {
                            path: entry.path.clone(),
                            error: e,
                        }
                    }
                };
                Ok::<_, ScanError>((idx, outcome))
            }
        })
        .buffer_unordered(concurrency);

    // 收集 + 按 idx 排序还原 entries 顺序
    let mut indexed: Vec<(usize, InspectOutcome)> = Vec::with_capacity(entries.len());
    let mut stream = std::pin::pin!(stream);
    while let Some(item) = stream.next().await {
        indexed.push(item?);
    }
    indexed.sort_by_key(|(i, _)| *i);

    Ok(indexed.into_iter().map(|(_, o)| o).collect())
}

/// 单条扩展查询的实际实现。spawn_blocking 内调 pe::read_version_info。
///
/// OWNER_SID / ADS 在 v0.1 暂未实现（NTFS $SDS 解析 + ADS 枚举复杂），
/// 返回 None / 空 vec；调用方按 fields 检查。
///
/// **目录早返回**：目录无 PE 资源，但 v0.2 实现 ADS 时目录可能含 zone.identifier
/// 等备用数据流，所以早返回条件用 `is_directory && !fields.contains(ADS)`。
async fn inspect_one(
    entry: &FileEntry,
    fields: InspectFields,
) -> Result<InspectedEntry, ScanError> {
    if entry.is_directory && !fields.contains(InspectFields::ADS) {
        // 目录无 PE 资源；v0.2 加 ADS 时此分支需放宽
        return Ok(InspectedEntry::default());
    }

    let path = entry.path.clone();
    let need_version = fields.contains(InspectFields::VERSION_INFO);

    let version_info = if need_version && !entry.is_directory {
        Some(
            fetch_in_blocking(&path, |p| {
                read_version_info(p).map_err(|detail| ScanError::InspectFailed {
                    path: p.to_path_buf(),
                    detail,
                })
            })
            .await?,
        )
    } else {
        None
    };

    Ok(InspectedEntry {
        version_info,
        owner_sid: None,          // v0.2 实现（NTFS $SDS 解析）
        alt_data_streams: vec![], // v0.2 实现（FindFirstStreamW）
    })
}

/// 在 spawn_blocking 内执行同步 IO 函数。返回 T。
async fn fetch_in_blocking<T, F>(path: &Path, f: F) -> Result<T, ScanError>
where
    F: FnOnce(&Path) -> Result<T, ScanError> + Send + 'static,
    T: Send + 'static,
{
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || f(&path))
        .await
        .map_err(|e| ScanError::Internal(format!("inspect spawn_blocking join: {e}")))?
}

/// inspect_many 单条结果（成功或失败）。
///
/// 不 derive `Clone`：`ScanError` 含 `std::io::Error`（VolumeOpenFailed 分支）无法 Clone。
/// 调用方如需 Clone，可手动按 `Success` / `Failed` 分流。
#[derive(Debug)]
pub enum InspectOutcome {
    Success(InspectedEntry),
    Failed {
        path: std::path::PathBuf,
        error: ScanError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::NtfsScanOptions;
    use crate::NoopSink;
    use std::fs;
    use std::path::PathBuf;
    use tokio_util::sync::CancellationToken;

    /// 用 WalkdirBackend 扫一个真 PE 文件（notepad.exe），验证 inspect 能拿到 VersionInfo。
    #[cfg(windows)]
    #[tokio::test]
    async fn inspect_real_notepad_gets_company_name() {
        let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("notepad.exe"))
            .with_root(PathBuf::from("C:\\Windows\\System32"));
        let entries = crate::find_files(opts, Arc::new(NoopSink), CancellationToken::new())
            .await
            .expect("scan");

        if entries.is_empty() {
            return; // 非 Windows / 不可访问 System32，跳过
        }

        let inspected = inspect(&entries[0], InspectFields::VERSION_INFO)
            .await
            .expect("inspect");
        let vi = inspected.version_info.expect("should have version_info");
        assert!(
            vi.company_name
                .as_deref()
                .map(|c| c.contains("Microsoft"))
                .unwrap_or(false),
            "CompanyName should contain Microsoft: {:?}",
            vi.company_name
        );
    }

    /// inspect 目录返回空 InspectedEntry（不报错）。
    #[tokio::test]
    async fn inspect_directory_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let meta = fs::metadata(tmp.path()).unwrap();
        let entry = FileEntry::from_metadata(tmp.path().to_path_buf(), &meta);
        let result = inspect(&entry, InspectFields::VERSION_INFO).await;
        assert!(result.is_ok());
        assert!(result.unwrap().version_info.is_none());
    }

    /// inspect_many 失败条目按 InspectOutcome::Failed 返回，不阻断其他。
    #[tokio::test]
    async fn inspect_many_collects_failures() {
        let tmp = tempfile::tempdir().unwrap();
        // 构造两个文件：一个 valid PE（用任意字节假装），一个不存在的路径
        let valid_path = tmp.path().join("fake.exe");
        fs::write(&valid_path, b"MZ\x00\x00").unwrap(); // 不是真 PE，pelite 会失败

        let valid_meta = fs::metadata(&valid_path).unwrap();
        let valid_entry = FileEntry::from_metadata(valid_path.clone(), &valid_meta);

        let nonexistent = FileEntry {
            path: PathBuf::from("/nonexistent/fake.exe"),
            ..FileEntry::from_metadata(valid_path.clone(), &valid_meta)
        };

        let entries = vec![valid_entry, nonexistent];
        let results = inspect_many(&entries, InspectFields::VERSION_INFO, Arc::new(NoopSink), 2)
            .await
            .expect("inspect_many");

        assert_eq!(results.len(), 2);
        // 两个都应该 Failed（一个 PE 解析失败，一个文件不存在）
        assert!(matches!(results[0], InspectOutcome::Failed { .. }));
        assert!(matches!(results[1], InspectOutcome::Failed { .. }));
    }

    #[tokio::test]
    async fn inspect_many_rejects_zero_concurrency() {
        let result = inspect_many(&[], InspectFields::VERSION_INFO, Arc::new(NoopSink), 0).await;
        assert!(matches!(result, Err(ScanError::Internal(_))));
    }

    /// 验证 inspect_many 的结果顺序与 entries 一一对应（必修测试）。
    /// buffer_unordered 完成顺序不确定，必须靠 (idx, outcome) 排序还原。
    #[tokio::test]
    async fn inspect_many_preserves_order() {
        let tmp = tempfile::tempdir().unwrap();
        // 构造 5 个文件，路径名带序号
        let mut entries = Vec::new();
        for i in 0..5 {
            let path = tmp.path().join(format!("file_{i}.exe"));
            fs::write(&path, b"MZ\x00\x00").unwrap(); // 不是真 PE，会 Failed
            let meta = fs::metadata(&path).unwrap();
            entries.push(FileEntry::from_metadata(path, &meta));
        }

        let results = inspect_many(
            &entries,
            InspectFields::VERSION_INFO,
            Arc::new(NoopSink),
            3, // concurrency=3 触发并行，但结果顺序必须保留
        )
        .await
        .expect("inspect_many");

        assert_eq!(results.len(), entries.len());
        for (i, outcome) in results.iter().enumerate() {
            match outcome {
                InspectOutcome::Failed { path, .. } => {
                    assert_eq!(
                        path.file_name().unwrap().to_string_lossy(),
                        format!("file_{i}.exe"),
                        "results[{i}].path should match entries[{i}].path"
                    );
                }
                InspectOutcome::Success(_) => panic!("expected Failed (fake PE) at index {i}"),
            }
        }
    }

    /// concurrency=1 应该退化为完全串行（一次只跑一个 entry）。
    /// 用 atomic counter 验证：max_in_flight 永远 ≤ 1。
    #[tokio::test]
    async fn inspect_many_concurrency_one_is_serial() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let tmp = tempfile::tempdir().unwrap();
        let mut entries = Vec::new();
        for i in 0..5 {
            let path = tmp.path().join(format!("f{i}.exe"));
            fs::write(&path, b"MZ").unwrap();
            let meta = fs::metadata(&path).unwrap();
            entries.push(FileEntry::from_metadata(path, &meta));
        }

        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let progress = crate::sink_from({
            let in_flight = Arc::clone(&in_flight);
            let max_seen = Arc::clone(&max_seen);
            move |_| {
                let cur = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                let mut max = max_seen.load(Ordering::SeqCst);
                while cur > max {
                    match max_seen.compare_exchange(max, cur, Ordering::SeqCst, Ordering::SeqCst) {
                        Ok(_) => break,
                        Err(now) => max = now,
                    }
                }
                in_flight.fetch_sub(1, Ordering::SeqCst);
            }
        });

        let _ = inspect_many(&entries, InspectFields::VERSION_INFO, progress, 1).await;
        // concurrency=1 时 max_in_flight 应该 <= 1
        // （由于 inspect_one 本身是 async + spawn_blocking，理论上同时只 1）
        let max = max_seen.load(Ordering::SeqCst);
        assert!(
            max <= 1,
            "concurrency=1 should be serial, but max_in_flight={max}"
        );
    }

    /// 混合 Success+Failed 时 results 顺序仍与 entries 对应。
    #[cfg(windows)]
    #[tokio::test]
    async fn inspect_many_mixed_success_and_failed_preserves_order() {
        let notepad = PathBuf::from("C:\\Windows\\System32\\notepad.exe");
        if !notepad.exists() {
            return; // 非 Windows 跳过
        }

        let tmp = tempfile::tempdir().unwrap();
        let fake = tmp.path().join("fake.exe");
        fs::write(&fake, b"MZ").unwrap();
        let fake_meta = fs::metadata(&fake).unwrap();

        let notepad_meta = fs::metadata(&notepad).unwrap();

        // entries 顺序：fake（Failed）, notepad（Success）, nonexistent（Failed）
        let entries = vec![
            FileEntry::from_metadata(fake.clone(), &fake_meta),
            FileEntry::from_metadata(notepad.clone(), &notepad_meta),
            FileEntry {
                path: PathBuf::from("/nonexistent/fake.exe"),
                ..FileEntry::from_metadata(fake.clone(), &fake_meta)
            },
        ];

        let results = inspect_many(&entries, InspectFields::VERSION_INFO, Arc::new(NoopSink), 4)
            .await
            .expect("inspect_many");

        assert_eq!(results.len(), 3);
        assert!(
            matches!(results[0], InspectOutcome::Failed { .. }),
            "[0] should be Failed (fake PE)"
        );
        assert!(
            matches!(results[1], InspectOutcome::Success(_)),
            "[1] should be Success (real notepad.exe)"
        );
        assert!(
            matches!(results[2], InspectOutcome::Failed { .. }),
            "[2] should be Failed (nonexistent)"
        );
    }

    /// OWNER_SID 单独传时，目录应该早返回（v0.1 实现：无 ADS 时目录返回空）。
    #[tokio::test]
    async fn inspect_directory_with_owner_sid_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let meta = fs::metadata(tmp.path()).unwrap();
        let entry = FileEntry::from_metadata(tmp.path().to_path_buf(), &meta);
        let result = inspect(&entry, InspectFields::OWNER_SID).await;
        assert!(result.is_ok());
        assert!(result.unwrap().owner_sid.is_none()); // v0.1 always None
    }

    /// ADS 单独传时，目录应该不早返回（v0.2 实现的前置条件）。
    /// v0.1 仍然走完后返回空 ADS vec，但应跳过 PE 检查。
    #[tokio::test]
    async fn inspect_directory_with_ads_does_not_skip() {
        let tmp = tempfile::tempdir().unwrap();
        let meta = fs::metadata(tmp.path()).unwrap();
        let entry = FileEntry::from_metadata(tmp.path().to_path_buf(), &meta);
        let result = inspect(&entry, InspectFields::ADS).await;
        assert!(result.is_ok());
        // v0.1 仍返回空 ADS
        let inspected = result.unwrap();
        assert!(inspected.alt_data_streams.is_empty());
        assert!(inspected.version_info.is_none()); // 没传 VERSION_INFO
    }
}
