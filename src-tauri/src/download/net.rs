//! 下载 URL 校验、HTTP 请求重定向跟随、流式写入与缓存清理。

use crate::error::ManagerError;
use crate::local_smoke;
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
/// `extra_hosts` 补充基线 [`TRUSTED_DOWNLOAD_HOSTS`]，让公共反代域名按配置动态放行；
/// SSRF 防护仍保留：拒绝 localhost / 私网 IP / 歧义数字 host。
///
/// 返回 [`ManagerError`]，让 `network_https_required` / `network_host_not_trusted`
/// 等稳定错误码在 IPC 边界保持编译期映射，而不是被 String 重新分类。
pub(crate) fn validate_download_url(url: &str, extra_hosts: &[String]) -> Result<(), ManagerError> {
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
    if !is_host_trusted(&normalized_host, extra_hosts) {
        return Err(ManagerError::NetworkHostNotTrusted(
            "download url host is not trusted".to_string(),
        ));
    }
    Ok(())
}

/// 判断 host 是否可信：命中基线 [`TRUSTED_DOWNLOAD_HOSTS`] 或用户配置的 `extra_hosts`。
///
/// 比较大小写不敏感；`extra_hosts` 条目会 trim 后比较。保留 SSRF 防护——
/// 调用方仍应通过 [`validate_download_url`] 拒绝私网 IP 和 localhost。
pub(crate) fn is_host_trusted(host: &str, extra_hosts: &[String]) -> bool {
    if TRUSTED_DOWNLOAD_HOSTS.contains(&host) {
        return true;
    }
    extra_hosts
        .iter()
        .any(|h| h.trim().eq_ignore_ascii_case(host))
}

/// 跟随 HTTP 重定向（上限 [`MAX_DOWNLOAD_REDIRECTS`]）并发起下载请求。
///
/// `start_offset > 0` 时附加 `Range: bytes={start_offset}-` header，用于从竞速测速
/// 字节之后续传。`extra_hosts` 透传给 [`validate_download_url`] 校验每个重定向跳。
pub(crate) async fn send_download_request(
    client: &reqwest::Client,
    asset_url: &str,
    extra_hosts: &[String],
    start_offset: u64,
) -> Result<reqwest::Response, ManagerError> {
    let mut current_url = Url::parse(asset_url)
        .map_err(|error| ManagerError::Internal(format!("invalid download url: {error}")))?;

    for _ in 0..=MAX_DOWNLOAD_REDIRECTS {
        validate_download_url(current_url.as_str(), extra_hosts)?;
        let mut request = client.get(current_url.clone());
        if start_offset > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={start_offset}-"));
        }
        let response = request.send().await.map_err(|error| {
            ManagerError::Internal(format!("failed to download update asset: {error}"))
        })?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| {
                    ManagerError::Internal("download redirect missing Location header".to_string())
                })?
                .to_str()
                .map_err(|error| {
                    ManagerError::Internal(format!(
                        "download redirect Location is invalid: {error}"
                    ))
                })?;
            current_url = current_url.join(location).map_err(|error| {
                ManagerError::Internal(format!("download redirect Location is invalid: {error}"))
            })?;
            continue;
        }

        return response.error_for_status().map_err(|error| {
            ManagerError::Internal(format!("failed to download update asset: {error}"))
        });
    }

    Err(ManagerError::Internal(format!(
        "download redirected more than {MAX_DOWNLOAD_REDIRECTS} times"
    )))
}

