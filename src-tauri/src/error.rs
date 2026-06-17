use serde::{Deserialize, Serialize};

/// 未知错误，未能匹配任何已知分类的错误码。
pub const IPC_ERROR_UNKNOWN: &str = "unknown";
/// 下载或请求目标 host 未被启用或未被信任。
pub const IPC_ERROR_NETWORK_HOST_NOT_TRUSTED: &str = "network_host_not_trusted";
/// 更新源地址必须使用 HTTPS。
pub const IPC_ERROR_NETWORK_HTTPS_REQUIRED: &str = "network_https_required";
/// 下载文件 size 或 sha256 校验失败。
pub const IPC_ERROR_CHECKSUM_MISMATCH: &str = "checksum_mismatch";
/// 客户端安装记录或下载任务未找到。
pub const IPC_ERROR_NOT_FOUND: &str = "not_found";
/// 目标客户端正在运行，无法写入或安装。
pub const IPC_ERROR_CLIENT_RUNNING: &str = "client_running";
/// manifest 或更新源拉取失败。
pub const IPC_ERROR_MANIFEST_UNREACHABLE: &str = "manifest_unreachable";
/// 更新资产缺少 sha256，自动安装被禁用。
pub const IPC_ERROR_SHA256_MISSING: &str = "sha256_missing";

/// 后端结构化错误类型，替代 String 错误提供稳定错误码映射。
///
/// 每个变体直接对应一个 `IPC_ERROR_*` 常量，消除字符串匹配分类的必要性。
/// 底层模块应返回此枚举而非裸 String，使 `IpcError::from(ManagerError)` 的
/// 错误码推导成为编译期确定的事实，而非运行时字符串猜测。
#[derive(Debug, thiserror::Error)]
pub enum ManagerError {
    /// 下载或请求目标 host 未被启用或未被信任。
    #[error("{0}")]
    NetworkHostNotTrusted(String),
    /// 更新源地址必须使用 HTTPS。
    #[error("{0}")]
    NetworkHttpsRequired(String),
    /// 下载文件 size 或 sha256 校验失败。
    #[error("{0}")]
    ChecksumMismatch(String),
    /// 客户端安装记录或下载任务未找到。
    #[error("{0}")]
    NotFound(String),
    /// 目标客户端正在运行，无法写入或安装。
    #[error("{0}")]
    ClientRunning(String),
    /// manifest 或更新源拉取失败。
    #[error("{0}")]
    ManifestUnreachable(String),
    /// 更新资产缺少 sha256，自动安装被禁用。
    #[error("{0}")]
    Sha256Missing(String),
    /// 未归入上述分类的内部错误。
    #[error("{0}")]
    Internal(String),
}

impl ManagerError {
    /// 返回此错误变体对应的稳定 IPC 错误码。
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::NetworkHostNotTrusted(_) => IPC_ERROR_NETWORK_HOST_NOT_TRUSTED,
            Self::NetworkHttpsRequired(_) => IPC_ERROR_NETWORK_HTTPS_REQUIRED,
            Self::ChecksumMismatch(_) => IPC_ERROR_CHECKSUM_MISMATCH,
            Self::NotFound(_) => IPC_ERROR_NOT_FOUND,
            Self::ClientRunning(_) => IPC_ERROR_CLIENT_RUNNING,
            Self::ManifestUnreachable(_) => IPC_ERROR_MANIFEST_UNREACHABLE,
            Self::Sha256Missing(_) => IPC_ERROR_SHA256_MISSING,
            Self::Internal(_) => IPC_ERROR_UNKNOWN,
        }
    }
}

impl From<ManagerError> for IpcError {
    fn from(error: ManagerError) -> Self {
        Self {
            code: error.error_code().to_string(),
            message: error.to_string(),
        }
    }
}

/// IPC 错误契约，携带稳定错误码与可读文案，供前端按 code 映射文案与重试策略。
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct IpcError {
    /// 稳定错误码，前端据此映射文案与重试策略。
    pub code: String,
    /// 面向调试的原始信息，前端可兜底展示。
    pub message: String,
}

impl IpcError {
    /// 用错误码与原始信息构造 IPC 错误。
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }
}

impl From<String> for IpcError {
    fn from(message: String) -> Self {
        // String 错误已经脱离结构化语义，统一归入 unknown；前端可基于 message 兜底展示。
        // 想要稳定错误码的模块必须返回 [`ManagerError`]，让 [`From<ManagerError>`] 走编译期映射。
        Self::new(IPC_ERROR_UNKNOWN, message)
    }
}

/// ntfs-search 错误到 ManagerError 的桥接。
///
/// ntfs-search 是独立 crate，不感知 ManagerError 存在；这里集中映射，避免业务
/// 层每个调用点都重复 match。取消、根无效等语义错误映射到对应变体，其余转 Internal。
impl From<ntfs_search::ScanError> for ManagerError {
    fn from(e: ntfs_search::ScanError) -> Self {
        use ntfs_search::ScanError;
        match e {
            ScanError::Cancelled => Self::Internal("scan cancelled by user".to_string()),
            ScanError::InvalidRoot(msg) => Self::NotFound(format!("invalid scan root: {msg}")),
            ScanError::NoBackendAvailable { root } => {
                Self::NotFound(format!("no scan backend available for {root}"))
            }
            other => Self::Internal(other.to_string()),
        }
    }
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

#[cfg(test)]
#[path = "test/error.rs"]
mod tests;
