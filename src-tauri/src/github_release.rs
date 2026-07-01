use crate::client_catalog::ClientCatalogEntry;
use crate::error::ManagerError;
use crate::models::{NetworkRouteConfig, UpdateAsset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

const GITHUB_API_BASE: &str = "https://api.github.com/repos";

/// 返回带当前包版本号的 User-Agent 字符串。
fn user_agent() -> String {
    format!("DDNet-Manager/{}", env!("CARGO_PKG_VERSION"))
}

/// 表示 GitHub API 返回的 Release 数据结构。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubReleaseResponse {
    /// 发布的 tag 名称（如 v0.1.0）。
    pub tag_name: String,
    /// 发布的网页 HTML URL。
    pub html_url: String,
    /// 发布的更新说明正文。
    pub body: Option<String>,
    /// 发布中附带的资产列表。
    pub assets: Vec<GitHubReleaseAsset>,
    /// 是否为预发布。GitHub `/releases/latest` 自动排除 prerelease，
    /// 但 `/releases/tags/{tag}` 返回的 nightly release 此字段为 true。
    #[serde(default)]
    pub prerelease: bool,
    /// 发布时间（ISO 8601 UTC，如 `2026-06-30T18:46:26Z`）。
    /// nightly rolling release 的 tag_name 固定为 "nightly"，无法做版本比较，
    /// 用此字段前 10 位日期生成 `nightly-{YYYY-MM-DD}` 版本号。
    #[serde(default)]
    pub published_at: String,
}

/// 表示 GitHub Release 中的一个资产项。
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubReleaseAsset {
    /// 资产名称。
    pub name: String,
    /// 资产的浏览器下载链接。
    pub browser_download_url: String,
    /// 资产文件的大小（字节）。
    pub size: u64,
    /// 资产的 sha256 散列校验值，如果存在的话。
    #[serde(default)]
    pub digest: Option<String>,
}

/// 表示 GitHub Release 更新检查结果。
pub enum GitHubReleaseCheck {
    /// 找到了可自动下载并校验的资产。
    Download {
        /// 最新版本。
        version: String,
        /// 可下载资产。
        asset: UpdateAsset,
    },
    /// 找到了平台资产，但缺少 sha256，必须手动处理。
    Manual {
        /// 最新版本。
        version: String,
        /// 手动下载提示。
        message: String,
    },
}

struct ReleaseSelection {
    platform: String,
    /// 已规范化的版本号。stable channel 由 `normalize_release_version(tag_name)` 计算，
    /// nightly channel 由 `nightly_version_from_published_at(published_at)` 计算。
    version: String,
    asset: GitHubReleaseAsset,
}

/// 从 GitHub 获取最新的 release 数据，并通过本地代理隧道（如已配置）访问。
pub async fn fetch_latest_github_release(
    owner: &str,
    repo: &str,
    route: Option<&NetworkRouteConfig>,
) -> Result<GitHubReleaseResponse, ManagerError> {
    let url = format!("{GITHUB_API_BASE}/{owner}/{repo}/releases/latest");
    let client = crate::network_route::build_routed_client(
        route,
        Some(Duration::from_secs(15)),
        Some(&user_agent()),
        true,
    )?;

    let response = client
        .get(&url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| ManagerError::ManifestUnreachable(format!("failed to fetch GitHub release: {error}")))?
        .json::<GitHubReleaseResponse>()
        .await
        .map_err(|error| ManagerError::Internal(format!("failed to parse GitHub release: {error}")))?;

    Ok(response)
}

/// 从 GitHub 按 tag 拉取单个 release（含 prerelease）。
///
/// 与 `/releases/latest` 不同，`/releases/tags/{tag}` 端点会返回该 tag 下的所有
/// release，包括标记为 prerelease 的 nightly rolling build。用于 nightly channel。
pub async fn fetch_github_release_by_tag(
    owner: &str,
    repo: &str,
    tag: &str,
    route: Option<&NetworkRouteConfig>,
) -> Result<GitHubReleaseResponse, ManagerError> {
    let url = format!("{GITHUB_API_BASE}/{owner}/{repo}/releases/tags/{tag}");
    let client = crate::network_route::build_routed_client(
        route,
        Some(Duration::from_secs(15)),
        Some(&user_agent()),
        true,
    )?;

    let response = client
        .get(&url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| ManagerError::ManifestUnreachable(format!("failed to fetch GitHub release by tag: {error}")))?
        .json::<GitHubReleaseResponse>()
        .await
        .map_err(|error| ManagerError::Internal(format!("failed to parse GitHub release: {error}")))?;

    Ok(response)
}

