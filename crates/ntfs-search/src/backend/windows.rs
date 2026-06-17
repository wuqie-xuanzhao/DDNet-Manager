//! Windows 平台 backend 子模块入口。
//!
//! 包含：
//! - `volume`：CreateFileW + DeviceIoControl 封装（只读访问）
//! - `usn`：FSCTL_ENUM_USN_DATA + USN_RECORD V2 解析
//!
//! 所有 Windows API 调用都在这里；模块外只暴露 safe Rust 接口。

pub(crate) mod usn;
pub(crate) mod volume;

use crate::backend::walkdir::WalkdirBackend;
use crate::backend::Backend;
use crate::error::ScanError;
use crate::options::{BackendKind, FileEntry, NtfsScanOptions, ProgressEvent};
use crate::ProgressSink;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// USN backend：尝试用 FSCTL_ENUM_USN_DATA 扫盘；如果 USN 不可用或扫描失败，
/// 自动降级到 WalkdirBackend（emit `BackendDowngraded`）。
pub(super) struct UsnBackend {
    drive_letter: char,
}

impl UsnBackend {
    pub(super) fn new(drive_letter: char) -> Self {
        Self { drive_letter }
    }
}

#[async_trait]
impl Backend for UsnBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Usn
    }

    async fn scan_root(
        &self,
        root: &Path,
        opts: &NtfsScanOptions,
        progress: Arc<dyn ProgressSink>,
        cancel: CancellationToken,
    ) -> Result<Vec<FileEntry>, ScanError> {
        // 尝试 USN
        match volume::VolumeHandle::open(self.drive_letter) {
            Ok(vol) => {
                if vol.query_usn_journal().is_err() {
                    return fallback_to_walkdir(
                        root,
                        opts,
                        progress,
                        cancel,
                        "volume does not support USN (FAT32/exFAT or disabled)",
                    )
                    .await;
                }
                match usn::scan(vol, opts, Arc::clone(&progress), cancel.clone()).await {
                    Ok(entries) => Ok(entries),
                    Err(e) => {
                        // USN 扫描失败 → 降级 Walkdir
                        fallback_to_walkdir(
                            root,
                            opts,
                            progress,
                            cancel,
                            &format!("USN scan failed: {e}"),
                        )
                        .await
                    }
                }
            }
            Err(e) => {
                fallback_to_walkdir(
                    root,
                    opts,
                    progress,
                    cancel,
                    &format!("open volume failed: {e}"),
                )
                .await
            }
        }
    }
}

/// 降级到 Walkdir。先 emit `BackendDowngraded`，再调 WalkdirBackend。
async fn fallback_to_walkdir(
    root: &Path,
    opts: &NtfsScanOptions,
    progress: Arc<dyn ProgressSink>,
    cancel: CancellationToken,
    reason: &str,
) -> Result<Vec<FileEntry>, ScanError> {
    progress.emit(ProgressEvent::BackendDowngraded {
        root: root.to_path_buf(),
        from: BackendKind::Usn,
        to: BackendKind::Walkdir,
        reason: reason.to_string(),
    });
    WalkdirBackend.scan_root(root, opts, progress, cancel).await
}