/// 下载远程资产到缓存文件，并通过回调报告已下载字节数。
///
/// 流程：
/// 1. 对 `candidate_urls` 做 SSRF 前置过滤（`validate_download_url` + `extra_hosts`），
///    淘汰私网/不可信 host。安全候选多于 1 个时调 [`race::select_best_source`] 并发
///    竞速选最快源；否则退化为用原始 `asset_url` 单源直连。竞速胜出的前 5 MiB
///    字节复用为 `.partial` 首段。
/// 2. 对胜出 URL 再次做 SSRF 校验（含 `extra_hosts` 动态放行反代域名）。
/// 3. 创建 `.part` 文件，写入竞速 `head_start` 字节。
/// 4. 用 `follow_redirects=false` 的 client 对胜出 URL 做 Range 续传（从 `head_start` 之后）。
/// 5. 流式写入 `.part`，完成后 rename 为最终缓存路径。
///
/// 两路取消协作：
/// - `on_progress` 返回 `false` 时立即终止下载并清理 `.part` 临时文件。
/// - `cancel.cancelled()` 触发时立即终止下载，不必等到下一个 chunk。
///
/// 两者任一触发都会返回 `"download canceled"` 错误，由调用方决定如何处理。
pub async fn download_asset_to_file<F>(
    request: super::DownloadFileRequest<'_>,
    cancel: Option<CancellationToken>,
    mut on_progress: F,
) -> Result<(), ManagerError>
where
    F: FnMut(u64) -> bool + Send,
{
    let part_path = part_file_path(request.cache_path);
    if let Some(parent) = request.cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ManagerError::Internal(format!("failed to create download cache dir: {error}"))
        })?;
    }
    if part_path.exists() {
        fs::remove_file(&part_path).map_err(|error| {
            ManagerError::Internal(format!("failed to clear partial download file: {error}"))
        })?;
    }

    // 阶段1：竞速选源（安全候选 >1 时）或单源直连退化
    let winner = select_download_winner(&request, cancel.as_ref()).await?;

    // 阶段2：对胜出 URL 做 SSRF 校验
    validate_download_url(&winner.url, request.extra_hosts)?;

    check_cancel_or_cleanup(cancel.as_ref(), &part_path)?;

    // 阶段3：写入 head_start（测速字节复用为 .partial 首段）
    let start_offset = winner.head_start.len() as u64;
    {
        let mut file = fs::File::create(&part_path).map_err(|error| {
            ManagerError::Internal(format!("failed to create download cache file: {error}"))
        })?;
        if !winner.head_start.is_empty() {
            file.write_all(&winner.head_start).map_err(|error| {
                ManagerError::Internal(format!("failed to write race head start: {error}"))
            })?;
        }
    }

    // head_start 已是完整文件（小文件场景）：直接 finalize
    if start_offset >= request.expected_size {
        return finalize_download_cache(request.cache_path, &part_path);
    }

    // 阶段4：用 follow_redirects=false 的 client 做 Range 续传
    let download_client =
        crate::network_route::build_routed_client(request.route, None, None, false)?;
    // review C2：send_download_request 失败（连接/TLS/重定向超限/4xx5xx）时
    // 清理已写入 head_start 的 .part 文件，避免临时文件泄漏。
    let mut response = send_download_request(
        &download_client,
        &winner.url,
        request.extra_hosts,
        start_offset,
    )
    .await
    .inspect_err(|_error| {
        let _ = fs::remove_file(&part_path);
    })?;

    check_cancel_or_cleanup(cancel.as_ref(), &part_path)?;
    validate_download_response(&response, start_offset, request.expected_size, &part_path)?;

    // 阶段5：流式写入 .part，完成后 rename 为最终缓存路径
    let mut control = DownloadControl {
        cancel: cancel.as_ref(),
        on_progress: &mut on_progress,
    };
    stream_download_to_partfile(&request, &mut response, &mut control, start_offset).await?;
    finalize_download_cache(request.cache_path, &part_path)
}

/// 检查 cancel 信号；已取消时清理 `.part` 文件并返回 `"download canceled"` 错误。
fn check_cancel_or_cleanup(
    cancel: Option<&CancellationToken>,
    part_path: &Path,
) -> Result<(), ManagerError> {
    if cancel.is_some_and(|token| token.is_cancelled()) {
        let _ = fs::remove_file(part_path);
        return Err(ManagerError::Internal("download canceled".to_string()));
    }
    Ok(())
}

/// 校验下载响应：续传场景必须返回 206，非续传场景 Content-Length 必须匹配 expected_size。
///
/// review C7：续传场景服务器必须返回 206 Partial Content；若忽略 Range 返回 200（整个
/// 文件），继续写入会导致 head_start + 完整文件超出 expected_size。校验失败时清理
/// `.part` 文件并返回错误。
fn validate_download_response(
    response: &reqwest::Response,
    start_offset: u64,
    expected_size: u64,
    part_path: &Path,
) -> Result<(), ManagerError> {
    if start_offset > 0 && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        let _ = fs::remove_file(part_path);
        return Err(ManagerError::Internal(
            "server ignored Range request (expected 206 Partial Content)".to_string(),
        ));
    }
    if start_offset == 0
        && response
            .content_length()
            .is_some_and(|length| length != expected_size)
    {
        let _ = fs::remove_file(part_path);
        return Err(ManagerError::Internal(
            "download content length does not match manifest size".to_string(),
        ));
    }
    Ok(())
}

