//! 网络路由辅助：根据用户配置构建走本地代理隧道的 reqwest 客户端。
//!
//! 三种代理模式：
//! - `Direct`：不使用代理，直接访问原始地址。
//! - `LocalProxy`：用户手动填写本地代理地址（如 Clash 的 http://127.0.0.1:7890）。
//! - `AutoDetect`：自动检测系统代理——先读环境变量 `HTTPS_PROXY`/`HTTP_PROXY`，
//!   再读 Windows 注册表系统代理（覆盖 Clash/V2Ray 设置系统代理的场景）。
//!
//! URL 本身不改写，代理只作为隧道出口。下载目标 host 仍由各模块的 SSRF 校验把关，
//! 代理地址（如 127.0.0.1）不进入目标校验。

use crate::error::ManagerError;
use crate::models::{NetworkRouteConfig, NetworkRouteMode};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

/// 路由探测结果缓存项。
struct RouteProbeCache {
    route: NetworkRouteConfig,
    local_proxy_url: Option<String>,
    timestamp: std::time::Instant,
}

/// 探测结果 TTL：5 分钟。
const ROUTE_PROBE_CACHE_TTL: Duration = Duration::from_secs(300);

/// 全局路由探测缓存。成功探测后写入，后续调用在 TTL 内直接命中，
/// 避免每次无路由配置时都重新串行探测（最坏 15 秒）。
static ROUTE_PROBE_CACHE: LazyLock<Mutex<Option<RouteProbeCache>>> =
    LazyLock::new(|| Mutex::new(None));

/// 构建带本地代理隧道的 reqwest 客户端。
///
/// - `direct` 模式或未配置 route：返回普通客户端。
/// - `local_proxy` 模式：显式注入 `route.local_proxy_url` 为 reqwest::Proxy。
/// - `auto_detect` 模式：调 [`detect_system_proxy`] 解析系统代理后注入。
///
/// `follow_redirects` 控制是否自动跟随 HTTP 重定向：metadata 请求（manifest、
/// GitHub release API、DDNet 官网）传 `true`；资产下载传 `false`，由下载层手动
/// 逐跳校验目标 host，避免重定向绕过 SSRF 校验。
///
/// 本地代理地址（如 127.0.0.1）不经过 SSRF 校验——它只是隧道出口；真正的下载
/// 目标 host 由调用方继续校验（https + 可信 host + 非回环）。
pub fn build_routed_client(
    route: Option<&NetworkRouteConfig>,
    timeout: Option<Duration>,
    user_agent: Option<&str>,
    follow_redirects: bool,
) -> Result<reqwest::Client, ManagerError> {
    let mut builder = reqwest::Client::builder().redirect(if follow_redirects {
        reqwest::redirect::Policy::default()
    } else {
        reqwest::redirect::Policy::none()
    });
    if let Some(timeout) = timeout {
        builder = builder.timeout(timeout);
    }
    if let Some(user_agent) = user_agent {
        builder = builder.user_agent(user_agent);
    }

    if let Some(proxy_url) = resolve_effective_proxy(route) {
        let proxy_url = proxy_url.trim();
        if !proxy_url.is_empty() {
            let proxy = reqwest::Proxy::all(proxy_url).map_err(|error| {
                ManagerError::Internal(format!("invalid local proxy url: {error}"))
            })?;
            builder = builder.proxy(proxy);
        }
    }

    builder
        .build()
        .map_err(|error| ManagerError::Internal(format!("failed to build http client: {error}")))
}

/// 根据路由配置解析实际生效的代理 URL。
///
/// - `Direct` 或 `None`：返回 `None`（直连）。
/// - `LocalProxy`：返回 `route.local_proxy_url`（可能为空）。
/// - `AutoDetect`：调 [`detect_system_proxy`] 检测系统代理。
pub fn resolve_effective_proxy(route: Option<&NetworkRouteConfig>) -> Option<String> {
    let route = route?;
    match route.mode {
        NetworkRouteMode::Direct => None,
        NetworkRouteMode::LocalProxy => route.local_proxy_url.clone(),
        NetworkRouteMode::AutoDetect => detect_system_proxy(),
    }
}

