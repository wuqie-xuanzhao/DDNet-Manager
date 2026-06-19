//! 下载缓存校验与下载任务恢复摘要构建。

use crate::error::ManagerError;
use crate::models::{DownloadCacheState, DownloadJob, DownloadJobRecovery, DownloadJobStatus};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

/// 表示下载任务恢复后可执行动作的判断结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DownloadJobRecoveryDecision {
    /// 当前是否允许直接安装缓存文件。
    pub can_install: bool,
    /// 当前是否建议重新下载。
    pub can_retry: bool,
}

impl DownloadJobRecoveryDecision {
    /// 根据任务状态与缓存状态推导恢复动作。
    pub fn from_cache_state(status: DownloadJobStatus, cache_state: DownloadCacheState) -> Self {
        let verified = cache_state == DownloadCacheState::Verified;
        let terminal = matches!(
            status,
            DownloadJobStatus::Verified | DownloadJobStatus::Failed
        );
        let completed = status == DownloadJobStatus::Completed;
        Self {
            can_install: verified && terminal,
            can_retry: !verified || !(terminal || completed),
        }
    }
}

/// 基于下载任务当前缓存文件构建恢复摘要。
pub fn build_download_job_recovery(job: &DownloadJob) -> Result<DownloadJobRecovery, String> {
    let cache_state = detect_download_cache_state(job)?;
    let permanent_install_failure = is_permanent_install_failure(job.error.as_deref());
    let decision = if permanent_install_failure && cache_state == DownloadCacheState::Verified {
        DownloadJobRecoveryDecision {
            can_install: false,
            can_retry: false,
        }
    } else {
        DownloadJobRecoveryDecision::from_cache_state(job.status.clone(), cache_state.clone())
    };
    Ok(DownloadJobRecovery {
        job: job.clone(),
        cache_state: cache_state.clone(),
        can_install: decision.can_install,
        can_retry: decision.can_retry,
        user_message: recovery_message(cache_state, permanent_install_failure),
    })
}

fn detect_download_cache_state(job: &DownloadJob) -> Result<DownloadCacheState, String> {
    let cache_path = Path::new(&job.cache_path);
    if !cache_path.exists() {
        return Ok(DownloadCacheState::Missing);
    }
    if verify_downloaded_file(cache_path, &job.sha256, job.size)
        .map_err(|error| error.to_string())
        .is_ok()
    {
        return Ok(DownloadCacheState::Verified);
    }
    if matches!(
        job.status,
        DownloadJobStatus::Pending | DownloadJobStatus::Downloading
    ) && job.downloaded_bytes < job.size
    {
        return Ok(DownloadCacheState::Present);
    }
    Ok(DownloadCacheState::Corrupted)
}

fn is_permanent_install_failure(error: Option<&str>) -> bool {
    error.is_some_and(|error| {
        error.contains("unknown package type requires manual install")
            || error.contains("requires macOS hdiutil")
    })
}

fn recovery_message(cache_state: DownloadCacheState, permanent_install_failure: bool) -> String {
    if permanent_install_failure {
        return "缓存文件已校验，但当前平台或包类型不支持自动安装，请改用手动安装。".to_string();
    }
    String::from(match cache_state {
        DownloadCacheState::Missing => "缓存文件不存在，请重新下载更新包。",
        DownloadCacheState::Present => "缓存文件未完成校验，建议重新下载后再安装。",
        DownloadCacheState::Verified => "缓存文件已校验，可直接安装。",
        DownloadCacheState::Corrupted => "缓存文件校验失败，请重新下载更新包。",
    })
}

/// 计算输入字节的 SHA-256 小写十六进制摘要。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

/// SHA-256 流式读取的缓冲区大小（review issue #14：verify_downloaded_file 和
/// compute_exe_sha256_if_small 共用同一常量，避免重复魔数 256*1024）。
pub const SHA256_BUFFER_SIZE: usize = 256 * 1024;

