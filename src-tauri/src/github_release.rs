use crate::client_catalog::ClientCatalogEntry;
use crate::models::{NetworkRouteConfig, UpdateAsset};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

const GITHUB_API_BASE: &str = "https://api.github.com/repos";
const USER_AGENT: &str = "DDNet-Manager/0.1.0";

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
    tag_name: String,
    asset: GitHubReleaseAsset,
}

/// 从 GitHub 获取最新的 release 数据，并处理网络路由（代理/镜像）。
pub async fn fetch_latest_github_release(
    owner: &str,
    repo: &str,
    route: Option<&NetworkRouteConfig>,
) -> Result<GitHubReleaseResponse, String> {
    let url_str = format!("{GITHUB_API_BASE}/{owner}/{repo}/releases/latest");
    let final_url = crate::manifest::build_url_with_route(&url_str, route)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("failed to create GitHub client: {error}"))?;

    let response = client
        .get(final_url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("failed to fetch GitHub release: {error}"))?
        .json::<GitHubReleaseResponse>()
        .await
        .map_err(|error| format!("failed to parse GitHub release: {error}"))?;

    Ok(response)
}

/// 从 GitHub latest release 中选择当前平台可校验资产。
pub async fn check_latest_release(
    entry: &ClientCatalogEntry,
    platform: &str,
    route: Option<&NetworkRouteConfig>,
) -> Result<Option<GitHubReleaseCheck>, String> {
    let crate::client_catalog::UpdateSourceDescriptor::GithubRelease { owner, repo, .. } =
        entry.update_source
    else {
        return Ok(None);
    };

    let release = fetch_latest_github_release(owner, repo, route).await?;
    let digests = fetch_expanded_assets_digests(owner, repo, &release.tag_name, route)
        .await
        .unwrap_or_default();

    select_release_asset(entry, platform, release, &digests)
}

fn select_release_asset(
    entry: &ClientCatalogEntry,
    platform: &str,
    release: GitHubReleaseResponse,
    digests: &HashMap<String, String>,
) -> Result<Option<GitHubReleaseCheck>, String> {
    let patterns = asset_patterns_for_platform(entry, platform);
    if patterns.is_empty() {
        return Ok(None);
    }

    let Some(asset) = release
        .assets
        .into_iter()
        .find(|asset| patterns.iter().any(|pattern| asset.name == *pattern))
    else {
        return Ok(None);
    };

    build_update_asset(
        ReleaseSelection {
            platform: platform.to_string(),
            tag_name: release.tag_name,
            asset,
        },
        digests,
    )
}

fn build_update_asset(
    selection: ReleaseSelection,
    digests: &HashMap<String, String>,
) -> Result<Option<GitHubReleaseCheck>, String> {
    let version = normalize_release_version(&selection.tag_name);
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
) -> Result<HashMap<String, String>, String> {
    let url = format!("https://github.com/{owner}/{repo}/releases/expanded_assets/{tag}");
    let final_url = crate::manifest::build_url_with_route(&url, route)?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("failed to create expanded-assets client: {error}"))?;

    let html = client
        .get(final_url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("failed to fetch expanded-assets: {error}"))?
        .text()
        .await
        .map_err(|error| format!("failed to read expanded-assets body: {error}"))?;

    Ok(parse_expanded_assets_digests(&html))
}

fn normalize_release_version(tag_name: &str) -> String {
    tag_name.trim_start_matches(['v', 'V']).to_string()
}

#[cfg(test)]
#[path = "test/github_release.rs"]
mod tests;
