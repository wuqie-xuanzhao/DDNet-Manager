use crate::local_smoke;
use crate::models::{
    ClientUpdateCheck, ClientUpdateSelector, NetworkRouteConfig, NetworkRouteMode, UpdateAction,
    UpdateManifest, UpdateSourceKind,
};
use reqwest::Url;
use std::net::{IpAddr, Ipv6Addr};
use std::time::Duration;

const MAX_MANIFEST_BYTES: usize = 1_048_576;
const TRUSTED_MANIFEST_HOSTS: &[&str] = &["raw.githubusercontent.com"];
const TRUSTED_ASSET_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "raw.githubusercontent.com",
    "ddnet.org",
];

/// 解析更新 manifest JSON，并校验基础结构约束。
pub fn parse_manifest(input: &str) -> Result<UpdateManifest, String> {
    let manifest: UpdateManifest =
        serde_json::from_str(input).map_err(|error| format!("invalid manifest json: {error}"))?;

    if manifest.schema_version == 0 {
        return Err("manifest schema_version must be greater than 0".to_string());
    }

    if manifest.clients.is_empty() {
        return Err("manifest must contain at least one client".to_string());
    }

    validate_manifest_schema(&manifest)?;

    Ok(manifest)
}

/// 构造并校验 manifest URL，确保只访问公开 HTTPS 地址。
pub fn build_manifest_url(url: &str) -> Result<Url, String> {
    if local_smoke::has_ambiguous_numeric_url_host(url) {
        return Err("manifest url host must be public".to_string());
    }

    let parsed = Url::parse(url).map_err(|error| format!("invalid manifest url: {error}"))?;

    validate_manifest_url(&parsed)?;

    Ok(parsed)
}

/// 根据显式网络路由配置构造并校验 manifest URL。
pub fn build_manifest_url_with_route(
    url: &str,
    route: Option<&NetworkRouteConfig>,
) -> Result<Url, String> {
    let original = build_manifest_url(url)?;
    let Some(route) = route else {
        return Ok(original);
    };

    match route.mode {
        NetworkRouteMode::Direct => Ok(original),
        NetworkRouteMode::ProxyPrefix => build_proxy_prefix_url(&original, route),
        NetworkRouteMode::MirrorTemplate => build_mirror_template_url(&original, route),
    }
}

/// 根据显式网络路由配置构造并校验 asset URL。
pub fn build_asset_url_with_route(
    url: &str,
    route: Option<&NetworkRouteConfig>,
) -> Result<Url, String> {
    if local_smoke::has_ambiguous_numeric_url_host(url) {
        return Err("manifest url host must be public".to_string());
    }

    let original =
        Url::parse(url).map_err(|error| format!("invalid manifest asset_url: {error}"))?;
    validate_asset_url(&original)?;
    let Some(route) = route else {
        return Ok(original);
    };

    match route.mode {
        NetworkRouteMode::Direct => Ok(original),
        NetworkRouteMode::ProxyPrefix => build_proxy_prefix_url(&original, route),
        NetworkRouteMode::MirrorTemplate => build_mirror_template_url(&original, route),
    }
}

/// 从远程地址拉取更新 manifest，并复用本地解析校验逻辑。
pub async fn fetch_manifest(url: &str) -> Result<UpdateManifest, String> {
    let final_url = build_manifest_url(url)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("failed to fetch manifest: {error}"))?;

    let response = client
        .get(final_url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("failed to fetch manifest: {error}"))?;
    let text = read_limited_manifest_response(response).await?;

    parse_manifest(&text)
}

/// 使用显式网络路由配置从远程地址拉取更新 manifest。
pub async fn fetch_manifest_with_route(
    url: &str,
    route: Option<&NetworkRouteConfig>,
) -> Result<UpdateManifest, String> {
    let final_url = build_manifest_url_with_route(url, route)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("failed to fetch manifest: {error}"))?;

    let response = client
        .get(final_url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("failed to fetch manifest: {error}"))?;
    let text = read_limited_manifest_response(response).await?;

    parse_manifest(&text)
}

