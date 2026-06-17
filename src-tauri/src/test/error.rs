use super::{
    classify_error_code, IpcError, ManagerError, IPC_ERROR_CHECKSUM_MISMATCH,
    IPC_ERROR_CLIENT_RUNNING, IPC_ERROR_MANIFEST_UNREACHABLE, IPC_ERROR_NETWORK_HOST_NOT_TRUSTED,
    IPC_ERROR_NETWORK_HTTPS_REQUIRED, IPC_ERROR_NOT_FOUND, IPC_ERROR_SHA256_MISSING,
    IPC_ERROR_UNKNOWN,
};

#[test]
fn classifies_known_error_messages_to_stable_codes() {
    assert_eq!(
        classify_error_code("host is not trusted: github.com"),
        IPC_ERROR_NETWORK_HOST_NOT_TRUSTED
    );
    assert_eq!(
        classify_error_code("host is not enabled"),
        IPC_ERROR_NETWORK_HOST_NOT_TRUSTED
    );
    assert_eq!(
        classify_error_code("asset url must use https"),
        IPC_ERROR_NETWORK_HTTPS_REQUIRED
    );
    assert_eq!(
        classify_error_code("checksum mismatch for download"),
        IPC_ERROR_CHECKSUM_MISMATCH
    );
    assert_eq!(
        classify_error_code("sha256 verification failed"),
        IPC_ERROR_CHECKSUM_MISMATCH
    );
    assert_eq!(
        classify_error_code("client installation not found: abc"),
        IPC_ERROR_NOT_FOUND
    );
    assert_eq!(
        classify_error_code("client is running, cannot install"),
        IPC_ERROR_CLIENT_RUNNING
    );
    assert_eq!(
        classify_error_code("failed to fetch manifest"),
        IPC_ERROR_MANIFEST_UNREACHABLE
    );
    assert_eq!(
        classify_error_code("更新资产缺少 sha256，自动安装已禁用"),
        IPC_ERROR_SHA256_MISSING
    );
}

#[test]
fn classifies_unknown_message_to_unknown_code() {
    assert_eq!(
        classify_error_code("一些未分类的随机错误"),
        IPC_ERROR_UNKNOWN
    );
}

#[test]
fn ipc_error_serializes_with_code_and_message() {
    let error = IpcError::new(IPC_ERROR_CHECKSUM_MISMATCH, "下载文件校验失败");
    let serialized = serde_json::to_value(error).expect("IPC 错误序列化应成功");
    assert_eq!(serialized["code"], "checksum_mismatch");
    assert_eq!(serialized["message"], "下载文件校验失败");
}

#[test]
fn ipc_error_from_string_classifies_and_keeps_message() {
    let error = IpcError::from("host is not trusted".to_string());
    assert_eq!(error.code, IPC_ERROR_NETWORK_HOST_NOT_TRUSTED);
    assert_eq!(error.message, "host is not trusted");
}

#[test]
fn manager_error_maps_variants_to_stable_codes() {
    assert_eq!(
        ManagerError::NetworkHostNotTrusted("x".into()).error_code(),
        IPC_ERROR_NETWORK_HOST_NOT_TRUSTED
    );
    assert_eq!(
        ManagerError::NetworkHttpsRequired("x".into()).error_code(),
        IPC_ERROR_NETWORK_HTTPS_REQUIRED
    );
    assert_eq!(
        ManagerError::ChecksumMismatch("x".into()).error_code(),
        IPC_ERROR_CHECKSUM_MISMATCH
    );
    assert_eq!(
        ManagerError::NotFound("x".into()).error_code(),
        IPC_ERROR_NOT_FOUND
    );
    assert_eq!(
        ManagerError::ClientRunning("x".into()).error_code(),
        IPC_ERROR_CLIENT_RUNNING
    );
    assert_eq!(
        ManagerError::ManifestUnreachable("x".into()).error_code(),
        IPC_ERROR_MANIFEST_UNREACHABLE
    );
    assert_eq!(
        ManagerError::Sha256Missing("x".into()).error_code(),
        IPC_ERROR_SHA256_MISSING
    );
    assert_eq!(
        ManagerError::Internal("x".into()).error_code(),
        IPC_ERROR_UNKNOWN
    );
}

#[test]
fn manager_error_converts_to_ipc_error_with_correct_code() {
    let ipc: IpcError = ManagerError::ChecksumMismatch("sha256 mismatch".into()).into();
    assert_eq!(ipc.code, IPC_ERROR_CHECKSUM_MISMATCH);
    assert_eq!(ipc.message, "sha256 mismatch");
}
