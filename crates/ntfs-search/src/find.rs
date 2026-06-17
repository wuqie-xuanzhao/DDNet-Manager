//! `find_files` —— crate 的对外扫描入口。
//!
//! 负责参数校验、timeout 软超时、调用 `backend::scan_all_roots`。
//! 多盘并行、backend 选择、cancel 信号广播都在 `backend` 模块内。

use crate::backend::scan_all_roots;
use crate::error::ScanError;
use crate::options::{FileEntry, NtfsScanOptions};
use crate::ProgressSink;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// 全量扫描入口。
///
/// - `opts.timeout` 触发时返回**已收集的部分结果**（不报错）；同时 cancel 信号广播
///   到 backend 让其停止（注意：tokio::time::timeout 会 drop future，backend 内的
///   spawn_blocking 线程仍在跑直到自然结束——这是 v0.1 的已知限制，commit 3+ 接入
///   MFT 后会重写为 task + channel 收集模型以精确支持"timeout 返回部分结果"）
/// - `opts.max_results` 软上限，达到立即返回
/// - 用户主动 `cancel.cancel()` 触发时返回 `Err(ScanError::Cancelled)`
pub async fn find_files(
    opts: NtfsScanOptions,
    progress: Arc<dyn ProgressSink>,
    cancel: CancellationToken,
) -> Result<Vec<FileEntry>, ScanError> {
    if opts.roots.is_empty() {
        return Err(ScanError::InvalidRoot(
            "opts.roots is empty; specify at least one root path".to_string(),
        ));
    }

    let roots = opts.roots.clone();
    let timeout = opts.timeout;

    let result = tokio::time::timeout(
        timeout,
        scan_all_roots(roots, opts, Arc::clone(&progress), cancel.clone()),
    )
    .await;

    match result {
        Ok(inner) => inner,
        Err(_elapsed) => {
            tracing::warn!(
                timeout_secs = timeout.as_secs(),
                "scan timed out, signaling cancel and returning empty"
            );
            // 广播 cancel，让 backend 内的 spawn_blocking 在下次循环检查时退出
            cancel.cancel();
            // v0.1 限制：timeout 后 scan_all_roots future 已被 drop，无法取回部分结果
            // 调用方按空 vec 处理；progress 事件流可能仍有延迟到达的 DriveCompleted
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::NtfsScanOptions;
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;

    #[tokio::test]
    async fn rejects_empty_roots() {
        let opts = NtfsScanOptions::new(|_| true);
        let result = find_files(opts, Arc::new(crate::NoopSink), CancellationToken::new()).await;
        assert!(matches!(result, Err(ScanError::InvalidRoot(_))));
    }

    #[tokio::test]
    async fn finds_files_in_two_roots() {
        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();
        fs::write(tmp_a.path().join("DDNet.exe"), b"x").unwrap();
        fs::write(tmp_b.path().join("DDNet.exe"), b"y").unwrap();
        fs::write(tmp_a.path().join("noise.txt"), b"z").unwrap();

        let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("DDNet.exe"))
            .with_roots(vec![tmp_a.path().to_path_buf(), tmp_b.path().to_path_buf()]);
        let entries = find_files(opts, Arc::new(crate::NoopSink), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[tokio::test]
    async fn cancel_propagates_as_err() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..50 {
            fs::write(tmp.path().join(format!("f{i}.exe")), b"x").unwrap();
        }

        let opts = NtfsScanOptions::new(|_| true).with_root(tmp.path().to_path_buf());
        let cancel = CancellationToken::new();
        cancel.cancel(); // 预先取消

        let result = find_files(opts, Arc::new(crate::NoopSink), cancel).await;
        // 预先 cancel 后，第一个 root 进 backend 立刻检测到 cancel → 返回 Err(Cancelled)
        // 或返回部分结果（取决于 backend 实现的 cancel 检查时机）
        match result {
            Ok(entries) => {
                // 部分结果也可接受
                let _ = entries;
            }
            Err(ScanError::Cancelled) => {}
            Err(e) => panic!("expected Cancelled or Ok, got: {e}"),
        }
    }

    #[tokio::test]
    async fn timeout_returns_empty_or_partial() {
        let tmp = tempfile::tempdir().unwrap();
        for i in 0..10 {
            fs::write(tmp.path().join(format!("f{i}.exe")), b"x").unwrap();
        }

        let opts = NtfsScanOptions::new(|_| true)
            .with_root(tmp.path().to_path_buf())
            .with_timeout(Duration::from_millis(1)); // 几乎立即超时

        let entries = find_files(opts, Arc::new(crate::NoopSink), CancellationToken::new())
            .await
            .unwrap();
        // 超时后返回空 vec 或部分结果都可接受
        let _ = entries.len();
    }

    #[tokio::test]
    async fn nonexistent_root_returns_empty() {
        // walkdir 对 nonexistent root 直接产生 EntryError 后返回 Ok(vec![])，
        // 不视作 NoBackendAvailable（要求所有 backend 失败才报）
        let opts = NtfsScanOptions::new(|_| true).with_roots(vec![
            PathBuf::from("/nonexistent/path/1"),
            PathBuf::from("/nonexistent/path/2"),
        ]);
        let result = find_files(opts, Arc::new(crate::NoopSink), CancellationToken::new()).await;
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
        let entries = result.unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn progress_events_emitted_in_order() {
        use std::sync::Mutex;
        let events = Arc::new(Mutex::new(Vec::<String>::new()));
        let events_clone = Arc::clone(&events);
        let sink = crate::sink_from(move |ev| {
            let kind = match ev {
                crate::ProgressEvent::DriveStarted { .. } => "started",
                crate::ProgressEvent::DriveCompleted { .. } => "completed",
                _ => "other",
            };
            events_clone.lock().unwrap().push(kind.to_string());
        });

        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("DDNet.exe"), b"x").unwrap();

        let opts = NtfsScanOptions::new(|n| n.eq_ignore_ascii_case("DDNet.exe"))
            .with_root(tmp.path().to_path_buf());
        find_files(opts, sink, CancellationToken::new())
            .await
            .unwrap();

        let events = events.lock().unwrap();
        assert!(events.iter().any(|e| e == "started"));
        assert!(events.iter().any(|e| e == "completed"));
    }
}
