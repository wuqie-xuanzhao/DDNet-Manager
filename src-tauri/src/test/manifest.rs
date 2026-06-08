use crate::manifest::{
    build_asset_url_with_route, build_manifest_url, build_manifest_url_with_route, parse_manifest,
    select_client_update,
};
use crate::models::{ClientUpdateSelector, NetworkRouteConfig, NetworkRouteMode};

const VALID_SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const TEST_MANIFEST_URL: &str =
    "https://raw.githubusercontent.com/example/ddnet-manager-manifest/main/manifest.json";
const ENCODED_TEST_MANIFEST_URL: &str =
        "https%3A%2F%2Fraw.githubusercontent.com%2Fexample%2Fddnet-manager-manifest%2Fmain%2Fmanifest.json";

#[test]
fn parses_manifest_with_qmclient_asset() {
    let manifest = parse_manifest(
                r#"{
                    "schema_version": 1,
                    "clients": [
                        {
                            "client_id": "qmclient",
                            "channel": "stable",
                            "version": "18.9.1",
                            "release_notes": "QmClient stable release",
                            "assets": [
                                {
                                    "platform": "windows-x86_64",
                                    "asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip",
                                    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                                    "size": 981467136
                                }
                            ]
                        }
                    ]
                }"#,
            )
            .expect("测试 manifest 应解析成功");

    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.clients[0].client_id, "qmclient");
    assert_eq!(manifest.clients[0].assets[0].platform, "windows-x86_64");
}

#[test]
fn rejects_zero_schema_version() {
    let error = parse_manifest(
        r#"{
                    "schema_version": 0,
                    "clients": [
                        {
                            "client_id": "qmclient",
                            "channel": "stable",
                            "version": "18.9.1",
                            "release_notes": "QmClient stable release",
                            "assets": []
                        }
                    ]
                }"#,
    )
    .expect_err("schema_version 为 0 应被拒绝");

    assert_eq!(error, "manifest schema_version must be greater than 0");
}

#[test]
fn rejects_empty_clients() {
    let error = parse_manifest(
        r#"{
                    "schema_version": 1,
                    "clients": []
                }"#,
    )
    .expect_err("空 clients 应被拒绝");

    assert_eq!(error, "manifest must contain at least one client");
}

#[test]
fn prefixes_invalid_json_errors() {
    let error = parse_manifest("{").expect_err("非法 JSON 应被拒绝");

    assert!(
        error.starts_with("invalid manifest json:"),
        "unexpected error: {error}"
    );
}

#[test]
fn build_manifest_url_accepts_public_https_url() {
    let url = build_manifest_url(TEST_MANIFEST_URL).expect("可信 HTTPS URL 应被接受");

    assert_eq!(url.as_str(), TEST_MANIFEST_URL);
}

#[test]
fn build_manifest_url_rejects_untrusted_host() {
    let error = build_manifest_url("https://example.com/manifest.json")
        .expect_err("非 allowlist host 应被拒绝");

    assert_eq!(error, "manifest url host is not trusted");
}

#[test]
fn build_manifest_url_rejects_http_url() {
    let error =
        build_manifest_url("http://example.com/manifest.json").expect_err("HTTP URL 应被拒绝");

    assert_eq!(error, "manifest url must use https");
}

#[test]
fn build_manifest_url_rejects_localhost_and_private_hosts() {
    for url in [
        "https://localhost/manifest.json",
        "https://localhost./manifest.json",
        "https://api.localhost/manifest.json",
        "https://127.0.0.1/manifest.json",
        "https://10.0.0.1/manifest.json",
        "https://169.254.1.1/manifest.json",
        "https://0.0.0.0/manifest.json",
        "https://[::1]/manifest.json",
        "https://[fc00::1]/manifest.json",
        "https://[fe80::1]/manifest.json",
        "https://[::ffff:127.0.0.1]/manifest.json",
    ] {
        assert!(build_manifest_url(url).is_err(), "{url} should be rejected");
    }
}

