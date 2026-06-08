use std::net::IpAddr;

const LOCAL_SMOKE_ENV: &str = "DDNET_MANAGER_ALLOW_LOCAL_SMOKE";

#[cfg(test)]
thread_local! {
    static LOCAL_SMOKE_TEST_OVERRIDE: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// 判断当前 URL scheme 是否可通过本地 smoke 例外放行。
pub fn is_allowed_local_smoke_scheme(scheme: &str) -> bool {
    matches!(scheme, "http" | "https")
}

/// 判断当前 host 是否属于仅限本地 smoke 的地址范围。
pub fn is_local_smoke_host(host: &str) -> bool {
    let lower_host = host.trim_end_matches('.').to_ascii_lowercase();
    if lower_host == "localhost" || lower_host.ends_with(".localhost") {
        return true;
    }

    let ip_host = normalized_ip_host(host);
    if let Ok(ip) = ip_host.parse::<IpAddr>() {
        return is_local_smoke_ip(ip);
    }

    false
}

/// 判断当前进程是否显式开启本地 smoke 放行。
pub fn is_local_smoke_enabled() -> bool {
    cfg!(debug_assertions) && local_smoke_switch_enabled()
}

/// 判断当前 URL 是否满足“debug + 显式 env + 本地地址”的 smoke 放行条件。
pub fn allows_local_smoke_url(scheme: &str, host: &str) -> bool {
    is_allowed_local_smoke_scheme(scheme) && is_local_smoke_enabled() && is_local_smoke_host(host)
}

/// 判断原始绝对 URL 中的 host 是否属于歧义数字地址写法。
pub fn has_ambiguous_numeric_url_host(url: &str) -> bool {
    raw_url_host(url).is_some_and(is_ambiguous_numeric_host)
}

/// 判断当前 host 是否属于歧义数字地址写法，应按非公网地址拒绝。
pub fn is_ambiguous_numeric_host(host: &str) -> bool {
    let normalized = normalized_ip_host(host).trim_end_matches('.');
    if normalized.is_empty() || normalized.parse::<IpAddr>().is_ok() {
        return false;
    }

    normalized
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
}

#[cfg(test)]
struct LocalSmokeTestOverrideRestore {
    previous: Option<bool>,
}

#[cfg(test)]
impl Drop for LocalSmokeTestOverrideRestore {
    fn drop(&mut self) {
        LOCAL_SMOKE_TEST_OVERRIDE.set(self.previous);
    }
}

#[cfg(test)]
fn test_override_enabled() -> Option<bool> {
    LOCAL_SMOKE_TEST_OVERRIDE.get()
}

#[cfg(not(test))]
fn test_override_enabled() -> Option<bool> {
    None
}

/// 在当前测试线程内临时覆盖 local smoke 开关，避免进程环境变量并发污染。
#[cfg(test)]
pub fn with_local_smoke_test_env<T>(enabled: bool, callback: impl FnOnce() -> T) -> T {
    let previous = LOCAL_SMOKE_TEST_OVERRIDE.replace(Some(enabled));
    let _restore = LocalSmokeTestOverrideRestore { previous };
    callback()
}

fn local_smoke_switch_enabled() -> bool {
    test_override_enabled().unwrap_or_else(env_flag_enabled)
}

fn env_flag_enabled() -> bool {
    std::env::var(LOCAL_SMOKE_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn normalized_ip_host(host: &str) -> &str {
    host.trim_start_matches('[').trim_end_matches(']')
}

fn raw_url_host(url: &str) -> Option<&str> {
    let (_, remainder) = url.split_once("://")?;
    let authority = remainder
        .split(['/', '?', '#'])
        .next()
        .filter(|authority| !authority.is_empty())?;
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, tail)| tail);
    if host_port.starts_with('[') {
        let end = host_port.find(']')?;
        return Some(&host_port[..=end]);
    }

    match host_port.rsplit_once(':') {
        Some((host, port))
            if !host.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            Some(host)
        }
        _ => Some(host_port),
    }
}

fn is_local_smoke_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() || ipv4.is_unspecified()
        }
        IpAddr::V6(ipv6) => {
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                return is_local_smoke_ip(IpAddr::V4(ipv4));
            }

            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || is_ipv6_unique_local(&ipv6)
                || is_ipv6_unicast_link_local(&ipv6)
        }
    }
}

fn is_ipv6_unique_local(ip: &std::net::Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_ipv6_unicast_link_local(ip: &std::net::Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}
