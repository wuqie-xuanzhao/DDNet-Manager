use crate::commands::{request_requires_manifest_url, required_manifest_url};
use crate::models::CheckClientUpdateRequest;

#[test]
fn required_manifest_url_rejects_missing_or_blank_input() {
    assert_eq!(
        required_manifest_url(None).expect_err("缺少 manifest 地址应被拒绝"),
        "manifest url is not configured"
    );
    assert_eq!(
        required_manifest_url(Some("   ")).expect_err("空 manifest 地址应被拒绝"),
        "manifest url is not configured"
    );
}

#[test]
fn required_manifest_url_trims_configured_input() {
    let url = required_manifest_url(Some(
        "  https://raw.githubusercontent.com/example/manifest/main/manifest.json  ",
    ))
    .expect("非空 manifest 地址应被接受");

    assert_eq!(
        url,
        "https://raw.githubusercontent.com/example/manifest/main/manifest.json"
    );
}

#[test]
fn manifest_url_is_optional_for_catalog_update_sources() {
    let request = CheckClientUpdateRequest {
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        manifest_url: None,
        platform: Some("windows-x86_64".to_string()),
        network_route: None,
        use_manifest_source: false,
    };

    assert!(!request_requires_manifest_url(&request));
}

#[test]
fn manifest_url_is_required_for_advanced_manifest_source() {
    let request = CheckClientUpdateRequest {
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        manifest_url: None,
        platform: Some("windows-x86_64".to_string()),
        network_route: None,
        use_manifest_source: true,
    };

    assert!(request_requires_manifest_url(&request));
}