/// 从已校验的 manifest 中选择指定客户端、渠道与平台的更新资产。
pub fn select_client_update(
    manifest: &UpdateManifest,
    selector: &ClientUpdateSelector,
) -> Result<Option<ClientUpdateCheck>, String> {
    let Some(client) = manifest.clients.iter().find(|client| {
        client.client_id == selector.client_id && client.channel == selector.channel
    }) else {
        return Ok(None);
    };

    let asset = client
        .assets
        .iter()
        .find(|asset| asset.platform == selector.platform)
        .cloned()
        .ok_or_else(|| {
            format!(
                "manifest has no asset for client {} channel {} platform {}",
                selector.client_id, selector.channel, selector.platform
            )
        })?;

    Ok(Some(ClientUpdateCheck {
        client_id: client.client_id.clone(),
        channel: client.channel.clone(),
        current_version: None,
        latest_version: client.version.clone(),
        asset,
        needs_update: true,
        source_kind: UpdateSourceKind::Manifest,
        action: UpdateAction::Download,
        action_url: None,
        message: None,
    }))
}

fn validate_manifest_schema(manifest: &UpdateManifest) -> Result<(), String> {
    for client in &manifest.clients {
        if client.client_id.trim().is_empty() {
            return Err("manifest client_id must not be empty".to_string());
        }
        if client.channel.trim().is_empty() {
            return Err("manifest client channel must not be empty".to_string());
        }
        if client.version.trim().is_empty() {
            return Err("manifest client version must not be empty".to_string());
        }
        if client.release_notes.trim().is_empty() {
            return Err("manifest client release_notes must not be empty".to_string());
        }
        if client.assets.is_empty() {
            return Err("manifest client assets must not be empty".to_string());
        }

        for asset in &client.assets {
            if asset.platform.trim().is_empty() {
                return Err("manifest asset platform must not be empty".to_string());
            }
            if asset.asset_url.trim().is_empty() {
                return Err("manifest asset_url must not be empty".to_string());
            }
            if asset.sha256.trim().is_empty() {
                return Err("manifest asset sha256 must not be empty".to_string());
            }
            if !is_sha256_hex(&asset.sha256) {
                return Err("manifest asset sha256 must be 64 ASCII hex chars".to_string());
            }
            if asset.size == 0 {
                return Err("manifest asset size must be greater than 0".to_string());
            }

            let asset_url = Url::parse(&asset.asset_url)
                .map_err(|error| format!("invalid manifest asset_url: {error}"))?;
            validate_asset_url(&asset_url)?;
        }
    }

    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_manifest_url(url: &Url) -> Result<(), String> {
    if url
        .host_str()
        .is_some_and(|host| local_smoke::allows_local_smoke_url(url.scheme(), host))
    {
        return Ok(());
    }

    let host = validate_public_https_url(url)?;
    validate_trusted_host(
        &host,
        TRUSTED_MANIFEST_HOSTS,
        "manifest url host is not trusted",
    )
}

fn validate_asset_url(url: &Url) -> Result<(), String> {
    if url
        .host_str()
        .is_some_and(|host| local_smoke::allows_local_smoke_url(url.scheme(), host))
    {
        return Ok(());
    }

    let host = validate_public_https_url(url)?;
    validate_trusted_host(
        &host,
        TRUSTED_ASSET_HOSTS,
        "manifest asset_url host is not trusted",
    )
}

fn build_proxy_prefix_url(original: &Url, route: &NetworkRouteConfig) -> Result<Url, String> {
    let prefix = route
        .proxy_prefix_url
        .as_deref()
        .ok_or_else(|| "proxy prefix url is required".to_string())?;
    let separator = if prefix.ends_with('/') { "" } else { "/" };
    let final_url = format!(
        "{prefix}{separator}{}",
        percent_encode_url(original.as_str())
    );
    parse_network_route_url(&final_url, route)
}

fn build_mirror_template_url(original: &Url, route: &NetworkRouteConfig) -> Result<Url, String> {
    let template = route
        .mirror_template
        .as_deref()
        .ok_or_else(|| "mirror template is required".to_string())?;
    if !template.contains("{url}") {
        return Err("mirror template must contain {url}".to_string());
    }

    let final_url = template.replace("{url}", original.as_str());
    parse_network_route_url(&final_url, route)
}

fn parse_network_route_url(input: &str, route: &NetworkRouteConfig) -> Result<Url, String> {
    if local_smoke::has_ambiguous_numeric_url_host(input) {
        return Err("manifest url host must be public".to_string());
    }

    let url = Url::parse(input).map_err(|error| format!("invalid manifest url: {error}"))?;
    let host = validate_public_https_url(&url)?;
    validate_enabled_route_host(&host, &route.enabled_hosts)?;
    Ok(url)
}

fn validate_enabled_route_host(host: &str, enabled_hosts: &[String]) -> Result<(), String> {
    if enabled_hosts
        .iter()
        .any(|enabled| enabled.trim_end_matches('.').eq_ignore_ascii_case(host))
    {
        Ok(())
    } else {
        Err("network route host is not enabled".to_string())
    }
}

fn percent_encode_url(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(char::from(byte));
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn validate_public_https_url(url: &Url) -> Result<String, String> {
    if url.scheme() != "https" {
        return Err("manifest url must use https".to_string());
    }

    let host = url
        .host_str()
        .ok_or_else(|| "manifest url must include host".to_string())?;
    let lower_host = host.trim_end_matches('.').to_ascii_lowercase();
    if lower_host == "localhost"
        || lower_host.ends_with(".localhost")
        || local_smoke::is_ambiguous_numeric_host(host)
    {
        return Err("manifest url host must be public".to_string());
    }

    let ip_host = normalized_ip_host(host);
    if let Ok(ip) = ip_host.parse::<IpAddr>() {
        validate_public_ip(ip)?;
    }

    Ok(lower_host)
}

fn validate_trusted_host(host: &str, allowed_hosts: &[&str], error: &str) -> Result<(), String> {
    if allowed_hosts.contains(&host) {
        return Ok(());
    }

    Err(error.to_string())
}

fn validate_public_ip(ip: IpAddr) -> Result<(), String> {
    let is_blocked = match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() || ipv4.is_unspecified()
        }
        IpAddr::V6(ipv6) => {
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                return validate_public_ip(IpAddr::V4(ipv4));
            }

            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || is_ipv6_unique_local(&ipv6)
                || is_ipv6_unicast_link_local(&ipv6)
        }
    };

    if is_blocked {
        return Err("manifest url host must be public".to_string());
    }

    Ok(())
}

fn normalized_ip_host(host: &str) -> &str {
    host.trim_start_matches('[').trim_end_matches(']')
}

fn is_ipv6_unique_local(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_ipv6_unicast_link_local(ip: &Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

async fn read_limited_manifest_response(mut response: reqwest::Response) -> Result<String, String> {
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MANIFEST_BYTES as u64)
    {
        return Err(format!(
            "manifest response exceeds {MAX_MANIFEST_BYTES} bytes"
        ));
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("failed to read manifest response: {error}"))?
    {
        if bytes.len() + chunk.len() > MAX_MANIFEST_BYTES {
            return Err(format!(
                "manifest response exceeds {MAX_MANIFEST_BYTES} bytes"
            ));
        }
        bytes.extend_from_slice(&chunk);
    }

    String::from_utf8(bytes)
        .map_err(|error| format!("manifest response is not valid UTF-8: {error}"))
}

#[cfg(test)]
#[path = "test/manifest.rs"]
mod tests;
