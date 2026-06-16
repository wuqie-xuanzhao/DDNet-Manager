//! 下载 URL 校验、HTTP 请求重定向跟随、流式写入与缓存清理。

use crate::local_smoke;
use crate::models::ManagerError;
use reqwest::Url;
use std::fs;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use tokio_util::sync::CancellationToken;

/// HTTP 重定向上限，避免客户端陷入无限循环。
const MAX_DOWNLOAD_REDIRECTS: usize = 5;

/// 受信任的下载资产 host 白名单；其他 host 一律拒绝直连。
const TRUSTED_DOWNLOAD_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
    "raw.githubusercontent.com",
    "ddnet.org",
];

/// 下载缓存文件默认保留天数；超过此期限的缓存文件可被清理。
const DEFAULT_CACHE_TTL_DAYS: i64 = 30;

/// 校验下载 URL，并允许用户显式启用的代理或镜像 host。
///
/// 返回 [`ManagerError`]，让 `network_https_required` / `network_host_not_trusted`
/// 等稳定错误码在 IPC 边界保持编译期映射，而不是被 String 重新分类。
pub(crate) fn validate_download_url(url: &str) -> Result<(), ManagerError> {
    if local_smoke::has_ambiguous_numeric_url_host(url) {
        return Err(ManagerError::NetworkHostNotTrusted(
            "download url host must be public".to_string(),
        ));
    }

    let parsed = Url::parse(url)
        .map_err(|error| ManagerError::Internal(format!("invalid download url: {error}")))?;
    let scheme = parsed.scheme();
    let host = parsed
        .host_str()
        .ok_or_else(|| ManagerError::Internal("download url must include host".to_string()))?;
    if local_smoke::is_ambiguous_numeric_host(host) {
        return Err(ManagerError::NetworkHostNotTrusted(
            "download url host must be public".to_string(),
        ));
    }
    if local_smoke::allows_local_smoke_url(scheme, host) {
        return Ok(());
    }
    if scheme != "https" {
        return Err(ManagerError::NetworkHttpsRequired(
            "download url must use https".to_string(),
        ));
    }
    let normalized_host = host.trim_end_matches('.').to_ascii_lowercase();
    if normalized_host == "localhost" || normalized_host.ends_with(".localhost") {
        return Err(ManagerError::NetworkHostNotTrusted(
            "download url host must be public".to_string(),
        ));
    }
    let ip_host = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = ip_host.parse::<IpAddr>() {
        if local_smoke::validate_public_ip(ip).is_err() {
            return Err(ManagerError::NetworkHostNotTrusted(
                "download url host must be public".to_string(),
            ));
        }
    }
    if !TRUSTED_DOWNLOAD_HOSTS.contains(&normalized_host.as_str()) {
        return Err(ManagerError::NetworkHostNotTrusted(
            "download url host is not trusted".to_string(),
        ));
    }
    Ok(())
}

/// 跟随 HTTP 重定向（上限 [`MAX_DOWNLOAD_REDIRECTS`]）并发起下载请求。
pub(crate) async fn send_download_request(
    client: &reqwest::Client,
    asset_url: &str,
) -> Result<reqwest::Response, String> {
    let mut current_url =
        Url::parse(asset_url).map_err(|error| format!("invalid download url: {error}"))?;

    for _ in 0..=MAX_DOWNLOAD_REDIRECTS {
        validate_download_url(current_url.as_str()).map_err(|error| error.to_string())?;
        let response = client
            .get(current_url.clone())
            .send()
            .await
            .map_err(|error| format!("failed to download update asset: {error}"))?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| "download redirect missing Location header".to_string())?
                .to_str()
                .map_err(|error| format!("download redirect Location is invalid: {error}"))?;
            current_url = current_url
                .join(location)
                .map_err(|error| format!("download redirect Location is invalid: {error}"))?;
            continue;
        }

        return response
            .error_for_status()
            .map_err(|error| format!("failed to download update asset: {error}"));
    }

    Err(format!(
        "download redirected more than {MAX_DOWNLOAD_REDIRECTS} times"
    ))
}