/// 阶段1：对候选 URL 做 SSRF 前置过滤，安全候选 >1 时竞速选源，否则单源退化。
///
/// review S1/C4：SSRF 校验在调用方完成而非 race 模块内部，避免 race 模块依赖
/// local_smoke 测试开关（local_smoke 的 thread-local override 与 async 不兼容，
/// 无法在 mockito 127.0.0.1 集成测试中放行）。竞速 client 用 `follow_redirects=false`，
/// 避免已校验 host 经 302 重定向到内网；反代通常不重定向，GitHub 原始 URL 若被
/// 302 淘汰，竞速失败后 fallback 到原始 `asset_url` 单源直连（走 `send_download_request`
/// 手动逐跳校验）。
///
/// - 安全候选 >1：调 [`race::select_best_source`] 竞速，失败时 fallback 到 `asset_url`。
/// - 安全候选 <=1 或全部被 SSRF 拒绝：直接用 `asset_url`，无竞速、无 head_start。
///   单候选不竞速是因为无择优意义，且 `send_download_request` 手动逐跳重定向比
///   竞速 client 的 `follow_redirects=false` 更适合单源场景（GitHub 302 → 最终资产）。
async fn select_download_winner(
    request: &super::DownloadFileRequest<'_>,
    cancel: Option<&CancellationToken>,
) -> Result<crate::download::race::RaceWinner, ManagerError> {
    let safe_candidates: Vec<String> = request
        .candidate_urls
        .iter()
        .filter(|url| validate_download_url(url, request.extra_hosts).is_ok())
        .cloned()
        .collect();

    if safe_candidates.len() > 1 {
        let race_client =
            crate::network_route::build_routed_client(request.route, None, None, false)?;
        Ok(crate::download::race::select_best_source(
            &race_client,
            &safe_candidates,
            request.expected_size,
            cancel,
        )
        .await
        .unwrap_or_else(|_| crate::download::race::RaceWinner {
            url: request.asset_url.to_string(),
            head_start: Vec::new(),
        }))
    } else {
        // 安全候选 <= 1 或全部被 SSRF 拒绝：直接用 asset_url，无竞速、无 head_start。
        // asset_url 仍会在阶段2 经过 validate_download_url 校验，不可信时返回错误。
        Ok(crate::download::race::RaceWinner {
            url: request.asset_url.to_string(),
            head_start: Vec::new(),
        })
    }
}

/// 下载流式写入的运行时控制上下文：cancel 信号 + on_progress 回调。
///
/// 把这两个紧耦合的流式写入控制参数封装在一起，避免 [`stream_download_to_partfile`]
/// 参数超过 4 个（CLAUDE.md 规约：函数参数超过 4 个优先封装为结构体）。
struct DownloadControl<'a, F>
where
    F: FnMut(u64) -> bool + Send,
{
    cancel: Option<&'a CancellationToken>,
    on_progress: &'a mut F,
}