/// 从 GitHub release 的 `published_at` 字段生成 nightly 版本号。
///
/// nightly rolling release 的 `tag_name` 固定为 `"nightly"`，无法做版本比较。
/// 取 `published_at` 前 10 位日期（`YYYY-MM-DD`）生成 `nightly-{date}`，
/// 供 `is_update_needed` fallback 字符串比较。输入过短或非日期格式时返回
/// `nightly-unknown`，避免 panic。
fn nightly_version_from_published_at(published_at: &str) -> String {
    let date: String = published_at.chars().take(10).collect();
    // 严格校验 `YYYY-MM-DD` 格式，避免误把短字符串当日期。
    let valid = date.len() == 10
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[7] == b'-'
        && date
            .bytes()
            .enumerate()
            .all(|(i, b)| (i == 4 || i == 7) || b.is_ascii_digit());
    if valid {
        format!("nightly-{date}")
    } else {
        "nightly-unknown".to_string()
    }
}

/// 按更新通道（stable/nightly）检查 GitHub Release 并选择当前平台可校验资产。
///
/// - `stable`：走 `/releases/latest`，版本号用 `normalize_release_version(tag_name)`。
/// - `nightly`：走 `/releases/tags/{nightly_tag}`，版本号用
///   `nightly_version_from_published_at(published_at)`。catalog 未配 `nightly_tag`
///   时返回 `None`（该客户端不支持 nightly）。
pub async fn check_release_by_channel(
    entry: &ClientCatalogEntry,
    platform: &str,
    channel: &str,
    route: Option<&NetworkRouteConfig>,
) -> Result<Option<GitHubReleaseCheck>, ManagerError> {
    let crate::client_catalog::UpdateSourceDescriptor::GithubRelease {
        owner,
        repo,
        nightly_tag,
        ..
    } = entry.update_source
    else {
        return Ok(None);
    };

    let (release, version) = match channel {
        "nightly" => {
            let Some(tag) = nightly_tag else {
                // 该客户端未配 nightly_tag，不支持 nightly channel。
                return Ok(None);
            };
            let release = fetch_github_release_by_tag(owner, repo, tag, route).await?;
            let version = nightly_version_from_published_at(&release.published_at);
            (release, version)
        }
        "stable" => {
            let release = fetch_latest_github_release(owner, repo, route).await?;
            let version = normalize_release_version(&release.tag_name);
            (release, version)
        }
        other => {
            // fail-fast：未知 channel 直接报错，避免静默走 stable 掩盖前端契约违规。
            // 当前前端只产生 "stable"/"nightly"（UpdatePanel.tsx），其他值属于调用方 bug。
            return Err(ManagerError::Internal(format!(
                "unsupported channel: {other}"
            )));
        }
    };

    let digests = fetch_expanded_assets_digests(owner, repo, &release.tag_name, route)
        .await
        .unwrap_or_default();

    select_release_asset(ReleaseAssetSelection {
        entry,
        platform,
        release,
        version,
        digests: &digests,
    })
}

/// `select_release_asset` 的参数包，避免函数参数超过 4 个。
struct ReleaseAssetSelection<'a> {
    entry: &'a ClientCatalogEntry,
    platform: &'a str,
    release: GitHubReleaseResponse,
    version: String,
    digests: &'a HashMap<String, String>,
}

fn select_release_asset(
    selection: ReleaseAssetSelection<'_>,
) -> Result<Option<GitHubReleaseCheck>, ManagerError> {
    let patterns = asset_patterns_for_platform(selection.entry, selection.platform);
    if patterns.is_empty() {
        return Ok(None);
    }

    let Some(asset) = selection
        .release
        .assets
        .into_iter()
        .find(|asset| patterns.iter().any(|pattern| asset.name == *pattern))
    else {
        return Ok(None);
    };

    build_update_asset(
        ReleaseSelection {
            platform: selection.platform.to_string(),
            version: selection.version,
            asset,
        },
        selection.digests,
    )
}

