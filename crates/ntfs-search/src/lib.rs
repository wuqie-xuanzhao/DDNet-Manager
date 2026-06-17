//! ntfs-search —— 跨平台文件搜索，Windows 上读 NTFS $MFT / USN，其他平台走 walkdir。
//!
//! ## 设计原则
//!
//! - **只读**：所有句柄 `GENERIC_READ`，与 Listary / Everything / chkdsk 完美共存。
//! - **跨平台统一 API**：`find_files` 单一入口，crate 自动按平台/权限选 backend。
//! - **谓词闭包匹配**：调用方决定匹配规则，crate 零依赖匹配库。
//! - **可取消 + 进度回调**：`CancellationToken` + `ProgressSink` trait，与 Tauri 现有
//!   download/install 的异步模式对称。
//!
//! ## 当前状态（commit 1：仅骨架）
//!
//! 本 commit 只定义公开类型与 trait，**未** 实现 backend / `find_files`。
//! 后续 commit 逐步补：
//! - commit 2: walkdir backend + `find_files` 入口
//! - commit 3: USN backend（Windows 普通用户）
//! - commit 4a-c: $MFT raw record backend（Windows admin）
//! - commit 5: inspect / inspect_many

mod backend;
mod error;
mod find;
mod matcher;
mod options;

pub use crate::error::ScanError;
pub use crate::find::find_files;
pub use crate::matcher::Matcher;
pub use crate::options::{
    BackendKind, FileAttributes, FileEntry, InspectFields, InspectedEntry, NtfsScanOptions,
    ProgressEvent, ScanLimitKind, VersionInfo,
};

use std::sync::Arc;

/// 进度回调 trait。调用方实现后在 `find_files` 内部接收扫描事件。
///
/// 实现需要 `Send + Sync` 因为多盘 backend 会并发触发事件。
/// 推荐实现内部用 `Mutex<State>` 或原子操作维护可观察状态。
pub trait ProgressSink: Send + Sync {
    fn emit(&self, event: ProgressEvent);
}

/// 默认空实现，调用方不需要进度时用。零开销。
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSink;

impl ProgressSink for NoopSink {
    #[inline]
    fn emit(&self, _: ProgressEvent) {}
}

/// 把闭包快速包成 ProgressSink，常用于测试与简单场景。
pub fn sink_from<F>(f: F) -> Arc<dyn ProgressSink>
where
    F: Fn(ProgressEvent) + Send + Sync + 'static,
{
    struct ClosureSink<F>(F);
    impl<F> ProgressSink for ClosureSink<F>
    where
        F: Fn(ProgressEvent) + Send + Sync,
    {
        #[inline]
        fn emit(&self, event: ProgressEvent) {
            (self.0)(event);
        }
    }
    Arc::new(ClosureSink(f))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_sink_does_not_panic() {
        let sink = NoopSink;
        sink.emit(ProgressEvent::EntriesFound { found: 0 });
        sink.emit(ProgressEvent::DriveCompleted {
            root: std::path::PathBuf::from("C:\\"),
            scanned: 0,
            found: 0,
        });
    }

    #[test]
    fn closure_sink_invokes_callback() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_closure = Arc::clone(&counter);
        let sink = sink_from(move |_| {
            counter_for_closure.fetch_add(1, Ordering::Relaxed);
        });
        sink.emit(ProgressEvent::EntriesFound { found: 1 });
        sink.emit(ProgressEvent::EntriesFound { found: 2 });
        assert_eq!(counter.load(Ordering::Relaxed), 2);
    }
}
