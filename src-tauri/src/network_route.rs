//! 网络路由辅助：根据用户配置构建走本地代理隧道的 reqwest 客户端。
//!
//! 本地代理模式（LocalProxy）通过 reqwest::Proxy 让所有出站请求走用户指定的
//! 本地代理（如 Clash 的 http://127.0.0.1:7890），URL 本身不改写。下载目标
//! host 仍由各模块的 SSRF 校验把关，代理地址只作为隧道出口，不进入目标校验。

use crate::models::NetworkRouteConfig;
use std::time::Duration;

/// 构建带本地代理隧道的 reqwest 客户端。
///
/// - `direct` 模式或未配置 route：返回普通客户端。
/// - `local_proxy` 模式：显式注入 reqwest::Proxy，让出站请求走用户指定的本地代理。
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
) -> Result<reqwest::Client, String> {
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

    if let Some(route) = route {
        if let Some(proxy_url) = route.local_proxy_url.as_deref() {
            let proxy_url = proxy_url.trim();
            if !proxy_url.is_empty() {
                let proxy = reqwest::Proxy::all(proxy_url)
                    .map_err(|error| format!("invalid local proxy url: {error}"))?;
                builder = builder.proxy(proxy);
            }
        }
    }

    builder
        .build()
        .map_err(|error| format!("failed to build http client: {error}"))
}

#[cfg(test)]
#[path = "test/network_route.rs"]
mod tests;