#[test]
fn build_manifest_url_allows_local_smoke_hosts_when_enabled() {
    crate::local_smoke::with_local_smoke_test_env(true, || {
        for url in [
            "http://localhost/manifest.json",
            "https://localhost/manifest.json",
            "http://127.0.0.1/manifest.json",
            "https://10.0.0.1/manifest.json",
            "http://[::1]/manifest.json",
            "https://[fc00::1]/manifest.json",
            "https://169.254.1.1/manifest.json",
        ] {
            let parsed =
                build_manifest_url(url).expect("显式开启 local smoke 后应允许本地 manifest 地址");
            assert_eq!(parsed.as_str(), url);
        }
    });
}

#[test]
fn build_manifest_url_still_rejects_public_http_when_local_smoke_enabled() {
    crate::local_smoke::with_local_smoke_test_env(true, || {
        let error = build_manifest_url("http://example.com/manifest.json")
            .expect_err("local smoke 开关不应放通公网 HTTP manifest 地址");

        assert_eq!(error, "manifest url must use https");
    });
}

#[test]
fn build_manifest_url_with_route_rejects_ambiguous_numeric_hosts_even_when_enabled() {
    for host in ["127.1", "2130706433", "0177.0.0.1"] {
        let route = NetworkRouteConfig {
            mode: NetworkRouteMode::ProxyPrefix,
            proxy_prefix_url: Some(format!("https://{host}/proxy/")),
            mirror_template: None,
            enabled_hosts: vec![host.to_string()],
        };

        let error = build_manifest_url_with_route(TEST_MANIFEST_URL, Some(&route))
            .expect_err("歧义数字 host 即使显式启用也应拒绝");

        assert_eq!(error, "manifest url host must be public", "{host}");
    }
}

#[test]
fn build_manifest_url_with_direct_route_keeps_original_url() {
    let route = NetworkRouteConfig::direct();

    let url = build_manifest_url_with_route(TEST_MANIFEST_URL, Some(&route))
        .expect("直连路由应接受可信 manifest URL");

    assert_eq!(url.as_str(), TEST_MANIFEST_URL);
}

#[test]
fn build_manifest_url_with_proxy_prefix_requires_enabled_public_host() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::ProxyPrefix,
        proxy_prefix_url: Some("https://proxy.invalid/proxy/".to_string()),
        mirror_template: None,
        enabled_hosts: vec!["proxy.invalid".to_string()],
    };

    let url = build_manifest_url_with_route(TEST_MANIFEST_URL, Some(&route))
        .expect("显式启用的 HTTPS 代理前缀应可用");

    assert_eq!(
        url.as_str(),
        format!("https://proxy.invalid/proxy/{ENCODED_TEST_MANIFEST_URL}")
    );
}

#[test]
fn build_manifest_url_with_proxy_prefix_rejects_unenabled_host() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::ProxyPrefix,
        proxy_prefix_url: Some("https://proxy.invalid/proxy/".to_string()),
        mirror_template: None,
        enabled_hosts: vec!["mirror.invalid".to_string()],
    };

    let error = build_manifest_url_with_route(TEST_MANIFEST_URL, Some(&route))
        .expect_err("未显式启用的代理 host 应被拒绝");

    assert_eq!(error, "network route host is not enabled");
}

#[test]
fn build_manifest_url_with_mirror_template_replaces_url_placeholder() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::MirrorTemplate,
        proxy_prefix_url: None,
        mirror_template: Some("https://mirror.invalid/fetch/{url}".to_string()),
        enabled_hosts: vec!["mirror.invalid".to_string()],
    };

    let url = build_manifest_url_with_route(TEST_MANIFEST_URL, Some(&route))
        .expect("显式启用的镜像模板应可用");

    assert_eq!(
        url.as_str(),
        format!("https://mirror.invalid/fetch/{TEST_MANIFEST_URL}")
    );
}

