//! Windows 平台 backend 子模块入口。
//!
//! 包含：
//! - `volume`：CreateFileW + DeviceIoControl 封装（只读访问）
//! - `usn`：FSCTL_ENUM_USN_DATA + USN_RECORD V2 解析
//! - `mft_record`：$MFT FILE record 字节解析（纯逻辑，跨平台测试）
//! - `mft`：$MFT raw record backend（admin 极速路径）
//!
//! 所有 Windows API 调用都在这里；模块外只暴露 safe Rust 接口。

pub(crate) mod mft;
pub(crate) mod mft_record;
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
use std::time::{Duration, SystemTime};
use tokio_util::sync::CancellationToken;

/// Windows FILETIME（自 1601-01-01 的 100ns 单位）转 SystemTime。
///
/// 由 mft.rs / usn.rs 共用，避免重复实现。
pub(crate) fn filetime_to_system_time(filetime: u64) -> SystemTime {
    if filetime == 0 {
        return SystemTime::UNIX_EPOCH;
    }
    const FILETIME_UNIX_OFFSET: u64 = 116_444_736_000_000_000; // 100ns 单位
    let unix_100ns = filetime.saturating_sub(FILETIME_UNIX_OFFSET);
    let secs = unix_100ns / 10_000_000;
    let nanos = ((unix_100ns % 10_000_000) * 100) as u32;
    SystemTime::UNIX_EPOCH + Duration::new(secs, nanos)
}

/// USN backend：尝试用 FSCTL_ENUM_USN_DATA 扫盘；如果 USN 不可用或扫描失败，
/// 自动降级到 WalkdirBackend（emit `BackendDowngraded`）。
///
/// **重要语义**：USN 扫的是**整个卷**，与传入 `root` 子树无关。为避免"调用方传
/// `C:\Windows` 但拿到整个 C 盘结果"的语义割裂，本 backend 只在 `root` 是整盘
/// 路径（`C:\` / `C:/` / `D:\` 等）时走 USN；否则**直接**走 Walkdir（不 emit
/// `BackendDowngraded`，因为不是降级，是调用方语义匹配的选择）。
pub(crate) struct UsnBackend {
    drive_letter: char,
}

impl UsnBackend {
    pub(crate) fn new(drive_letter: char) -> Self {
        Self { drive_letter }
    }
}

/// 判断 `root` 是否是整盘根路径（如 `C:\` / `c:/` / `D:\`）。
pub(super) fn is_whole_drive_root(root: &Path, expected_drive: char) -> bool {
    let Some(actual) = volume::path_to_drive_letter(root) else {
        return false;
    };
    if !actual.eq_ignore_ascii_case(&expected_drive) {
        return false;
    }
    // 去掉 `X:` 后，剩余必须为空或仅为分隔符（`\` 或 `/`）
    let s = root.to_str().unwrap_or("");
    let rest = s.get(2..).unwrap_or("");
    rest.is_empty() || rest.chars().all(|c| c == '\\' || c == '/')
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
        // 子树路径直接走 Walkdir（不是降级，是调用方语义匹配）
        if !is_whole_drive_root(root, self.drive_letter) {
            tracing::debug!(
                root = %root.display(),
                drive = %self.drive_letter,
                "root is a subtree, not whole drive; using walkdir directly"
            );
            return WalkdirBackend.scan_root(root, opts, progress, cancel).await;
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn whole_drive_root_matches_canonical() {
        assert!(is_whole_drive_root(&PathBuf::from("C:\\"), 'C'));
        assert!(is_whole_drive_root(&PathBuf::from("c:/"), 'C'));
        assert!(is_whole_drive_root(&PathBuf::from("D:\\"), 'D'));
    }

    #[test]
    fn whole_drive_root_rejects_subtree() {
        assert!(!is_whole_drive_root(&PathBuf::from("C:\\Windows"), 'C'));
        assert!(!is_whole_drive_root(&PathBuf::from("C:/Users"), 'C'));
        assert!(!is_whole_drive_root(&PathBuf::from("/usr"), 'C'));
    }

    #[test]
    fn whole_drive_root_rejects_wrong_drive() {
        assert!(!is_whole_drive_root(&PathBuf::from("D:\\"), 'C'));
    }
}
