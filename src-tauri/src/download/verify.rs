//! 下载缓存校验与下载任务恢复摘要构建。

use crate::models::{
    DownloadCacheState, DownloadJob, DownloadJobRecovery, DownloadJobStatus, ManagerError,
};
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

/// 校验已下载文件的字节数和 SHA-256 摘要，使用流式读取避免全量加载到内存。
///
/// 返回 [`ManagerError`]，让 `checksum_mismatch` 稳定错误码在 IPC 边界保持
/// 编译期映射，而不是被 String 重新分类。
pub fn verify_downloaded_file(
    path: &Path,
    expected_sha256: &str,
    expected_size: u64,
) -> Result<(), ManagerError> {
    let file = fs::File::open(path).map_err(|error| {
        ManagerError::Internal(format!("failed to open download file: {error}"))
    })?;
    let metadata = file.metadata().map_err(|error| {
        ManagerError::Internal(format!("failed to read download file metadata: {error}"))
    })?;
    let actual_size = metadata.len();
    if actual_size != expected_size {
        return Err(ManagerError::ChecksumMismatch(format!(
            "download size mismatch: expected {expected_size}, got {actual_size}"
        )));
    }

    let mut reader = std::io::BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = reader.read(&mut buffer).map_err(|error| {
            ManagerError::Internal(format!(
                "failed to read download file for verification: {error}"
            ))
        })?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    let digest = hasher.finalize();
    let actual_sha256 = format!("{digest:x}");
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(ManagerError::ChecksumMismatch(format!(
            "download sha256 mismatch: expected {expected_sha256}, got {actual_sha256}"
        )));
    }

    Ok(())
}