#[test]
fn build_manifest_url_with_mirror_template_requires_placeholder() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::MirrorTemplate,
        proxy_prefix_url: None,
        mirror_template: Some("https://mirror.invalid/fetch".to_string()),
        enabled_hosts: vec!["mirror.invalid".to_string()],
    };

    let error = build_manifest_url_with_route(TEST_MANIFEST_URL, Some(&route))
        .expect_err("缺少 {url} 占位符的镜像模板应被拒绝");

    assert_eq!(error, "mirror template must contain {url}");
}

#[test]
fn build_asset_url_with_proxy_prefix_uses_enabled_route_host() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::ProxyPrefix,
        proxy_prefix_url: Some("https://proxy.invalid/proxy/".to_string()),
        mirror_template: None,
        enabled_hosts: vec!["proxy.invalid".to_string()],
    };

    let url = build_asset_url_with_route(
        "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip",
        Some(&route),
    )
    .expect("显式启用的 asset 代理前缀应可用");

    assert_eq!(
            url.as_str(),
            "https://proxy.invalid/proxy/https%3A%2F%2Fgithub.com%2Fddnet%2Fddnet%2Freleases%2Fdownload%2Fv1%2Fqmclient.zip"
        );
}

#[test]
fn build_asset_url_allows_local_smoke_hosts_when_enabled() {
    crate::local_smoke::with_local_smoke_test_env(true, || {
        for url in [
            "http://127.0.0.1/qmclient.zip",
            "https://localhost/qmclient.zip",
            "http://[::1]/qmclient.zip",
        ] {
            let parsed = build_asset_url_with_route(url, None)
                .expect("显式开启 local smoke 后应允许本地 asset 地址");
            assert_eq!(parsed.as_str(), url);
        }
    });
}

#[test]
fn rejects_empty_assets() {
    let error = parse_manifest(
        r#"{
                    "schema_version": 1,
                    "clients": [
                        {
                            "client_id": "qmclient",
                            "channel": "stable",
                            "version": "18.9.1",
                            "release_notes": "QmClient stable release",
                            "assets": []
                        }
                    ]
                }"#,
    )
    .expect_err("空 assets 应被拒绝");

    assert_eq!(error, "manifest client assets must not be empty");
}

#[test]
fn rejects_invalid_sha256() {
    let error = parse_manifest(
                r#"{
                    "schema_version": 1,
                    "clients": [
                        {
                            "client_id": "qmclient",
                            "channel": "stable",
                            "version": "18.9.1",
                            "release_notes": "QmClient stable release",
                            "assets": [
                                {
                                    "platform": "windows-x86_64",
                                    "asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip",
                                    "sha256": "not-a-sha256",
                                    "size": 981467136
                                }
                            ]
                        }
                    ]
                }"#,
            )
            .expect_err("非法 sha256 应被拒绝");

    assert_eq!(error, "manifest asset sha256 must be 64 ASCII hex chars");
}

#[test]
fn rejects_http_asset_url() {
    let error = parse_manifest(
                r#"{
                    "schema_version": 1,
                    "clients": [
                        {
                            "client_id": "qmclient",
                            "channel": "stable",
                            "version": "18.9.1",
                            "release_notes": "QmClient stable release",
                            "assets": [
                                {
                                    "platform": "windows-x86_64",
                                    "asset_url": "http://example.com/qmclient.zip",
                                    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                                    "size": 981467136
                                }
                            ]
                        }
                    ]
                }"#,
            )
            .expect_err("HTTP asset_url 应被拒绝");

    assert_eq!(error, "manifest url must use https");
}

#[test]
fn accepts_trusted_asset_url() {
    let manifest = parse_manifest(&valid_manifest()).expect("可信 asset_url 应被接受");

    assert_eq!(
        manifest.clients[0].assets[0].asset_url,
        "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip"
    );
}