/// 自动检测系统代理：先读环境变量，再读 Windows 注册表。
///
/// 检测顺序：
/// 1. `HTTPS_PROXY` / `https_proxy` / `HTTP_PROXY` / `http_proxy` 环境变量。
/// 2. Windows 注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings`
///    的 `ProxyEnable` + `ProxyServer`（覆盖 Clash/V2Ray 设置系统代理的场景）。
///
/// 非 Windows 平台仅走环境变量分支。返回 `http://host:port` 格式字符串。
pub fn detect_system_proxy() -> Option<String> {
    if let Some(env_proxy) = detect_env_proxy() {
        return Some(env_proxy);
    }
    #[cfg(windows)]
    if let Some(reg_proxy) = detect_windows_registry_proxy() {
        return Some(reg_proxy);
    }
    None
}

/// 从环境变量检测代理：依次尝试 HTTPS_PROXY/https_proxy/HTTP_PROXY/http_proxy。
fn detect_env_proxy() -> Option<String> {
    for var in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(value) = std::env::var(var) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(normalize_proxy_url(trimmed));
            }
        }
    }
    None
}

/// 把检测到的代理地址规范化为 reqwest::Proxy 能识别的 `scheme://host:port` 格式。
///
/// 用户配置或注册表值可能省略 scheme（如 `127.0.0.1:7890`），reqwest 会拒绝；
/// 补上 `http://` 前缀。已含 scheme（含 `socks5://`、`http://`、`https://` 等）的
/// 保持原样。review C5：此前仅检查 `http://`/`https://`，其他 scheme（如
/// `socks5://127.0.0.1:1080`）会被错误补成 `http://socks5://...`。
fn normalize_proxy_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.contains("://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

/// 解析 Windows 注册表 `ProxyServer` 值为 `host:port`。
///
/// `ProxyServer` 格式可能是：
/// - `host:port`（简单格式，直接用）
/// - `http=host:port;https=host:port;ftp=host:port;socks=host:port`（分协议格式）
///
/// 分协议格式优先取 `https=`，其次 `http=`，最后第一个条目。
fn parse_windows_proxy_server(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.contains('=') {
        // 简单格式 host:port
        return Some(trimmed.to_string());
    }
    // 分协议格式：按 ; 分割，优先 https=，其次 http=
    let mut https_entry: Option<&str> = None;
    let mut http_entry: Option<&str> = None;
    let mut first_entry: Option<&str> = None;
    for entry in trimmed.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        if first_entry.is_none() {
            first_entry = Some(entry);
        }
        if let Some(rest) = entry.strip_prefix("https=") {
            https_entry = Some(rest);
        } else if let Some(rest) = entry.strip_prefix("http=") {
            http_entry = Some(rest);
        }
    }
    let chosen = https_entry.or(http_entry).or(first_entry)?;
    // first_entry 可能含 `proto=host:port`，取 `=` 后部分
    let host_port = chosen.split('=').next_back().unwrap_or(chosen);
    if host_port.is_empty() {
        None
    } else {
        Some(host_port.to_string())
    }
}

/// 读取 Windows 注册表系统代理设置。
///
/// 仅在 Windows 平台编译。读 `HKCU\...\Internet Settings` 的 `ProxyEnable`
/// (DWORD) 和 `ProxyServer` (String)。`ProxyEnable=0` 时返回 `None`。
#[cfg(windows)]
fn detect_windows_registry_proxy() -> Option<String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let settings = hkcu
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        .ok()?;
    let proxy_enable: u32 = settings.get_value("ProxyEnable").ok()?;
    if proxy_enable == 0 {
        return None;
    }
    let proxy_server: String = settings.get_value("ProxyServer").ok()?;
    let host_port = parse_windows_proxy_server(&proxy_server)?;
    Some(normalize_proxy_url(&host_port))
}