/// 校验进度回调 trait。bytes_read 是累计已读取字节数，total 是文件总字节数。
/// 调用方实现后在 [`compute_file_sha256_hex_with_progress`] 内部接收读取进度，
/// 用于 verify 阶段给前端 emit 进度事件（避免校验大文件时 spinner 无反馈）。
///
/// 设计参考 ntfs_search::ProgressSink：trait + 默认 NoopVerifySink 零开销。
pub trait VerifyProgressSink: Send + Sync {
    /// 报告当前读取进度。total 为 0 时表示未知（极端边界，一般不会发生）。
    fn emit(&self, bytes_read: u64, total: u64);
}

/// 默认空实现。无进度需求时传它，零开销。
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopVerifySink;

impl VerifyProgressSink for NoopVerifySink {
    #[inline]
    fn emit(&self, _bytes_read: u64, _total: u64) {}
}

/// 流式计算文件 SHA-256 小写十六进制摘要，使用 [`SHA256_BUFFER_SIZE`] 大小的
/// 堆分配缓冲区。复用函数避免 verify_downloaded_file 和 compute_exe_sha256_if_small
/// 各自重复 buffer 分配逻辑。
pub fn compute_file_sha256_hex(path: &Path) -> std::io::Result<String> {
    compute_file_sha256_hex_with_progress(path, &NoopVerifySink)
}

/// [`compute_file_sha256_hex`] 的进度回调版本。每读一个 buffer 块就调
/// `sink.emit(bytes_read, total)`，让前端能渲染校验进度条。无 progress 需求时
/// 传 [`NoopVerifySink`] 与原 `compute_file_sha256_hex` 性能等同。
pub fn compute_file_sha256_hex_with_progress(
    path: &Path,
    sink: &dyn VerifyProgressSink,
) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let total = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut hasher = Sha256::new();
    let mut buffer: Box<[u8; SHA256_BUFFER_SIZE]> = Box::new([0u8; SHA256_BUFFER_SIZE]);
    let mut bytes_read_total: u64 = 0;
    loop {
        let bytes_read = file.read(&mut buffer[..])?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        bytes_read_total = bytes_read_total.saturating_add(bytes_read as u64);
        sink.emit(bytes_read_total, total);
    }
    let digest = hasher.finalize();
    Ok(format!("{digest:x}"))
}

/// 校验已下载文件的字节数和 SHA-256 摘要，使用流式读取避免全量加载到内存。
///
/// 返回 [`ManagerError`]，让 `checksum_mismatch` 稳定错误码在 IPC 边界保持
/// 编译期映射，而不是被 String 重新分类。
pub fn verify_downloaded_file(
    path: &Path,
    expected_sha256: &str,
    expected_size: u64,
) -> Result<(), ManagerError> {
    verify_downloaded_file_with_progress(path, expected_sha256, expected_size, &NoopVerifySink)
}

/// [`verify_downloaded_file`] 的进度回调版本。download.rs 主流程切换到这个版本，
/// 通过 TauriVerifySink 给前端 emit `verify-progress` 事件，让 DownloadButton
/// 能在校验大文件时显示进度，避免 spinner 无反馈。
pub fn verify_downloaded_file_with_progress(
    path: &Path,
    expected_sha256: &str,
    expected_size: u64,
    sink: &dyn VerifyProgressSink,
) -> Result<(), ManagerError> {
    // 先做 size 校验（fast fail，避免对 size 不匹配的文件浪费 sha 计算）
    let metadata = fs::metadata(path).map_err(|error| {
        ManagerError::Internal(format!("failed to read download file metadata: {error}"))
    })?;
    let actual_size = metadata.len();
    if actual_size != expected_size {
        return Err(ManagerError::ChecksumMismatch(format!(
            "download size mismatch: expected {expected_size}, got {actual_size}"
        )));
    }

    // 起始 emit 一次 0/total，让前端立即进入"校验中"状态而不是 spinner
    sink.emit(0, actual_size);

    // 用公共 compute_file_sha256_hex_with_progress 复用缓冲区分配逻辑（review issue #14）
    let actual_sha256 = compute_file_sha256_hex_with_progress(path, sink).map_err(|error| {
        ManagerError::Internal(format!(
            "failed to read download file for verification: {error}"
        ))
    })?;
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(ManagerError::ChecksumMismatch(format!(
            "download sha256 mismatch: expected {expected_sha256}, got {actual_sha256}"
        )));
    }

    Ok(())
}
