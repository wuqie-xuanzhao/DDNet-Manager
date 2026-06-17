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
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Semaphore;

type InspectFuture = Pin<Box<dyn Future<Output = Result<InspectOutcome, ScanError>> + Send>>;

/// 单条 entry 扩展查询。返回 `InspectedEntry`（含调用方请求的所有 fields）。
///
/// 失败时返回 `Err(ScanError)`，调用方决定如何处理。
pub async fn inspect(
    entry: &FileEntry,
    fields: InspectFields,
) -> Result<InspectedEntry, ScanError> {
    inspect_one(entry, fields).await
}

/// 批量扩展查询。内置 `Semaphore` 并发控制（默认 16，调用方可调）。
///
/// 返回 `Vec<InspectOutcome>`，与 `entries` 顺序对应。单条失败不阻断其他，
/// 按 `InspectOutcome::Failed` 标记。
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
    let mut futures: Vec<InspectFuture> = Vec::with_capacity(entries.len());

    for entry in entries {
        let permit_sem = Arc::clone(&semaphore);
        let entry = entry.clone();
        let progress = Arc::clone(&progress);

        futures.push(Box::pin(async move {
            // 等待 permit（spawn_blocking 期间持有 permit 控制并发）
            let _permit = permit_sem
                .acquire()
                .await
                .map_err(|e| ScanError::Internal(format!("semaphore acquire: {e}")))?;

            match inspect_one(&entry, fields).await {
                Ok(info) => Ok(InspectOutcome::Success(info)),
                Err(e) => {
                    progress.emit(crate::options::ProgressEvent::EntryError {
                        path: Some(entry.path.clone()),
                        error: e.to_string(),
                    });
                    Ok(InspectOutcome::Failed {
                        path: entry.path.clone(),
                        error: e,
                    })
                }
            }
        }) as InspectFuture);
    }

    let mut results = Vec::with_capacity(futures.len());
    for fut in futures {
        let outcome = fut.await?;
        results.push(outcome);
    }
    Ok(results)
}

/// 单条扩展查询的实际实现。spawn_blocking 内调 pe::read_version_info。
///
/// OWNER_SID / ADS 在 v0.1 暂未实现（NTFS $SDS 解析 + ADS 枚举复杂），
/// 返回 None；调用方按 fields 检查。
async fn inspect_one(
    entry: &FileEntry,
    fields: InspectFields,
) -> Result<InspectedEntry, ScanError> {
    if entry.is_directory {
        // 目录无 PE 资源 / 文件 size，跳过
        return Ok(InspectedEntry::default());
    }

    let path = entry.path.clone();
    let need_version = fields.contains(InspectFields::VERSION_INFO);

    let version_info = if need_version {
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
}
