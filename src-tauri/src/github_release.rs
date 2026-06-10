use crate::client_catalog::ClientCatalogEntry;
use crate::models::{NetworkRouteConfig, UpdateAsset};
use serde::{Deserialize, Serialize};
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

    select_release_asset(entry, platform, release)
}

fn select_release_asset(
    entry: &ClientCatalogEntry,
    platform: &str,
    release: GitHubReleaseResponse,
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

    build_update_asset(ReleaseSelection {
        platform: platform.to_string(),
        tag_name: release.tag_name,
        asset,
    })
}

fn build_update_asset(selection: ReleaseSelection) -> Result<Option<GitHubReleaseCheck>, String> {
    let version = normalize_release_version(&selection.tag_name);
    let Some(sha256) = selection
        .asset
        .digest
        .as_deref()
        .and_then(parse_github_sha256_digest)
    else {
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

fn normalize_release_version(tag_name: &str) -> String {
    tag_name.trim_start_matches(['v', 'V']).to_string()
}

#[cfg(test)]
#[path = "test/github_release.rs"]
mod tests;
