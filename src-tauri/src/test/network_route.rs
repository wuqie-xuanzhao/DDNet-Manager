use super::{
    build_routed_client, detect_env_proxy, normalize_proxy_url, parse_windows_proxy_server,
    resolve_effective_proxy,
};
use crate::models::{NetworkRouteConfig, NetworkRouteMode};
use std::sync::Mutex;
use std::time::Duration;

/// env 变量是进程级共享的，并发测试会互相干扰。用静态 Mutex 串行化所有
/// 操作 HTTPS_PROXY/HTTP_PROXY 等环境变量的测试。
static ENV_MUTEX: Mutex<()> = Mutex::new(());

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
    assert!(error.to_string().contains("invalid local proxy url"));
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

// ===== 新增：AutoDetect 模式与代理检测 =====

#[test]
fn resolve_effective_proxy_returns_none_for_direct() {
    let route = NetworkRouteConfig::direct();
    assert_eq!(resolve_effective_proxy(Some(&route)), None);
}

#[test]
fn resolve_effective_proxy_returns_none_for_no_route() {
    assert_eq!(resolve_effective_proxy(None), None);
}

#[test]
fn resolve_effective_proxy_returns_url_for_local_proxy() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::LocalProxy,
        local_proxy_url: Some("http://127.0.0.1:7890".to_string()),
    };
    assert_eq!(
        resolve_effective_proxy(Some(&route)),
        Some("http://127.0.0.1:7890".to_string())
    );
}

#[test]
fn resolve_effective_proxy_returns_none_for_local_proxy_with_empty_url() {
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::LocalProxy,
        local_proxy_url: None,
    };
    assert_eq!(resolve_effective_proxy(Some(&route)), None);
}

#[test]
fn normalize_proxy_url_adds_http_scheme_when_missing() {
    assert_eq!(
        normalize_proxy_url("127.0.0.1:7890"),
        "http://127.0.0.1:7890"
    );
}

#[test]
fn normalize_proxy_url_keeps_existing_scheme() {
    assert_eq!(
        normalize_proxy_url("http://127.0.0.1:7890"),
        "http://127.0.0.1:7890"
    );
    assert_eq!(
        normalize_proxy_url("https://proxy.example:8443"),
        "https://proxy.example:8443"
    );
}

#[test]
fn parse_windows_proxy_server_simple_format() {
    assert_eq!(
        parse_windows_proxy_server("127.0.0.1:7890"),
        Some("127.0.0.1:7890".to_string())
    );
}

#[test]
fn parse_windows_proxy_server_per_protocol_format_picks_https() {
    let value = "http=127.0.0.1:7888;https=127.0.0.1:7890;ftp=127.0.0.1:7891";
    assert_eq!(
        parse_windows_proxy_server(value),
        Some("127.0.0.1:7890".to_string())
    );
}

#[test]
fn parse_windows_proxy_server_per_protocol_format_falls_back_to_http() {
    let value = "http=127.0.0.1:7888;ftp=127.0.0.1:7891";
    assert_eq!(
        parse_windows_proxy_server(value),
        Some("127.0.0.1:7888".to_string())
    );
}

#[test]
fn parse_windows_proxy_server_empty_returns_none() {
    assert_eq!(parse_windows_proxy_server(""), None);
    assert_eq!(parse_windows_proxy_server("   "), None);
}

#[test]
fn detect_env_proxy_reads_https_proxy_first() {
    let _guard = ENV_MUTEX.lock().expect("env 测试串行锁应可获取");
    // Windows env 变量名大小写不敏感：set_var("HTTPS_PROXY") 后 remove_var("https_proxy")
    // 会删掉同一变量。故先清空全部变体，再设置目标变量，避免互相覆盖。
    std::env::remove_var("HTTPS_PROXY");
    std::env::remove_var("https_proxy");
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("http_proxy");
    std::env::set_var("HTTPS_PROXY", "http://env-proxy.example:8443");
    let proxy = detect_env_proxy();
    std::env::remove_var("HTTPS_PROXY");
    assert_eq!(proxy, Some("http://env-proxy.example:8443".to_string()));
}

#[test]
fn detect_env_proxy_normalizes_missing_scheme() {
    let _guard = ENV_MUTEX.lock().expect("env 测试串行锁应可获取");
    std::env::remove_var("HTTPS_PROXY");
    std::env::remove_var("https_proxy");
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("http_proxy");
    std::env::set_var("HTTPS_PROXY", "env-proxy.example:8443");
    let proxy = detect_env_proxy();
    std::env::remove_var("HTTPS_PROXY");
    assert_eq!(proxy, Some("http://env-proxy.example:8443".to_string()));
}

#[test]
fn detect_env_proxy_returns_none_when_all_empty() {
    let _guard = ENV_MUTEX.lock().expect("env 测试串行锁应可获取");
    std::env::remove_var("HTTPS_PROXY");
    std::env::remove_var("https_proxy");
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("http_proxy");
    assert_eq!(detect_env_proxy(), None);
}

#[test]
fn builds_client_with_auto_detect_route_picks_up_env_proxy() {
    let _guard = ENV_MUTEX.lock().expect("env 测试串行锁应可获取");
    std::env::remove_var("HTTPS_PROXY");
    std::env::remove_var("https_proxy");
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("http_proxy");
    std::env::set_var("HTTPS_PROXY", "http://127.0.0.1:7890");
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::AutoDetect,
        local_proxy_url: None,
    };
    let _client = build_routed_client(Some(&route), Some(Duration::from_secs(10)), None, true)
        .expect("auto_detect 模式应从 env 检测到代理并注入");
    std::env::remove_var("HTTPS_PROXY");
}

/// review C10：Windows 上 AutoDetect 无 env 代理时会 fall through 到注册表检测。
/// 开发机若设了系统代理，注册表分支返回代理 URL，测试断言"构建普通客户端"会失败。
/// 该测试仅在非 Windows 平台运行；Windows 注册表代理检测由手动集成测试覆盖。
#[cfg(not(windows))]
#[test]
fn auto_detect_route_without_env_proxy_builds_plain_client() {
    let _guard = ENV_MUTEX.lock().expect("env 测试串行锁应可获取");
    std::env::remove_var("HTTPS_PROXY");
    std::env::remove_var("https_proxy");
    std::env::remove_var("HTTP_PROXY");
    std::env::remove_var("http_proxy");
    let route = NetworkRouteConfig {
        mode: NetworkRouteMode::AutoDetect,
        local_proxy_url: None,
    };
    // 无 env 代理时，AutoDetect 应回退为直连
    let _client = build_routed_client(Some(&route), Some(Duration::from_secs(10)), None, true)
        .expect("auto_detect 模式无检测到代理时应构建普通客户端");
}
