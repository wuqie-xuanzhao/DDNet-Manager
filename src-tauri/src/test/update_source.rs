use super::current_platform;
use crate::models::{CheckClientUpdateRequest, UpdateAction, UpdateCheckReason, UpdateSourceKind};

#[test]
fn linux_current_platform_includes_architecture() {
    let platform = current_platform_for("linux", "x86_64");

    assert_eq!(platform, "linux-x86_64");
}

#[test]
fn linux_arm64_current_platform_includes_architecture() {
    let platform = current_platform_for("linux", "aarch64");

    assert_eq!(platform, "linux-aarch64");
}

fn current_platform_for(os: &str, arch: &str) -> String {
    super::platform_from_os_arch(os, arch)
}

#[test]
fn current_platform_uses_runtime_constants_without_empty_result() {
    assert!(!current_platform().is_empty());
}

/// 未知 client_id 不走网络，直接返回 ClientNotInCatalog。
/// 守卫 reason 字段映射正确，避免前端误判为"已是最新版"。
#[tokio::test]
async fn check_client_update_returns_client_not_in_catalog_for_unknown_client() {
    let request = CheckClientUpdateRequest {
        client_id: "unknown-client".to_string(),
        channel: "stable".to_string(),
        manifest_url: None,
        platform: Some("windows-x86_64".to_string()),
        network_route: None,
        use_manifest_source: false,
    };
    let result = super::check_client_update(&request, None)
        .await
        .expect("未知 client_id 不应返回 Err");
    assert_eq!(result.reason, UpdateCheckReason::ClientNotInCatalog);
    assert_eq!(result.action, UpdateAction::None);
    assert!(!result.needs_update);
    assert!(result.latest_version.is_empty());
    assert!(result.message.is_some());
}

/// unavailable_check 是所有不可用更新检查结果的统一构造入口。
/// 守卫其字段完整性，确保 action=None / needs_update=false / 空 latest_version 等不变量。
#[test]
fn unavailable_check_builds_correct_fields() {
    let result = super::unavailable_check(super::UnavailableCheckInput {
        client_id: "qmclient",
        channel: "stable",
        current_version: Some("1.0.0"),
        platform: "windows-x86_64",
        reason: UpdateCheckReason::AutoUpdateDisabled,
        source_kind: UpdateSourceKind::None,
        message: "该客户端不支持自动更新。".to_string(),
    });
    assert_eq!(result.client_id, "qmclient");
    assert_eq!(result.channel, "stable");
    assert_eq!(result.current_version.as_deref(), Some("1.0.0"));
    assert!(result.latest_version.is_empty());
    assert_eq!(result.asset.size, 0);
    assert!(!result.needs_update);
    assert_eq!(result.action, UpdateAction::None);
    assert!(result.action_url.is_none());
    assert!(result.message.is_some());
    assert_eq!(result.reason, UpdateCheckReason::AutoUpdateDisabled);
}