/// 下载远程资产到缓存文件，并通过回调报告已下载字节数。
///
/// 两路取消协作：
/// - `on_progress` 返回 `false` 时立即终止下载并清理 `.part` 临时文件
///   （兼容旧的 chunk 边界取消）。
/// - `cancel.cancelled()` 触发时立即终止下载，不必等到下一个 chunk。
///
/// 两者任一触发都会返回 `"download canceled"` 错误，由调用方决定如何处理。
pub async fn download_asset_to_file<F>(
    request: super::DownloadFileRequest<'_>,
    cancel: Option<CancellationToken>,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64) -> bool + Send,
{
    validate_download_url(request.asset_url).map_err(|error| error.to_string())?;
    let part_path = part_file_path(request.cache_path);
    if let Some(parent) = request.cache_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create download cache dir: {error}"))?;
    }
    if part_path.exists() {
        fs::remove_file(&part_path)
            .map_err(|error| format!("failed to clear partial download file: {error}"))?;
    }

    if let Some(token) = cancel.as_ref() {
        if token.is_cancelled() {
            return Err("download canceled".to_string());
        }
    }

    let client = crate::network_route::build_routed_client(request.route, None, None, false)?;
    let response = send_download_request(&client, request.asset_url).await?;

    if cancel.as_ref().is_some_and(|token| token.is_cancelled()) {
        return Err("download canceled".to_string());
    }

    if response
        .content_length()
        .is_some_and(|length| length != request.expected_size)
    {
        return Err("download content length does not match manifest size".to_string());
    }

    let mut response = response;
    let mut file = fs::File::create(&part_path)
        .map_err(|error| format!("failed to create download cache file: {error}"))?;
    let mut downloaded = 0_u64;

    loop {
        // 在等待下一个 chunk 的同时监听 cancel，让取消信号在 chunk 边界之间也能即时生效。
        let next_chunk = match cancel.as_ref() {
            Some(token) => {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        let _ = fs::remove_file(&part_path);
                        return Err("download canceled".to_string());
                    }
                    chunk = response.chunk() => chunk,
                }
            }
            None => response.chunk().await,
        };
        let next_chunk = match next_chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                let _ = fs::remove_file(&part_path);
                return Err(format!("failed to read download stream: {error}"));
            }
        };
        let Some(chunk) = next_chunk else {
            break;
        };
        downloaded = match downloaded.checked_add(chunk.len() as u64) {
            Some(value) => value,
            None => {
                let _ = fs::remove_file(&part_path);
                return Err("downloaded byte count overflow".to_string());
            }
        };
        if downloaded > request.expected_size {
            let _ = fs::remove_file(&part_path);
            return Err("downloaded bytes exceed manifest size".to_string());
        }
        if let Err(error) = file.write_all(&chunk) {
            let _ = fs::remove_file(&part_path);
            return Err(format!("failed to write download cache file: {error}"));
        }
        if !on_progress(downloaded) {
            let _ = fs::remove_file(&part_path);
            return Err("download canceled".to_string());
        }
    }

    drop(file);
    if request.cache_path.exists() {
        fs::remove_file(request.cache_path)
            .map_err(|error| format!("failed to replace existing cache file: {error}"))?;
    }
    fs::rename(&part_path, request.cache_path)
        .map_err(|error| format!("failed to finalize download cache file: {error}"))
}

/// 清理下载缓存目录中超过 TTL 的缓存文件和临时分片文件。
///
/// 返回已清理的文件数量。仅清理下载缓存目录（`downloads_dir`）内的文件，
/// 不删除子目录本身。注意：当前仅按修改时间判断，不检查是否有活跃下载任务引用；
pub fn cleanup_expired_cache_files(
    downloads_dir: &Path,
    ttl_days: Option<i64>,
) -> Result<usize, String> {
    if !downloads_dir.exists() {
        return Ok(0);
    }
    let ttl = ttl_days.unwrap_or(DEFAULT_CACHE_TTL_DAYS);
    let cutoff = OffsetDateTime::now_utc()
        .checked_sub(time::Duration::days(ttl))
        .ok_or_else(|| "invalid TTL days".to_string())?;
    let cutoff_unix = cutoff.unix_timestamp();

    let entries = fs::read_dir(downloads_dir)
        .map_err(|error| format!("failed to read downloads dir: {error}"))?;
    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read cache entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("failed to read cache metadata: {error}"))?;
        let modified = metadata
            .modified()
            .map_err(|error| format!("failed to read cache mtime: {error}"))?;
        let modified_unix = modified
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("cache mtime before epoch: {error}"))?
            .as_secs() as i64;
        if modified_unix < cutoff_unix && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

/// 把路径中的反斜杠统一成正斜杠，与持久化在注册表 / manifest 中的格式对齐。
pub(crate) fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// 返回下载缓存文件对应的 `.part` 临时文件路径，下载完成后 rename 为最终路径。
pub(crate) fn part_file_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download.zip");
    path.with_file_name(format!("{file_name}.part"))
}
