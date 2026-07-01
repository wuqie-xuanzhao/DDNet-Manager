use crate::error::ManagerError;
use crate::local_smoke;
use crate::models::{
    ClientUpdateCheck, ClientUpdateSelector, NetworkRouteConfig, UpdateAction, UpdateCheckReason,
    UpdateManifest, UpdateSourceKind,
};
use reqwest::Url;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
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
///
/// 本地代理模式不改写 URL（代理在 reqwest 客户端层注入），仅复用 manifest 层
/// 的 SSRF 校验确保目标 host 公开且可信。保留 route 参数供调用方语义对齐。
pub fn build_manifest_url_with_route(
    url: &str,
    _route: Option<&NetworkRouteConfig>,
) -> Result<Url, String> {
    build_manifest_url(url)
}

/// 从远程地址拉取更新 manifest，并复用本地解析校验逻辑。
///
/// 不带缓存目录，用于测试或无需持久化缓存的场景。
pub async fn fetch_manifest(url: &str) -> Result<UpdateManifest, String> {
    fetch_manifest_with_route(url, None, None).await
}

/// 使用显式网络路由配置从远程地址拉取更新 manifest。
///
/// 本地代理模式通过 reqwest 客户端层注入代理隧道，URL 本身不改写；目标 host
/// 仍由 build_manifest_url 校验为公开可信地址。
///
/// 成功后把 manifest 原文写入本地缓存；网络失败时若缓存存在则返回缓存版本
/// （日志记录 fallback），让离线或网络抖动时仍能基于最近一次成功拉取的 manifest
/// 执行更新检查。
pub async fn fetch_manifest_with_route(
    url: &str,
    route: Option<&NetworkRouteConfig>,
    cache_dir: Option<&Path>,
) -> Result<UpdateManifest, String> {
    let final_url = build_manifest_url_with_route(url, route)?;
    let cache_path = cache_dir.map(|dir| manifest_cache_path(dir, url));

    let fetch_result = async {
        let client = crate::network_route::build_routed_client(
            route,
            Some(Duration::from_secs(15)),
            None,
            true,
        )?;
        let response = client
            .get(final_url)
            .send()
            .await
            .and_then(|response| response.error_for_status())
            .map_err(|error| format!("failed to fetch manifest: {error}"))?;
        read_limited_manifest_response(response).await
    }
    .await;

    match fetch_result {
        Ok(text) => {
            // 写缓存不影响主流程，失败只打日志。
            if let Some(path) = &cache_path {
                if let Err(error) = write_manifest_cache(path, &text) {
                    eprintln!(
                        "failed to write manifest cache at {}: {error}",
                        path.display()
                    );
                }
            }
            parse_manifest(&text)
        }
        Err(network_error) => {
            // 网络失败 → 回退到本地缓存。
            if let Some(path) = &cache_path {
                if let Ok(cached_text) = std::fs::read_to_string(path) {
                    eprintln!(
                        "manifest fetch failed, falling back to cached manifest at {}: {network_error}",
                        path.display()
                    );
                    return parse_manifest(&cached_text).map_err(|parse_error| {
                        format!(
                            "cached manifest is corrupted ({parse_error}); original network error: {network_error}"
                        )
                    });
                }
            }
            Err(network_error)
        }
    }
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
        reason: UpdateCheckReason::None,
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

    let host = validate_public_https_url(url).map_err(|error| error.to_string())?;
    validate_trusted_host(
        &host,
        TRUSTED_MANIFEST_HOSTS,
        "manifest url host is not trusted",
    )
    .map_err(|error| error.to_string())
}

fn validate_asset_url(url: &Url) -> Result<(), String> {
    if url
        .host_str()
        .is_some_and(|host| local_smoke::allows_local_smoke_url(url.scheme(), host))
    {
        return Ok(());
    }

    let host = validate_public_https_url(url).map_err(|error| error.to_string())?;
    validate_trusted_host(
        &host,
        TRUSTED_ASSET_HOSTS,
        "manifest asset_url host is not trusted",
    )
    .map_err(|error| error.to_string())
}

/// 校验 manifest URL 使用公开 HTTPS、非 loopback / 私网 host。
///
/// 返回 [`ManagerError`]，让稳定错误码（`network_https_required` /
/// `network_host_not_trusted`）能直接传播到 IPC 边界，避免在 String 上下文中
/// 被运行时字符串匹配重新分类。
fn validate_public_https_url(url: &Url) -> Result<String, ManagerError> {
    if url.scheme() != "https" {
        return Err(ManagerError::NetworkHttpsRequired(
            "manifest url must use https".to_string(),
        ));
    }

    let host = url
        .host_str()
        .ok_or_else(|| ManagerError::Internal("manifest url must include host".to_string()))?;
    let lower_host = host.trim_end_matches('.').to_ascii_lowercase();
    if lower_host == "localhost"
        || lower_host.ends_with(".localhost")
        || local_smoke::is_ambiguous_numeric_host(host)
    {
        return Err(ManagerError::NetworkHostNotTrusted(
            "manifest url host must be public".to_string(),
        ));
    }

    let ip_host = normalized_ip_host(host);
    if let Ok(ip) = ip_host.parse::<IpAddr>() {
        if local_smoke::validate_public_ip(ip).is_err() {
            return Err(ManagerError::NetworkHostNotTrusted(
                "manifest url host must be public".to_string(),
            ));
        }
    }

    Ok(lower_host)
}

/// 校验 host 是否在受信任白名单中。
///
/// 返回 [`ManagerError::NetworkHostNotTrusted`] 让稳定错误码在 IPC 边界保持
/// 编译期映射。
fn validate_trusted_host(
    host: &str,
    allowed_hosts: &[&str],
    error: &str,
) -> Result<(), ManagerError> {
    if allowed_hosts.contains(&host) {
        return Ok(());
    }

    Err(ManagerError::NetworkHostNotTrusted(error.to_string()))
}

fn normalized_ip_host(host: &str) -> &str {
    host.trim_start_matches('[').trim_end_matches(']')
}

/// 根据 manifest URL 计算本地缓存路径，文件名用 URL 的 FNV-1a 64 位哈希。
///
/// 放在 `<cache_dir>/manifests/` 子目录下，避免与其他缓存文件混在一起。
/// URL 中可能含 query string，但完整 URL 参与哈希能区分不同参数的 manifest。
fn manifest_cache_path(cache_dir: &Path, url: &str) -> PathBuf {
    let hash = fnv1a_64(url.as_bytes());
    cache_dir
        .join("manifests")
        .join(format!("{hash:016x}.json"))
}

/// FNV-1a 64 位哈希，用于把 URL 映射成稳定的短文件名。
///
/// 不引入额外依赖，直接手写 FNV-1a 算法，足够用于缓存键计算。
fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 把 manifest 原文写入本地缓存文件，父目录不存在则自动创建。
fn write_manifest_cache(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create manifest cache dir: {error}"))?;
    }
    std::fs::write(path, content)
        .map_err(|error| format!("failed to write manifest cache: {error}"))
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

/// 测试辅助模块，暴露内部缓存函数供单元测试直接调用。
#[cfg(test)]
pub mod test_helpers {
    use super::*;

    /// 计算给定 URL 在缓存目录下的 manifest 缓存路径。
    pub fn manifest_cache_path(cache_dir: &Path, url: &str) -> PathBuf {
        super::manifest_cache_path(cache_dir, url)
    }

    /// 把 manifest 原文写入指定缓存路径，自动创建父目录。
    pub fn write_manifest_cache(path: &Path, content: &str) -> Result<(), String> {
        super::write_manifest_cache(path, content)
    }
}
