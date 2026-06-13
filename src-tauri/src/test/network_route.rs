use super::build_routed_client;
use crate::models::{NetworkRouteConfig, NetworkRouteMode};
use std::time::Duration;

#[test]
fn builds_client_without_route() {
    let _client = build_routed_client(None, Some(Duration::from_secs(10)), None, true)
        .expect("无 route 应构建普通客户端");
}

#[test]
fn builds_client_with_direct_route() {
    let route = NetworkRouteConfig::direct();
    let _client = build_routed_client(Some(&route), Some(Duration::from_secs(10)), None, true)
        .expect("direct route 应构建普通客户端");
}

#[test]
fn builds_client_with_local_http_proxy() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::LocalProxy,
        local_proxy_url: Some("http://127.0.0.1:7890".to_string()),
    };
    let _client = build_routed_client(Some(&route), Some(Duration::from_secs(10)), None, true)
        .expect("本地代理应成功注入 reqwest proxy");
}

#[test]
fn rejects_invalid_local_proxy_url() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::LocalProxy,
        local_proxy_url: Some("not a url".to_string()),
    };
    let error = build_routed_client(Some(&route), Some(Duration::from_secs(10)), None, true)
        .expect_err("无效代理 URL 应被拒绝");
    assert!(error.contains("invalid local proxy url"));
}

#[test]
fn ignores_empty_local_proxy_url() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::LocalProxy,
        local_proxy_url: Some("   ".to_string()),
    };
    let _client = build_routed_client(Some(&route), Some(Duration::from_secs(10)), None, true)
        .expect("空白代理 URL 应视为未配置代理，构建普通客户端");
}

#[test]
fn builds_download_client_without_timeout_for_large_files() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::LocalProxy,
        local_proxy_url: Some("http://127.0.0.1:7890".to_string()),
    };
    let _client = build_routed_client(Some(&route), None, None, false)
        .expect("下载客户端传 None timeout 应不限时，避免大文件被误杀");
}
