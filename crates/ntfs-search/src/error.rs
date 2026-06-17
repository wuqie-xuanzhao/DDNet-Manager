//! crate 错误类型。完全独立于业务层 ManagerError，桥接由调用方完成。

use std::path::PathBuf;

/// 扫描或扩展信息查询过程中产生的错误。
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// 调用方传入无效的 root 路径（空字符串、不存在的盘符等）。
    #[error("invalid root path: {0}")]
    InvalidRoot(String),

    /// 打开卷句柄失败（如盘符不存在、chkdsk 独占、杀软拦截）。
    #[error("failed to open volume {root}: {source}")]
    VolumeOpenFailed {
        root: String,
        #[source]
        source: std::io::Error,
    },

    /// $MFT raw record 读取失败（admin 路径降级信号）。
    #[error("MFT raw read failed on {root}: {detail}")]
    MftReadFailed { root: String, detail: String },

    /// USN 枚举失败（普通用户路径降级信号）。
    #[error("USN enumeration failed on {root}: {detail}")]
    UsnEnumFailed { root: String, detail: String },

    /// 该 root 的所有 backend 都失败。
    #[error("no backend available for {root} (MFT/USN/walkdir all failed)")]
    NoBackendAvailable { root: String },

    /// 调用方主动取消扫描。返回 `Err(ScanError::Cancelled)`，调用方决定是转 `Ok(vec![])`
    /// 还是 propagate 到上游错误。
    #[error("scan cancelled by user")]
    Cancelled,

    /// inspect 单条扩展查询失败。`inspect_many` 不传播该错误，而是用 `InspectOutcome::Failed` 标记。
    #[error("inspect failed for {path}: {detail}")]
    InspectFailed { path: PathBuf, detail: String },

    /// 内部 bug：spawn join 失败、平台不支持等不应在正常流程中出现的错误。
    #[error("internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_invalid_root_includes_message() {
        let err = ScanError::InvalidRoot("empty path".into());
        assert_eq!(err.to_string(), "invalid root path: empty path");
    }

    #[test]
    fn display_cancelled_is_stable() {
        let err = ScanError::Cancelled;
        assert_eq!(err.to_string(), "scan cancelled by user");
    }

    #[test]
    fn display_volume_open_failed_chains_source() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err = ScanError::VolumeOpenFailed {
            root: "C:".into(),
            source: io,
        };
        let msg = err.to_string();
        assert!(msg.contains("C:"), "msg should contain root: {msg}");
        assert!(
            msg.contains("access denied"),
            "msg should contain source: {msg}"
        );
    }

    #[test]
    fn display_no_backend_available_lists_strategies() {
        let err = ScanError::NoBackendAvailable { root: "all".into() };
        let msg = err.to_string();
        assert!(msg.contains("MFT"));
        assert!(msg.contains("USN"));
        assert!(msg.contains("walkdir"));
    }
}