/// 流式写入 `.part` 文件，监听 cancel 与 on_progress 信号；任一触发立即清理并返回错误。
///
/// `start_offset > 0` 时以 append 模式打开已存在的 `.part`（已由调用方写入竞速
/// head_start 字节）；`start_offset == 0` 时覆盖创建。size 守卫以
/// `start_offset + downloaded` 与 `expected_size` 比对。
async fn stream_download_to_partfile<F>(
    request: &super::DownloadFileRequest<'_>,
    response: &mut reqwest::Response,
    control: &mut DownloadControl<'_, F>,
    start_offset: u64,
) -> Result<(), ManagerError>
where
    F: FnMut(u64) -> bool + Send,
{
    let part_path = part_file_path(request.cache_path);
    let mut file = if start_offset > 0 {
        std::fs::OpenOptions::new()
            .append(true)
            .open(&part_path)
            .map_err(|error| {
                ManagerError::Internal(format!("failed to open partial download file: {error}"))
            })?
    } else {
        fs::File::create(&part_path).map_err(|error| {
            ManagerError::Internal(format!("failed to create download cache file: {error}"))
        })?
    };
    let mut downloaded = start_offset;

    loop {
        // 在等待下一个 chunk 的同时监听 cancel，让取消信号在 chunk 边界之间也能即时生效。
        let next_chunk = match control.cancel {
            Some(token) => {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => {
                        let _ = fs::remove_file(part_path);
                        return Err(ManagerError::Internal("download canceled".to_string()));
                    }
                    chunk = response.chunk() => chunk,
                }
            }
            None => response.chunk().await,
        };
        let next_chunk = match next_chunk {
            Ok(chunk) => chunk,
            Err(error) => {
                let _ = fs::remove_file(part_path);
                return Err(ManagerError::Internal(format!(
                    "failed to read download stream: {error}"
                )));
            }
        };
        let Some(chunk) = next_chunk else {
            // review C9：正常 EOF 时校验已下载字节数与期望 size 一致，提前暴露
            // 服务器断流问题，而非依赖后续 SHA-256 兜底。
            if downloaded != request.expected_size {
                let _ = fs::remove_file(part_path);
                return Err(ManagerError::Internal(format!(
                    "downloaded bytes {downloaded} do not match manifest size {}",
                    request.expected_size
                )));
            }
            break;
        };
        downloaded = match downloaded.checked_add(chunk.len() as u64) {
            Some(value) => value,
            None => {
                let _ = fs::remove_file(part_path);
                return Err(ManagerError::Internal(
                    "downloaded byte count overflow".to_string(),
                ));
            }
        };
        if downloaded > request.expected_size {
            let _ = fs::remove_file(part_path);
            return Err(ManagerError::Internal(
                "downloaded bytes exceed manifest size".to_string(),
            ));
        }
        if let Err(error) = file.write_all(&chunk) {
            let _ = fs::remove_file(part_path);
            return Err(ManagerError::Internal(format!(
                "failed to write download cache file: {error}"
            )));
        }
        if !(control.on_progress)(downloaded) {
            let _ = fs::remove_file(part_path);
            return Err(ManagerError::Internal("download canceled".to_string()));
        }
    }

    Ok(())
}

/// 关闭 `.part` 文件句柄，原子重命名为最终缓存路径。
fn finalize_download_cache(cache_path: &Path, part_path: &Path) -> Result<(), ManagerError> {
    if cache_path.exists() {
        fs::remove_file(cache_path).map_err(|error| {
            ManagerError::Internal(format!("failed to replace existing cache file: {error}"))
        })?;
    }
    fs::rename(part_path, cache_path).map_err(|error| {
        ManagerError::Internal(format!("failed to finalize download cache file: {error}"))
    })
}

/// 清理下载缓存目录中超过 TTL 的缓存文件和临时分片文件。
///
/// 返回已清理的文件数量。仅清理下载缓存目录（`downloads_dir`）内的文件，
/// 不删除子目录本身。注意：当前仅按修改时间判断，不检查是否有活跃下载任务引用；
pub fn cleanup_expired_cache_files(
    downloads_dir: &Path,
    ttl_days: Option<i64>,
) -> Result<usize, ManagerError> {
    if !downloads_dir.exists() {
        return Ok(0);
    }
    let ttl = ttl_days.unwrap_or(DEFAULT_CACHE_TTL_DAYS);
    let cutoff = OffsetDateTime::now_utc()
        .checked_sub(time::Duration::days(ttl))
        .ok_or_else(|| ManagerError::Internal("invalid TTL days".to_string()))?;
    let cutoff_unix = cutoff.unix_timestamp();

    let entries = fs::read_dir(downloads_dir).map_err(|error| {
        ManagerError::Internal(format!("failed to read downloads dir: {error}"))
    })?;
    let mut removed = 0usize;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ManagerError::Internal(format!("failed to read cache entry: {error}"))
        })?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| {
            ManagerError::Internal(format!("failed to read cache metadata: {error}"))
        })?;
        let modified = metadata.modified().map_err(|error| {
            ManagerError::Internal(format!("failed to read cache mtime: {error}"))
        })?;
        let modified_unix = modified
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| ManagerError::Internal(format!("cache mtime before epoch: {error}")))?
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
