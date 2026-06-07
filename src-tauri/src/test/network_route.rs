use super::{
    probe_manifest_route, select_fastest_successful_route, select_manifest_route,
    NetworkRouteCandidate, NetworkRouteProbeOutcome,
};
use crate::models::{NetworkRouteConfig, NetworkRouteMode};

#[test]
fn select_fastest_successful_route_prefers_lowest_elapsed_ms() {
    let fast = NetworkRouteCandidate::named(
        "proxy-fast",
        NetworkRouteConfig {
            mode: NetworkRouteMode::ProxyPrefix,
            proxy_prefix_url: Some("https://fast.invalid/proxy/".to_string()),
            mirror_template: None,
            enabled_hosts: vec!["fast.invalid".to_string()],
        },
    );
    let slow = NetworkRouteCandidate::named(
        "proxy-slow",
        NetworkRouteConfig {
            mode: NetworkRouteMode::ProxyPrefix,
            proxy_prefix_url: Some("https://slow.invalid/proxy/".to_string()),
            mirror_template: None,
            enabled_hosts: vec!["slow.invalid".to_string()],
        },
    );

    let selected = select_fastest_successful_route(vec![
        NetworkRouteProbeOutcome::success(slow.clone(), 120),
        NetworkRouteProbeOutcome::failure(NetworkRouteCandidate::direct(), "timeout"),
        NetworkRouteProbeOutcome::success(fast.clone(), 40),
    ])
    .expect("应选中最快成功 route");

    assert_eq!(selected.name, "proxy-fast");
    assert_eq!(selected.route, fast.route);
}

#[test]
fn select_fastest_successful_route_reports_all_failures() {
    let error = select_fastest_successful_route(vec![
        NetworkRouteProbeOutcome::failure(NetworkRouteCandidate::direct(), "timeout"),
        NetworkRouteProbeOutcome::failure(
            NetworkRouteCandidate::named(
                "proxy-a",
                NetworkRouteConfig {
                    mode: NetworkRouteMode::ProxyPrefix,
                    proxy_prefix_url: Some("https://proxy.invalid/proxy/".to_string()),
                    mirror_template: None,
                    enabled_hosts: vec!["proxy.invalid".to_string()],
                },
            ),
            "503",
        ),
    ])
    .expect_err("全部失败应返回聚合错误");

    assert!(error.contains("direct: timeout"));
    assert!(error.contains("proxy-a: 503"));
}

#[test]
fn candidate_manifest_url_rejects_unenabled_route_host() {
    let candidate = NetworkRouteCandidate::named(
        "proxy",
        NetworkRouteConfig {
            mode: NetworkRouteMode::ProxyPrefix,
            proxy_prefix_url: Some("https://proxy.invalid/proxy/".to_string()),
            mirror_template: None,
            enabled_hosts: vec!["other.invalid".to_string()],
        },
    );

    let error = candidate
        .manifest_probe_url(
            "https://raw.githubusercontent.com/example/ddnet-manager-manifest/main/manifest.json",
        )
        .expect_err("未启用的 route host 应被拒绝");

    assert_eq!(error, "network route host is not enabled");
}

#[tokio::test]
async fn probe_manifest_route_reports_route_validation_failure() {
    let candidate = NetworkRouteCandidate::named(
        "proxy",
        NetworkRouteConfig {
            mode: NetworkRouteMode::ProxyPrefix,
            proxy_prefix_url: Some("https://proxy.invalid/proxy/".to_string()),
            mirror_template: None,
            enabled_hosts: vec!["other.invalid".to_string()],
        },
    );

    let outcome = probe_manifest_route(
        "https://raw.githubusercontent.com/example/ddnet-manager-manifest/main/manifest.json",
        candidate,
    )
    .await;

    let error =
        select_fastest_successful_route(vec![outcome]).expect_err("路由校验失败应作为探测失败聚合");

    assert!(error.contains("proxy: network route host is not enabled"));
}

#[tokio::test]
async fn select_manifest_route_aggregates_validation_failures() {
    let error = select_manifest_route(
        "https://raw.githubusercontent.com/example/ddnet-manager-manifest/main/manifest.json",
        vec![
            NetworkRouteCandidate::named(
                "proxy-a",
                NetworkRouteConfig {
                    mode: NetworkRouteMode::ProxyPrefix,
                    proxy_prefix_url: Some("https://proxy-a.invalid/proxy/".to_string()),
                    mirror_template: None,
                    enabled_hosts: vec!["other-a.invalid".to_string()],
                },
            ),
            NetworkRouteCandidate::named(
                "proxy-b",
                NetworkRouteConfig {
                    mode: NetworkRouteMode::ProxyPrefix,
                    proxy_prefix_url: Some("https://proxy-b.invalid/proxy/".to_string()),
                    mirror_template: None,
                    enabled_hosts: vec!["other-b.invalid".to_string()],
                },
            ),
        ],
    )
    .await
    .expect_err("全部候选校验失败时应返回聚合错误");

    assert!(error.contains("proxy-a: network route host is not enabled"));
    assert!(error.contains("proxy-b: network route host is not enabled"));
}