#[test]
fn select_client_update_filters_client_channel_and_platform() {
    let manifest = parse_manifest(&valid_manifest()).expect("测试 manifest 应解析成功");

    let update = select_client_update(
        &manifest,
        &ClientUpdateSelector {
            client_id: "qmclient".to_string(),
            channel: "stable".to_string(),
            platform: "windows-x86_64".to_string(),
        },
    )
    .expect("应找到匹配更新")
    .expect("匹配更新不应为空");

    assert_eq!(update.client_id, "qmclient");
    assert_eq!(update.channel, "stable");
    assert_eq!(update.latest_version, "18.9.1");
    assert_eq!(update.asset.platform, "windows-x86_64");
    assert!(update.needs_update);
}

#[test]
fn select_client_update_returns_none_for_unmatched_channel() {
    let manifest = parse_manifest(&valid_manifest()).expect("测试 manifest 应解析成功");

    let update = select_client_update(
        &manifest,
        &ClientUpdateSelector {
            client_id: "qmclient".to_string(),
            channel: "nightly".to_string(),
            platform: "windows-x86_64".to_string(),
        },
    )
    .expect("选择更新不应报错");

    assert!(update.is_none());
}

#[test]
fn rejects_untrusted_asset_url() {
    let input = valid_manifest().replace(
        r#""asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip""#,
        r#""asset_url": "https://example.com/qmclient.zip""#,
    );
    let error = parse_manifest(&input).expect_err("非 allowlist asset_url 应被拒绝");

    assert_eq!(error, "manifest asset_url host is not trusted");
}

#[test]
fn rejects_empty_client_id() {
    let input = valid_manifest().replace(r#""client_id": "qmclient""#, r#""client_id": """#);
    let error = parse_manifest(&input).expect_err("空 client_id 应被拒绝");

    assert_eq!(error, "manifest client_id must not be empty");
}

#[test]
fn rejects_empty_channel() {
    let input = valid_manifest().replace(r#""channel": "stable""#, r#""channel": """#);
    let error = parse_manifest(&input).expect_err("空 channel 应被拒绝");

    assert_eq!(error, "manifest client channel must not be empty");
}

#[test]
fn rejects_empty_version() {
    let input = valid_manifest().replace(r#""version": "18.9.1""#, r#""version": """#);
    let error = parse_manifest(&input).expect_err("空 version 应被拒绝");

    assert_eq!(error, "manifest client version must not be empty");
}

#[test]
fn rejects_empty_release_notes() {
    let input = valid_manifest().replace(
        r#""release_notes": "QmClient stable release""#,
        r#""release_notes": """#,
    );
    let error = parse_manifest(&input).expect_err("空 release_notes 应被拒绝");

    assert_eq!(error, "manifest client release_notes must not be empty");
}

#[test]
fn rejects_empty_platform() {
    let input = valid_manifest().replace(r#""platform": "windows-x86_64""#, r#""platform": """#);
    let error = parse_manifest(&input).expect_err("空 platform 应被拒绝");

    assert_eq!(error, "manifest asset platform must not be empty");
}

#[test]
fn rejects_empty_asset_url() {
    let input = valid_manifest().replace(
        r#""asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip""#,
        r#""asset_url": """#,
    );
    let error = parse_manifest(&input).expect_err("空 asset_url 应被拒绝");

    assert_eq!(error, "manifest asset_url must not be empty");
}

#[test]
fn rejects_zero_asset_size() {
    let input = valid_manifest().replace(r#""size": 981467136"#, r#""size": 0"#);
    let error = parse_manifest(&input).expect_err("size 为 0 应被拒绝");

    assert_eq!(error, "manifest asset size must be greater than 0");
}

fn valid_manifest() -> String {
    format!(
        r#"{{
                    "schema_version": 1,
                    "clients": [
                        {{
                            "client_id": "qmclient",
                            "channel": "stable",
                            "version": "18.9.1",
                            "release_notes": "QmClient stable release",
                            "assets": [
                                {{
                                    "platform": "windows-x86_64",
                                    "asset_url": "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip",
                                    "sha256": "{VALID_SHA256}",
                                    "size": 981467136
                                }}
                            ]
                        }}
                    ]
                }}"#
    )
}
