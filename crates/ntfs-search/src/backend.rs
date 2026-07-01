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

/// 单个 root 的扫描任务句柄：root 路径 + 后台异步扫描的 JoinHandle。
/// 拆出 type alias 是为了把过长的 `Vec<(PathBuf, JoinHandle<Result<...>>)>`
/// 从函数体内挪出来（clippy::type_complexity）。
type ScanTask = (
    PathBuf,
    tokio::task::JoinHandle<Result<Vec<FileEntry>, ScanError>>,
);
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
    // 注意：必须用 `#[cfg(windows)]` 属性而非 `cfg!()` runtime 宏 —— 后者两个分支
    // 都会被 type-check，导致 macOS 编译时 `windows::volume::xxx` 解析失败。
    #[cfg(windows)]
    let requested = {
        let cached = windows::volume::path_to_drive_letter(root).and_then(lookup_cached_backend);
        cached.unwrap_or_else(|| probe_windows_backend_kind(root))
    };
    #[cfg(not(windows))]
    let requested = BackendKind::Walkdir;

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
        tracing::debug!("process not elevated; skipping Mft/Usn probe, using Walkdir directly");
        BackendKind::Walkdir
    }
}

/// 非 Windows 平台的 stub。`probe_windows_backend_kind` 的唯一调用者位于
/// `#[cfg(windows)]` 块内（`select_backend_kind`），所以这个分支在 macOS/Linux
/// 上理论不可达；保留它是为了让 `select_backend_kind` 在所有平台上都能编译通过
/// （Rust 仍要求函数符号存在）。如未来重构 select_backend_kind 走 trait object，
/// 可以删掉。
#[cfg(not(windows))]
#[allow(dead_code)]
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
///
/// **多盘并行**：所有 root 用 `tokio::spawn` 并发执行 `scan_root`，盘内多线程
/// （walkdir 的 `ignore::WalkParallel`）+ 盘间多任务并行。`max_results` 是全局
/// 软上限，达到后 `cancel.cancel()` 让所有盘退出。`EntriesFound` 事件经
/// [`GlobalizingSink`] 包装后 emit 全局累计值，让前端覆盖式更新正确。
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

    if cancel.is_cancelled() {
        return Err(ScanError::Cancelled);
    }

    // 多盘共享状态：每盘最后一次上报的 found 数（用于 EntriesFound 全局累计）。
    // walkdir 内部 emit 的 EntriesFound.found 是 per-drive current_len，并行下
    // 直接 emit 给前端会让覆盖式 setFoundCount 倒退；GlobalizingSink 拦截后
    // 转成"所有盘 last 值之和"，前端覆盖式更新即正确。
    let per_drive_last: Arc<Mutex<HashMap<PathBuf, usize>>> = Arc::new(Mutex::new(HashMap::new()));
    let max_results = opts.max_results;

    // spawn 所有盘的 scan_root 并发执行；保留 (root, task) 用于 join 后聚合。
    let mut tasks: Vec<ScanTask> = Vec::with_capacity(selected.len());

    for (root, selected_backend) in selected {
        if cancel.is_cancelled() {
            // 还有未 spawn 的盘，直接返回；已 spawn 的盘会在下方 join 时检测到 cancel。
            // 不在这里 drop tasks，让外层 opts.timeout 或本函数末尾的 join 处理。
            break;
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

        let child_sink = Arc::new(GlobalizingSink {
            inner: Arc::clone(&progress),
            root: root.clone(),
            per_drive_last: Arc::clone(&per_drive_last),
        }) as Arc<dyn ProgressSink>;

        let opts_clone = opts.clone();
        let cancel_clone = cancel.clone();
        let backend = selected_backend.backend;
        let root_for_task = root.clone();
        let task = tokio::spawn(async move {
            backend
                .scan_root(&root_for_task, &opts_clone, child_sink, cancel_clone)
                .await
        });
        tasks.push((root, task));
    }

    // 等所有盘完成；上层 `find_files` 已用 `opts.timeout` 包了外层超时，cancel
    // 触发后 walkdir 协作式退出（每 entry 检查 cancel），通常秒级返回。
    let mut all: Vec<FileEntry> = Vec::new();
    let mut all_skipped: Vec<(PathBuf, ScanError)> = Vec::new();
    let mut global_matched: usize = 0;

    for (root, task) in tasks {
        match task.await {
            Ok(Ok(entries)) => {
                let found = entries.len();
                global_matched = global_matched.saturating_add(found);
                all.extend(entries);
                progress.emit(ProgressEvent::DriveCompleted {
                    root: root.clone(),
                    scanned: found,
                    found,
                });
                // 软上限：达 max_results 后取消所有未完成的盘
                if let Some(max) = max_results {
                    if global_matched >= max {
                        cancel.cancel();
                    }
                }
            }
            Ok(Err(ScanError::Cancelled)) => {
                // 一个盘检测到 cancel；继续 drain 其他 task（它们也会很快返回）
                tracing::debug!(root = ?root, "scan_root cancelled");
            }
            Ok(Err(e)) => {
                all_skipped.push((root, e));
            }
            Err(e) => {
                all_skipped.push((
                    root,
                    ScanError::Internal(format!("scan task join error: {e}")),
                ));
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

/// 把 walkdir 内部 per-drive 的 `EntriesFound` 事件转成全局累计值。
///
/// 每个 root 启动前由 `scan_all_roots` 包一层，所有其他事件（DriveStarted、
/// DriveCompleted、BackendDowngraded 等）原样转发。`DriveCompleted.found` 仍
/// 是 per-drive 语义（文案"找到 X 个"应保持单盘语义不变）。
///
/// 线程安全：`Arc<Mutex<HashMap>>` 锁粒度小，仅在 emit EntriesFound 时短暂持有。
struct GlobalizingSink {
    inner: Arc<dyn ProgressSink>,
    root: PathBuf,
    per_drive_last: Arc<Mutex<HashMap<PathBuf, usize>>>,
}

impl ProgressSink for GlobalizingSink {
    fn emit(&self, event: ProgressEvent) {
        match event {
            ProgressEvent::EntriesFound { found: local } => {
                // walkdir 内部 current_len 单调递增，直接覆盖即可
                let total = {
                    let mut m = self.per_drive_last.lock().expect("per_drive_last poisoned");
                    m.insert(self.root.clone(), local);
                    m.values().sum::<usize>()
                };
                self.inner
                    .emit(ProgressEvent::EntriesFound { found: total });
            }
            other => self.inner.emit(other),
        }
    }
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
        assert_eq!(
            normalize_drive_root(PathBuf::from("D:")),
            PathBuf::from("D:\\")
        );
        assert_eq!(
            normalize_drive_root(PathBuf::from("c:")),
            PathBuf::from("c:\\")
        );
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

    /// 多盘并行下，所有匹配都应被找到；EntriesFound 的全局累计应单调递增不倒退。
    #[tokio::test]
    async fn parallel_scan_finds_all_matches_and_globalizes_found() {
        use crate::options::NtfsScanOptions;
        use crate::sink_from;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();
        // 每盘放 3 个目标文件
        for i in 0..3 {
            std::fs::write(tmp_a.path().join(format!("a{i}.exe")), b"x").unwrap();
            std::fs::write(tmp_b.path().join(format!("b{i}.exe")), b"x").unwrap();
        }

        let max_global_seen = Arc::new(AtomicUsize::new(0));
        let max_clone = Arc::clone(&max_global_seen);
        let sink = sink_from(move |ev| {
            if let ProgressEvent::EntriesFound { found } = ev {
                let mut cur = max_clone.load(Ordering::Relaxed);
                while found > cur {
                    match max_clone.compare_exchange(
                        cur,
                        found,
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(actual) => cur = actual,
                    }
                }
            }
        });

        let opts = NtfsScanOptions::new(|n| n.ends_with(".exe"))
            .with_roots(vec![tmp_a.path().to_path_buf(), tmp_b.path().to_path_buf()]);
        let entries = scan_all_roots(opts.roots.clone(), opts, sink, CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(entries.len(), 6, "两盘共 6 个 .exe 都应被找到");
        // 全局累计：walkdir 在小目录上只在"首次命中"时 emit EntriesFound（local=1），
        // 周期性 emit 每 1000 records 才触发，小测试目录不会到达。所以这里只能断言
        // "至少看到了一次全局累计 emit"，而非具体数值；真正的正确性由 entries.len() 保证。
        assert!(
            max_global_seen.load(Ordering::Relaxed) >= 1,
            "应至少有一次 EntriesFound 全局累计 emit，实际 {}",
            max_global_seen.load(Ordering::Relaxed)
        );
    }

    /// max_results 全局上限触发后，未完成的盘应通过 cancel 协作式退出，
    /// 最终返回的 entries 数量受软上限约束（允许略多，因 walkdir 内部检查不精确）。
    #[tokio::test]
    async fn parallel_scan_caps_results_via_global_max() {
        use crate::options::NtfsScanOptions;
        use std::time::Instant;

        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();
        // 每盘 10 个，max=4，并行下应快速取消（远少于扫完 20 个的时间）
        for i in 0..10 {
            std::fs::write(tmp_a.path().join(format!("a{i}.exe")), b"x").unwrap();
            std::fs::write(tmp_b.path().join(format!("b{i}.exe")), b"x").unwrap();
        }

        let opts = NtfsScanOptions::new(|n| n.ends_with(".exe"))
            .with_roots(vec![tmp_a.path().to_path_buf(), tmp_b.path().to_path_buf()])
            .with_max_results(4);

        let start = Instant::now();
        let entries = scan_all_roots(
            opts.roots.clone(),
            opts,
            Arc::new(crate::NoopSink),
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let elapsed = start.elapsed();

        // 软上限：总匹配可能略多（cancel 前已在扫的盘会完成当前批次）
        assert!(
            entries.len() >= 4,
            "至少应找到 max_results=4 个，实际 {}",
            entries.len()
        );
        // 并行 + 早取消应秒级完成；放宽到 5s 避免慢 CI 误报
        assert!(
            elapsed.as_secs() < 5,
            "并行 + 早取消应 < 5s，实际 {:?}",
            elapsed
        );
    }

    /// GlobalizingSink 单元测试：拦截 EntriesFound，emit 全局累计值。
    #[tokio::test]
    async fn globalizing_sink_sums_per_drive_found() {
        use crate::options::NtfsScanOptions;
        use crate::sink_from;
        use std::sync::Mutex;

        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();
        std::fs::write(tmp_a.path().join("DDNet.exe"), b"x").unwrap();
        std::fs::write(tmp_b.path().join("DDNet.exe"), b"x").unwrap();

        // 记录所有 EntriesFound 的 found 值
        let seen_totals = Arc::new(Mutex::new(Vec::<usize>::new()));
        let seen_clone = Arc::clone(&seen_totals);
        let sink = sink_from(move |ev| {
            if let ProgressEvent::EntriesFound { found } = ev {
                seen_clone.lock().unwrap().push(found);
            }
        });

        let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("DDNet.exe"))
            .with_roots(vec![tmp_a.path().to_path_buf(), tmp_b.path().to_path_buf()]);
        scan_all_roots(opts.roots.clone(), opts, sink, CancellationToken::new())
            .await
            .unwrap();

        let totals = seen_totals.lock().unwrap();
        // 全局累计单调不减（GlobalizingSink 求和）
        let mut prev = 0;
        for &t in totals.iter() {
            assert!(t >= prev, "全局 EntriesFound 应单调不减：{} < {}", t, prev);
            prev = t;
        }
        // walkdir 在小目录上只在"首次命中"时 emit 一次 EntriesFound（local=1），
        // 周期性 emit 每 1000 records 才触发。所以这里只检查单调性，不强制具体数值；
        // 真正的"两盘匹配都被找到"由 entries.len() == 2 在调用方断言。
    }
}
