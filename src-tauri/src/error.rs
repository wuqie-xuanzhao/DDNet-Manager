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

    /// 将底层 String 错误按已知模式分类为带 code 的 IPC 错误。
    pub fn classify(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = classify_error_code(&message);
        Self {
            code: code.to_string(),
            message,
        }
    }
}

impl From<String> for IpcError {
    fn from(message: String) -> Self {
        Self::classify(message)
    }
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// 根据已知错误信息模式推导稳定错误码，作为底层 String 错误的集中分类兜底。
///
/// 真正彻底的结构化应让底层模块返回 `thiserror` 枚举；当前先集中分类，
/// 把字符串匹配从前端散落处收敛到后端单一可测函数。
pub fn classify_error_code(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("host is not trusted") || lower.contains("host is not enabled") {
        IPC_ERROR_NETWORK_HOST_NOT_TRUSTED
    } else if lower.contains("must use https") || lower.contains("https is required") {
        IPC_ERROR_NETWORK_HTTPS_REQUIRED
    } else if lower.contains("sha256")
        && (lower.contains("missing") || lower.contains("缺少") || lower.contains("禁用"))
    {
        IPC_ERROR_SHA256_MISSING
    } else if lower.contains("checksum") || lower.contains("sha256") || lower.contains("校验失败")
    {
        IPC_ERROR_CHECKSUM_MISMATCH
    } else if lower.contains("is running") || lower.contains("正在运行") {
        IPC_ERROR_CLIENT_RUNNING
    } else if lower.contains("not found") || lower.contains("未找到") {
        IPC_ERROR_NOT_FOUND
    } else if lower.contains("manifest")
        || lower.contains("failed to fetch")
        || lower.contains("拉取失败")
    {
        IPC_ERROR_MANIFEST_UNREACHABLE
    } else {
        IPC_ERROR_UNKNOWN
    }
}

#[cfg(test)]
#[path = "test/error.rs"]
mod tests;
