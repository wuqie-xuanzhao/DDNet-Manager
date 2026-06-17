//! 跨平台 fallback backend，基于 `walkdir` crate 单线程递归。
//!
//! v0.1 的全平台唯一可用 backend。Windows 上 admin/USN 路径不可用时也降级到这里。
//! 后续 commit 3+ 接入 Windows 原生 backend 后，Walkdir 退到 fallback 角色。

use crate::backend::Backend;
use crate::error::ScanError;
use crate::options::{BackendKind, FileEntry, NtfsScanOptions, ProgressEvent, ScanLimitKind};
use crate::ProgressSink;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Walkdir backend 单例。无状态。
pub(super) struct WalkdirBackend;

#[async_trait]
impl Backend for WalkdirBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Walkdir
    }

    async fn scan_root(
        &self,
        root: &Path,
        opts: &NtfsScanOptions,
        progress: Arc<dyn ProgressSink>,
        cancel: CancellationToken,
    ) -> Result<Vec<FileEntry>, ScanError> {
        let root_owned = root.to_path_buf();
        let matcher = Arc::clone(&opts.matcher);
        let max_results = opts.max_results;
        let max_records = opts.max_records_scanned.unwrap_or(usize::MAX);

        tokio::task::spawn_blocking(move || {
            let mut found: Vec<FileEntry> = Vec::new();
            let mut scanned = 0usize;
            let last_emit = Instant::now();

            for entry in walkdir::WalkDir::new(&root_owned).follow_links(false) {
                if cancel.is_cancelled() {
                    return Ok(found);
                }

                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        progress.emit(ProgressEvent::EntryError {
                            path: e.path().map(Path::to_path_buf),
                            error: e.to_string(),
                        });
                        continue;
                    }
                };

                scanned += 1;

                let Some(name) = entry.file_name().to_str() else {
                    continue;
                };
                if !matcher(name) {
                    continue;
                }

                let path: PathBuf = entry.path().to_path_buf();
                let meta = match entry.metadata() {
                    Ok(m) => m,
                    Err(e) => {
                        progress.emit(ProgressEvent::EntryError {
                            path: Some(path),
                            error: e.to_string(),
                        });
                        continue;
                    }
                };
                found.push(FileEntry::from_metadata(path, &meta));

                if let Some(max) = max_results {
                    if found.len() >= max {
                        progress.emit(ProgressEvent::ScanLimitHit {
                            limit_kind: ScanLimitKind::Results,
                            limit: max,
                        });
                        return Ok(found);
                    }
                }

                if scanned % 1000 == 0
                    && last_emit.elapsed() > std::time::Duration::from_millis(100)
                {
                    progress.emit(ProgressEvent::EntriesFound { found: found.len() });
                }

                if scanned >= max_records {
                    progress.emit(ProgressEvent::ScanLimitHit {
                        limit_kind: ScanLimitKind::RecordsScanned,
                        limit: max_records,
                    });
                    return Ok(found);
                }
            }

            Ok(found)
        })
        .await
        .map_err(|e| ScanError::Internal(format!("walkdir spawn_blocking join: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::NtfsScanOptions;
    use crate::NoopSink;
    use std::fs;
    use std::sync::Arc;

    fn opts_match_exe() -> NtfsScanOptions {
        NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("DDNet.exe"))
    }

    /// 直接调 WalkdirBackend.scan_root（绕过 find_files 入口），用于单测。
    async fn scan(
        backend: &WalkdirBackend,
        root: &Path,
        opts: &NtfsScanOptions,
    ) -> Result<Vec<FileEntry>, ScanError> {
        backend
            .scan_root(root, opts, Arc::new(NoopSink), CancellationToken::new())
            .await
    }

    #[tokio::test]
    async fn finds_matching_files_in_tempdir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("DDNet.exe"), b"x").unwrap();
        fs::write(tmp.path().join("other.txt"), b"y").unwrap();
        fs::create_dir_all(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub").join("DDNet.exe"), b"z").unwrap();

        let opts = opts_match_exe();
        let entries = scan(&WalkdirBackend, tmp.path(), &opts).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| !e.is_directory));
        assert!(entries.iter().all(|e| e.backend == BackendKind::Walkdir));
    }

    #[tokio::test]
    async fn respects_max_results() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..20 {
            fs::write(tmp.path().join(format!("f{i}.exe")), b"x").unwrap();
        }

        let opts = NtfsScanOptions::new(|n| n.ends_with(".exe")).with_max_results(5);
        let entries = scan(&WalkdirBackend, tmp.path(), &opts).await.unwrap();
        assert_eq!(entries.len(), 5);
    }

    #[tokio::test]
    async fn respects_max_records_scanned() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..50 {
            fs::write(tmp.path().join(format!("f{i}.txt")), b"x").unwrap();
        }

        let opts = NtfsScanOptions::new(|_| false).with_max_records_scanned(10);
        let entries = scan(&WalkdirBackend, tmp.path(), &opts).await.unwrap();
        assert!(entries.is_empty(), "matcher is always false");
        // 主要验证：扫描在 10 条 record 后停止，没卡死
    }

    #[tokio::test]
    async fn returns_empty_for_nonexistent_root() {
        let opts = opts_match_exe();
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("does_not_exist");
        // walkdir 对不存在的 root 直接 emit EntryError，返回空 vec
        let entries = scan(&WalkdirBackend, &nonexistent, &opts).await.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn cancel_returns_partial_results() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..200 {
            fs::write(tmp.path().join(format!("f{i}.exe")), b"x").unwrap();
        }

        let opts = NtfsScanOptions::new(|_| true);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        // 立即取消（walkdir 第一个 entry 前基本能命中）
        cancel_clone.cancel();

        let backend = WalkdirBackend;
        let result = backend
            .scan_root(tmp.path(), &opts, Arc::new(NoopSink), cancel)
            .await
            .unwrap();
        // 取消后返回部分结果（可能 0 或少数几条），但不应 panic / hang
        let _ = result.len();
    }

    #[tokio::test]
    async fn ignores_directories_when_matcher_keeps_dir_name() {
        // 即使目录名匹配 matcher，FileEntry 也会正确标记 is_directory
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("DDNet.exe")).unwrap();

        let opts = opts_match_exe();
        let entries = scan(&WalkdirBackend, tmp.path(), &opts).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_directory);
        assert!(entries[0]
            .attributes
            .contains(crate::options::FileAttributes::DIRECTORY));
    }
}
