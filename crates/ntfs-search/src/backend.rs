//! Backend 抽象层。trait Backend 定义扫描某个 root 的契约，
//! 各平台/权限路径各自实现（Walkdir 跨平台 / Windows MFT / Windows USN）。
//!
//! 本模块的 trait 与各 backend 实现都是 `pub(super)`——不暴露给调用方。
//! 调用方只通过 `crate::find_files` 入口与 crate 交互。

use crate::error::ScanError;
use crate::options::{BackendKind, FileEntry, NtfsScanOptions, ProgressEvent};
use crate::ProgressSink;
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

mod walkdir;

#[cfg(windows)]
mod windows;

/// 扫描某个 root 的 backend 契约。
#[async_trait]
pub(super) trait Backend: Send + Sync {
    fn kind(&self) -> BackendKind;

    async fn scan_root(
        &self,
        root: &Path,
        opts: &NtfsScanOptions,
        progress: Arc<dyn ProgressSink>,
        cancel: CancellationToken,
    ) -> Result<Vec<FileEntry>, ScanError>;
}

/// 为指定 root 探测可用的 backend（按 Mft → Usn → Walkdir 链降级）。
///
/// - Windows 上 v0.2：尝试 USN backend（普通用户路径）。admin $MFT 路径留 M3 实现。
/// - 其他平台：直接 Walkdir。
pub(super) fn select_backend_for_root(root: &Path) -> SelectedBackend {
    let requested = if cfg!(windows) {
        probe_windows_backend_kind(root)
    } else {
        BackendKind::Walkdir
    };

    let backend: Box<dyn Backend> = match requested {
        #[cfg(windows)]
        BackendKind::Mft => {
            // admin 路径：$MFT raw record backend。失败自动 fallback USN → Walkdir。
            let drive = windows::volume::path_to_drive_letter(root).unwrap_or('C');
            Box::new(windows::mft::MftBackend::new(drive))
        }
        #[cfg(windows)]
        BackendKind::Usn => {
            let drive = windows::volume::path_to_drive_letter(root).unwrap_or('C');
            Box::new(windows::UsnBackend::new(drive))
        }
        #[cfg(not(windows))]
        BackendKind::Mft | BackendKind::Usn => Box::new(walkdir::WalkdirBackend),
        BackendKind::Walkdir => Box::new(walkdir::WalkdirBackend),
    };

    let actual_kind = backend.kind();

    SelectedBackend {
        kind: actual_kind,
        backend,
        downgraded_from: if actual_kind == requested {
            None
        } else {
            Some(requested)
        },
    }
}

/// 选定 backend 的结果。`downgraded_from = Some(higher)` 表示被降级。
pub(super) struct SelectedBackend {
    pub kind: BackendKind,
    pub backend: Box<dyn Backend>,
    pub downgraded_from: Option<BackendKind>,
}

/// Windows 平台 backend 探测：v0.3 默认走 $MFT（admin 路径），admin 不可用时
/// MftBackend 内部自动 fallback USN → Walkdir。
#[cfg(windows)]
fn probe_windows_backend_kind(_root: &Path) -> BackendKind {
    BackendKind::Mft
}

#[cfg(not(windows))]
fn probe_windows_backend_kind(_root: &Path) -> BackendKind {
    BackendKind::Walkdir
}

/// 给定一组 roots 与 backend 选择函数，扫描后聚合结果。
/// 任一盘失败不阻断其他盘；全部失败才返回 `NoBackendAvailable`。
pub(super) async fn scan_all_roots(
    roots: Vec<PathBuf>,
    opts: NtfsScanOptions,
    progress: Arc<dyn ProgressSink>,
    cancel: CancellationToken,
) -> Result<Vec<FileEntry>, ScanError> {
    let selected: Vec<(PathBuf, SelectedBackend)> = roots
        .into_iter()
        .map(|root| {
            let sel = select_backend_for_root(&root);
            (root, sel)
        })
        .collect();

    // v0.1：串行扫描各盘，让 cancel/timeout 逻辑清晰可见。
    // commit 3+ 接入 MFT 后会重写为 spawn_blocking 并行（每盘一个阻塞线程）。
    let mut all: Vec<FileEntry> = Vec::new();
    let mut all_skipped: Vec<(PathBuf, ScanError)> = Vec::new();
    let total_scanned: Mutex<Vec<(PathBuf, usize, usize)>> = Mutex::new(Vec::new());

    for (root, selected_backend) in selected {
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }

        if let Some(from) = selected_backend.downgraded_from {
            progress.emit(ProgressEvent::BackendDowngraded {
                root: root.clone(),
                from,
                to: selected_backend.kind,
                reason: "v0.1: backend not yet implemented".to_string(),
            });
        }

        progress.emit(ProgressEvent::DriveStarted {
            root: root.clone(),
            backend: selected_backend.kind,
        });

        let root_for_track = root.clone();
        let result = selected_backend
            .backend
            .scan_root(&root, &opts, Arc::clone(&progress), cancel.clone())
            .await;

        match result {
            Ok(entries) => {
                let found = entries.len();
                all.extend(entries);
                progress.emit(ProgressEvent::DriveCompleted {
                    root: root_for_track.clone(),
                    scanned: found, // walkdir 不区分 scanned/found；commit 4 MFT 后会精确区分
                    found,
                });
                total_scanned
                    .lock()
                    .expect("total_scanned mutex poisoned")
                    .push((root_for_track, found, found));
            }
            Err(ScanError::Cancelled) => {
                return Err(ScanError::Cancelled);
            }
            Err(e) => {
                all_skipped.push((root_for_track, e));
            }
        }
    }

    if all.is_empty() && !all_skipped.is_empty() {
        return Err(ScanError::NoBackendAvailable {
            root: "all".to_string(),
        });
    }

    for (root, e) in all_skipped {
        progress.emit(ProgressEvent::DriveSkipped {
            root,
            reasons: vec![e.to_string()],
        });
    }

    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_backend_returns_mft_on_windows() {
        let sel = select_backend_for_root(Path::new("C:\\"));
        #[cfg(windows)]
        {
            assert_eq!(sel.kind, BackendKind::Mft);
        }
        #[cfg(not(windows))]
        {
            assert_eq!(sel.kind, BackendKind::Walkdir);
            assert!(sel.downgraded_from.is_none());
        }
    }
}
