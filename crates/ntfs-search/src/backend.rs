//! Backend 抽象层。trait Backend 定义扫描某个 root 的契约，
//! 各平台/权限路径各自实现（Walkdir 跨平台 / Windows MFT / Windows USN）。
//!
//! 本模块的 trait 与各 backend 实现都是 `pub(super)`——不暴露给调用方。
//! 调用方只通过 `crate::find_files` 入口与 crate 交互。

use crate::error::ScanError;
use crate::options::{BackendKind, FileEntry, NtfsScanOptions, ProgressEvent};
use crate::ProgressSink;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tokio_util::sync::CancellationToken;

/// Per-drive backend 选型缓存。
///
/// 第一次扫描发现某盘 Mft/Usn 都不可用时记录"该盘用 Walkdir"，下次扫描同盘
/// 直接走 Walkdir，跳过无效探测。key 是 drive letter（小写），value 是实际
/// 能跑通的 backend kind。子模块（windows/fallback_to_walkdir）写入；
/// `select_backend_for_root` 读取。
static BACKEND_FALLBACK_CACHE: OnceLock<Mutex<HashMap<char, BackendKind>>> = OnceLock::new();

fn backend_fallback_cache() -> &'static Mutex<HashMap<char, BackendKind>> {
    BACKEND_FALLBACK_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 子模块记录"该 drive 实际能跑通的 backend"，下次 select 直接命中。
pub(super) fn remember_backend_for_drive(drive: char, kind: BackendKind) {
    if let Ok(mut cache) = backend_fallback_cache().lock() {
        cache.insert(drive.to_ascii_lowercase(), kind);
    }
}

/// 查询某 drive 的缓存 backend。未扫描过返回 None。
pub(super) fn lookup_cached_backend(drive: char) -> Option<BackendKind> {
    backend_fallback_cache()
        .lock()
        .ok()?
        .get(&drive.to_ascii_lowercase())
        .copied()
}

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
/// - Windows 上：elevated 进程走 Mft，普通进程跳过 Mft/Usn 直接 Walkdir
///   （per-drive cache 命中时进一步跳过 probe）
/// - 其他平台：直接 Walkdir。
pub(super) fn select_backend_for_root(root: &Path) -> SelectedBackend {
    // 优先查 per-drive 缓存：上次扫描发现该盘走 Walkdir 的，直接命中跳过探测。
    let requested = if cfg!(windows) {
        let cached = windows::volume::path_to_drive_letter(root)
            .and_then(lookup_cached_backend);
        cached.unwrap_or_else(|| probe_windows_backend_kind(root))
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

/// Windows 平台 backend 探测：admin（UAC elevated）走 Mft 极速路径，普通用户
/// 直接走 Walkdir，避免 Mft/Usn 无效探测（两个都需要 raw volume 句柄权限）。
#[cfg(windows)]
fn probe_windows_backend_kind(_root: &Path) -> BackendKind {
    if windows::elevation::is_process_elevated() {
        BackendKind::Mft
    } else {
        tracing::debug!(
            "process not elevated; skipping Mft/Usn probe, using Walkdir directly"
        );
        BackendKind::Walkdir
    }
}

#[cfg(not(windows))]
fn probe_windows_backend_kind(_root: &Path) -> BackendKind {
    BackendKind::Walkdir
}

/// 把裸盘符 `D:` 规范化为 `D:\`，避免 walkdir 在 Windows 上拼出 `D:Steam\...`。
///
/// Windows 路径语义：`D:` 是"D 盘当前目录"（依赖进程的 CDS），`D:\` 才是"D 盘根"。
/// 其他形式（`D:\` / `D:/` / `C:/Users`）原样返回。
fn normalize_drive_root(root: PathBuf) -> PathBuf {
    let Some(s) = root.to_str() else {
        return root;
    };
    let bytes = s.as_bytes();
    if bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        PathBuf::from(format!("{}\\", s))
    } else {
        root
    }
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
            // Windows 上 `D:` 表示"D 盘当前目录"，walkdir 会拼出 `D:Steam\...`
            // 这种缺分隔符的路径；统一补成 `D:\`。
            let root = normalize_drive_root(root);
            let sel = select_backend_for_root(&root);
            (root, sel)
        })
        .collect();

    // 串行扫描各盘，让 cancel/timeout 逻辑清晰可见。各盘后端在自己的 spawn_blocking
    // 内并行（walkdir 用 ignore::WalkParallel 多线程），盘间串行避免 IO 抢占。
    let mut all: Vec<FileEntry> = Vec::new();
    let mut all_skipped: Vec<(PathBuf, ScanError)> = Vec::new();

    for (root, selected_backend) in selected {
        if cancel.is_cancelled() {
            return Err(ScanError::Cancelled);
        }

        if let Some(from) = selected_backend.downgraded_from {
            progress.emit(ProgressEvent::BackendDowngraded {
                root: root.clone(),
                from,
                to: selected_backend.kind,
                reason: "backend not available".to_string(),
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
                    scanned: found, // walkdir 不区分 scanned/found；MFT backend 区分
                    found,
                });
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
            // elevated 进程走 Mft；普通进程跳过 Mft/Usn 直接 Walkdir
            if windows::elevation::is_process_elevated() {
                assert_eq!(sel.kind, BackendKind::Mft);
            } else {
                assert_eq!(sel.kind, BackendKind::Walkdir);
                assert!(sel.downgraded_from.is_none());
            }
        }
        #[cfg(not(windows))]
        {
            assert_eq!(sel.kind, BackendKind::Walkdir);
            assert!(sel.downgraded_from.is_none());
        }
    }

    #[test]
    fn normalize_drive_root_appends_separator_for_bare_drive_letter() {
        assert_eq!(normalize_drive_root(PathBuf::from("D:")), PathBuf::from("D:\\"));
        assert_eq!(normalize_drive_root(PathBuf::from("c:")), PathBuf::from("c:\\"));
    }

    #[test]
    fn normalize_drive_root_passes_through_already_rooted() {
        assert_eq!(
            normalize_drive_root(PathBuf::from("D:\\")),
            PathBuf::from("D:\\")
        );
        assert_eq!(
            normalize_drive_root(PathBuf::from("D:/")),
            PathBuf::from("D:/")
        );
    }

    #[test]
    fn normalize_drive_root_passes_through_subtree_paths() {
        assert_eq!(
            normalize_drive_root(PathBuf::from("C:/Users")),
            PathBuf::from("C:/Users")
        );
        assert_eq!(
            normalize_drive_root(PathBuf::from(r"D:\Steam")),
            PathBuf::from(r"D:\Steam")
        );
    }

    #[test]
    fn backend_cache_roundtrip_lowercases_drive_letter() {
        // 清空：用小写 d 写入，大写 D 查询也能命中（内部统一小写）
        remember_backend_for_drive('D', BackendKind::Walkdir);
        assert_eq!(lookup_cached_backend('D'), Some(BackendKind::Walkdir));
        assert_eq!(lookup_cached_backend('d'), Some(BackendKind::Walkdir));
        assert_eq!(lookup_cached_backend('E'), None);
    }
}