/// 对指定路由做真实 HTTP 连通性探测，返回请求耗时（毫秒级精度）。
///
/// 探测目标为 `https://github.com`（HEAD 请求），超时 5 秒。该端点稳定且
/// 与下载/更新检查的实际目标一致，能真实反映路由可用性。
///
/// 返回 `Ok(Duration)` 表示可达，`Err(ManagerError)` 表示不可达或超时。
pub async fn probe_route_connectivity(
    route: Option<&NetworkRouteConfig>,
) -> Result<std::time::Duration, ManagerError> {
    let client = build_routed_client(route, Some(Duration::from_secs(5)), None, true)?;
    let start = std::time::Instant::now();
    let response = client
        .head("https://github.com")
        .send()
        .await
        .map_err(|error| ManagerError::Internal(format!("route probe request failed: {error}")))?;
    if !response.status().is_success() && !response.status().is_redirection() {
        return Err(ManagerError::Internal(format!(
            "route probe returned status {}",
            response.status()
        )));
    }
    Ok(start.elapsed())
}

/// 自动选择最佳网络路由：依次探测 direct、auto_detect、local_proxy，
/// 返回第一个可达的路由配置。
///
/// 探测顺序：
/// 1. `Direct`（无代理）—— 挂代理/VPN 的用户直连 GitHub 往往最快。
/// 2. `AutoDetect`（系统代理）—— 已配置系统代理的用户。
/// 3. `LocalProxy`（用户手动代理）—— 显式配置了本地代理的用户。
///
/// 若全部不可达，返回 `None`（调用方应降级提示用户检查网络）。
/// `local_proxy_url` 为空或无效时跳过 local_proxy 探测。
///
/// 探测结果在 TTL（5 分钟）内缓存，避免频繁重复探测。
pub async fn auto_select_route(local_proxy_url: Option<&str>) -> Option<NetworkRouteConfig> {
    // 先检查缓存：TTL 内且 local_proxy_url 匹配时直接命中
    {
        let guard = ROUTE_PROBE_CACHE
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(cache) = guard.as_ref() {
            if cache.timestamp.elapsed() < ROUTE_PROBE_CACHE_TTL
                && cache.local_proxy_url.as_deref() == local_proxy_url
            {
                return Some(cache.route.clone());
            }
        }
    }

    // 1. 探测 direct
    let direct = NetworkRouteConfig {
        mode: NetworkRouteMode::Direct,
        local_proxy_url: None,
    };
    if probe_route_connectivity(Some(&direct)).await.is_ok() {
        let mut guard = ROUTE_PROBE_CACHE
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *guard = Some(RouteProbeCache {
            route: direct.clone(),
            local_proxy_url: local_proxy_url.map(|s| s.to_string()),
            timestamp: std::time::Instant::now(),
        });
        return Some(direct);
    }

    // 2. 探测 auto_detect
    let auto = NetworkRouteConfig {
        mode: NetworkRouteMode::AutoDetect,
        local_proxy_url: None,
    };
    if probe_route_connectivity(Some(&auto)).await.is_ok() {
        let mut guard = ROUTE_PROBE_CACHE
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *guard = Some(RouteProbeCache {
            route: auto.clone(),
            local_proxy_url: local_proxy_url.map(|s| s.to_string()),
            timestamp: std::time::Instant::now(),
        });
        return Some(auto);
    }

    // 3. 探测 local_proxy（需用户已填写地址）
    if let Some(url) = local_proxy_url {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            let local = NetworkRouteConfig {
                mode: NetworkRouteMode::LocalProxy,
                local_proxy_url: Some(trimmed.to_string()),
            };
            if probe_route_connectivity(Some(&local)).await.is_ok() {
                let mut guard = ROUTE_PROBE_CACHE
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                *guard = Some(RouteProbeCache {
                    route: local.clone(),
                    local_proxy_url: local_proxy_url.map(|s| s.to_string()),
                    timestamp: std::time::Instant::now(),
                });
                return Some(local);
            }
        }
    }

    None
}

#[cfg(test)]
#[path = "test/network_route.rs"]
mod tests;