fn build_update_asset(
    selection: ReleaseSelection,
    digests: &HashMap<String, String>,
) -> Result<Option<GitHubReleaseCheck>, ManagerError> {
    // version 已由调用方规范化（stable: normalize_release_version, nightly: nightly_version_from_published_at）。
    let version = selection.version;
    // 优先用 expanded_assets 的 digest（标准 release API 默认不返回），回退到 asset.digest
    let sha256 = digests.get(&selection.asset.name).cloned().or_else(|| {
        selection
            .asset
            .digest
            .as_deref()
            .and_then(parse_github_sha256_digest)
    });
    let Some(sha256) = sha256 else {
        return Ok(Some(GitHubReleaseCheck::Manual {
            version,
            message: "更新资产缺少 sha256，自动安装已禁用，请打开 Release 页面手动下载。"
                .to_string(),
        }));
    };

    Ok(Some(GitHubReleaseCheck::Download {
        version,
        asset: UpdateAsset {
            platform: selection.platform,
            asset_url: selection.asset.browser_download_url,
            sha256,
            size: selection.asset.size,
        },
    }))
}

fn asset_patterns_for_platform(
    entry: &ClientCatalogEntry,
    platform: &str,
) -> &'static [&'static str] {
    let crate::client_catalog::UpdateSourceDescriptor::GithubRelease {
        windows_assets,
        macos_assets,
        linux_assets,
        ..
    } = entry.update_source
    else {
        return &[];
    };

    if platform.starts_with("windows") {
        windows_assets
    } else if platform.starts_with("macos") || platform == "darwin" {
        macos_assets
    } else if platform.starts_with("linux") {
        linux_assets
    } else {
        &[]
    }
}

fn parse_github_sha256_digest(input: &str) -> Option<String> {
    let value = input.trim();
    let digest = value.strip_prefix("sha256:").unwrap_or(value);
    if digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Some(digest.to_ascii_lowercase())
    } else {
        None
    }
}

/// 从 GitHub expanded_assets HTML 片段解析 asset 名到 sha256 digest 的映射。
///
/// expanded_assets 端点返回 HTML（非 JSON），digest 只出现在每个 asset 的
/// `clipboard-copy` 元素属性对里：`aria-label="Copy to clipboard digest for {name}"`
/// 配 `value="sha256:{digest}"`。这里用稳定属性锚点做轻量解析，避免引入 HTML 依赖。
pub fn parse_expanded_assets_digests(html: &str) -> HashMap<String, String> {
    const LABEL_PREFIX: &str = "aria-label=\"Copy to clipboard digest for ";
    const VALUE_PREFIX: &str = "value=\"sha256:";

    let mut map = HashMap::new();
    for chunk in html.split(LABEL_PREFIX) {
        // LABEL_PREFIX 后紧跟 asset 名，到第一个 "
        let Some((name, _)) = chunk.split_once('"') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || name.contains('<') || name.contains('>') {
            continue;
        }
        // 同一 chunk 内找 value="sha256:..."，取 64 位十六进制
        let Some(pos) = chunk.find(VALUE_PREFIX) else {
            continue;
        };
        let after = &chunk[pos + VALUE_PREFIX.len()..];
        let digest: String = after.chars().take(64).collect();
        if digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()) {
            map.insert(name.to_string(), digest.to_ascii_lowercase());
        }
    }
    map
}

/// 拉取 GitHub expanded_assets 页面并解析 asset 名到 sha256 digest 的映射。
///
/// expanded_assets 端点 `https://github.com/{owner}/{repo}/releases/expanded_assets/{tag}`
/// 返回 HTML，包含标准 release API 不提供的 digest。失败时调用方应回退空映射，
/// 不应阻塞更新检查主流程。
async fn fetch_expanded_assets_digests(
    owner: &str,
    repo: &str,
    tag: &str,
    route: Option<&NetworkRouteConfig>,
) -> Result<HashMap<String, String>, ManagerError> {
    let url = format!("https://github.com/{owner}/{repo}/releases/expanded_assets/{tag}");
    let client = crate::network_route::build_routed_client(
        route,
        Some(Duration::from_secs(15)),
        Some(&user_agent()),
        true,
    )?;

    let html = client
        .get(&url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| ManagerError::ManifestUnreachable(format!("failed to fetch expanded-assets: {error}")))?
        .text()
        .await
        .map_err(|error| ManagerError::Internal(format!("failed to read expanded-assets body: {error}")))?;

    Ok(parse_expanded_assets_digests(&html))
}

fn normalize_release_version(tag_name: &str) -> String {
    tag_name.trim_start_matches(['v', 'V']).to_string()
}

#[cfg(test)]
#[path = "test/github_release.rs"]
mod tests;
