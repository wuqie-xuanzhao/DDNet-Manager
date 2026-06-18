//! 跨平台 fallback backend，基于 `ignore` crate 多线程递归扫描。
//!
//! v0.1 的全平台唯一可用 backend。Windows 上 admin/USN 路径不可用时也降级到这里。
//! 大磁盘优化：多线程并行 + 目录黑名单跳过 node_modules / .git / $RECYCLE.BIN 等
//! 几乎不可能含目标文件但巨大的目录，HDD/SSD 都显著加速。

use crate::backend::Backend;
use crate::error::ScanError;
use crate::options::{BackendKind, FileEntry, NtfsScanOptions, ProgressEvent, ScanLimitKind};
use crate::ProgressSink;
use async_trait::async_trait;
use ignore::WalkState;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// 保守目录黑名单：几乎不可能含目标文件但通常巨大的目录。
///
/// - `node_modules` / `.git` / `.hg` / `.svn`：版本控制和依赖，开发仓库常见
/// - `$RECYCLE.BIN` / `System Volume Information`：Windows 系统目录，无用户文件
/// - `Package Cache`：Windows Installer 缓存，含大量 .msi 不含目标可执行
///
/// 注意：只过滤"目录名完全匹配"的项；`Steam` / `Program Files` 等业务可能含
/// 目标的位置**不**过滤（让上层业务 roots 决定）。
const SKIP_DIR_NAMES: &[&str] = &[
    "node_modules",
    ".git",
    ".hg",
    ".svn",
    "$RECYCLE.BIN",
    "System Volume Information",
    "Package Cache",
];

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
            // 并行度：min(可用 CPU, 8)；HDD 上 IO 瓶颈，再多线程收益小；
            // SSD 上 4-8 线程能跑满带宽。
            let num_threads = std::thread::available_parallelism()
                .map(|n| n.get().clamp(1, 8))
                .unwrap_or(4);

            let walker = ignore::WalkBuilder::new(&root_owned)
                .follow_links(false)
                // 显式关掉 ignore crate 的默认过滤，避免误跳用户文件
                .hidden(false)
                .git_ignore(false)
                .git_exclude(false)
                .git_global(false)
                .parents(false)
                .ignore(false)
                .threads(num_threads)
                .filter_entry(|entry| {
                    // 只过滤目录；文件一律不跳（matcher 自己判断）
                    if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        return true;
                    }
                    let Some(name) = entry.file_name().to_str() else {
                        return true;
                    };
                    !SKIP_DIR_NAMES.contains(&name)
                })
                .build_parallel();

            let found: Arc<Mutex<Vec<FileEntry>>> = Arc::new(Mutex::new(Vec::new()));
            let scanned = Arc::new(AtomicUsize::new(0));
            let last_emit = Arc::new(Mutex::new(Instant::now()));

            walker.run(|| {
                let found = Arc::clone(&found);
                let scanned = Arc::clone(&scanned);
                let last_emit = Arc::clone(&last_emit);
                let matcher = Arc::clone(&matcher);
                let progress = Arc::clone(&progress);
                let cancel = cancel.clone();

                Box::new(move |result| {
                    if cancel.is_cancelled() {
                        return WalkState::Quit;
                    }

                    let n = scanned.fetch_add(1, Ordering::Relaxed) + 1;

                    let entry = match result {
                        Ok(e) => e,
                        Err(e) => {
                            progress.emit(ProgressEvent::EntryError {
                                path: None,
                                error: e.to_string(),
                            });
                            return WalkState::Continue;
                        }
                    };

                    let Some(name) = entry.file_name().to_str() else {
                        return WalkState::Continue;
                    };
                    if !matcher(name) {
                        if n >= max_records {
                            return WalkState::Quit;
                        }
                        return WalkState::Continue;
                    }

                    let path: PathBuf = entry.path().to_path_buf();
                    let meta = match entry.metadata() {
                        Ok(m) => m,
                        Err(e) => {
                            progress.emit(ProgressEvent::EntryError {
                                path: Some(path),
                                error: e.to_string(),
                            });
                            return WalkState::Continue;
                        }
                    };

                    let current_len = {
                        let mut guard = found.lock().expect("found mutex poisoned");
                        guard.push(FileEntry::from_metadata(path, &meta));
                        guard.len()
                    };

                    if let Some(max) = max_results {
                        if current_len >= max {
                            progress.emit(ProgressEvent::ScanLimitHit {
                                limit_kind: ScanLimitKind::Results,
                                limit: max,
                            });
                            return WalkState::Quit;
                        }
                    }

                    if n % 1000 == 0 {
                        let mut le = last_emit.lock().expect("last_emit mutex poisoned");
                        if le.elapsed() > std::time::Duration::from_millis(100) {
                            progress.emit(ProgressEvent::EntriesFound { found: current_len });
                            *le = Instant::now();
                        }
                    }

                    if n >= max_records {
                        progress.emit(ProgressEvent::ScanLimitHit {
                            limit_kind: ScanLimitKind::RecordsScanned,
                            limit: max_records,
                        });
                        return WalkState::Quit;
                    }

                    WalkState::Continue
                })
            });

            let mut guard = found.lock().expect("found mutex poisoned");
            Ok(std::mem::take(&mut *guard))
        })
        .await
        .map_err(|e| ScanError::Internal(format!("walkdir spawn_blocking join: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::NtfsScanOptions;
    use std::fs;

    async fn scan(backend: &WalkdirBackend, root: &Path, opts: &NtfsScanOptions) -> Vec<FileEntry> {
        backend
            .scan_root(root, opts, Arc::new(crate::NoopSink), CancellationToken::new())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn finds_matching_files_recursively() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("DDNet.exe"), b"x").unwrap();
        fs::write(tmp.path().join("other.txt"), b"y").unwrap();
        fs::create_dir_all(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub").join("DDNet.exe"), b"z").unwrap();

        let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("DDNet.exe"));
        let entries = scan(&WalkdirBackend, tmp.path(), &opts).await;
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn respects_max_results() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..10 {
            fs::write(tmp.path().join(format!("f{i}.exe")), b"x").unwrap();
        }
        let opts = NtfsScanOptions::new(|n| n.ends_with(".exe")).with_max_results(3);
        let entries = scan(&WalkdirBackend, tmp.path(), &opts).await;
        assert_eq!(entries.len(), 3);
    }

    #[tokio::test]
    async fn respects_max_records_scanned() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..100 {
            fs::write(tmp.path().join(format!("f{i}.txt")), b"x").unwrap();
        }
        let opts = NtfsScanOptions::new(|n| n.ends_with(".exe"))
            .with_max_records_scanned(10);
        let entries = scan(&WalkdirBackend, tmp.path(), &opts).await;
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn returns_empty_for_nonexistent_root() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("does_not_exist");
        let opts = NtfsScanOptions::new(|n| n.ends_with(".exe"));
        let entries = scan(&WalkdirBackend, &nonexistent, &opts).await;
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn cancel_stops_early() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..1000 {
            fs::write(tmp.path().join(format!("f{i}.txt")), b"x").unwrap();
        }
        let opts = NtfsScanOptions::new(|n| n.ends_with(".txt"));
        let cancel = CancellationToken::new();
        cancel.cancel();
        let entries = WalkdirBackend
            .scan_root(tmp.path(), &opts, Arc::new(crate::NoopSink), cancel)
            .await
            .unwrap();
        // cancel 后能立即返回，不会扫完所有 1000 个
        assert!(entries.len() <= 1000);
    }

    #[tokio::test]
    async fn skips_blacklisted_directories() {
        let tmp = tempfile::tempdir().unwrap();
        // 主目录有一个目标文件
        fs::write(tmp.path().join("DDNet.exe"), b"main").unwrap();
        // 黑名单目录也有目标文件，但应该被跳过
        fs::create_dir_all(tmp.path().join("node_modules").join("pkg")).unwrap();
        fs::write(
            tmp.path().join("node_modules").join("pkg").join("DDNet.exe"),
            b"node_modules",
        )
        .unwrap();
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        fs::write(tmp.path().join(".git").join("DDNet.exe"), b"git").unwrap();
        // 非黑名单子目录的目标文件应该被找到
        fs::create_dir_all(tmp.path().join("sub")).unwrap();
        fs::write(tmp.path().join("sub").join("DDNet.exe"), b"sub").unwrap();

        let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("DDNet.exe"));
        let entries = scan(&WalkdirBackend, tmp.path(), &opts).await;
        let paths: Vec<String> = entries
            .iter()
            .map(|e| e.path.to_string_lossy().replace('\\', "/"))
            .collect();
        assert_eq!(entries.len(), 2, "应找到主目录 + sub，跳过 node_modules / .git");
        assert!(paths.iter().any(|p| p.ends_with("/DDNet.exe")));
        assert!(paths.iter().any(|p| p.contains("/sub/DDNet.exe")));
        assert!(!paths.iter().any(|p| p.contains("node_modules")));
        assert!(!paths.iter().any(|p| p.contains("/.git/")));
    }
}
