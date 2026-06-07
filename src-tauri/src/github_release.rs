use crate::client_catalog::ClientCatalogEntry;
use crate::models::UpdateAsset;
use serde::Deserialize;
use std::time::Duration;

const GITHUB_API_BASE: &str = "https://api.github.com/repos";
const USER_AGENT: &str = "DDNet-Manager/0.1.0";

#[derive(Debug, Deserialize)]
struct GitHubReleaseResponse {
    tag_name: String,
    assets: Vec<GitHubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
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

/// 从 GitHub latest release 中选择当前平台可校验资产。
pub async fn check_latest_release(
    entry: &ClientCatalogEntry,
    platform: &str,
) -> Result<Option<GitHubReleaseCheck>, String> {
    let crate::client_catalog::UpdateSourceDescriptor::GithubRelease { owner, repo, .. } =
        entry.update_source
    else {
        return Ok(None);
    };
    let url = format!("{GITHUB_API_BASE}/{owner}/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("failed to create GitHub client: {error}"))?;
    let release = client
        .get(url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("failed to fetch GitHub release: {error}"))?
        .json::<GitHubReleaseResponse>()
        .await
        .map_err(|error| format!("failed to parse GitHub release: {error}"))?;

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
mod tests {
    use super::{
        build_update_asset, normalize_release_version, parse_github_sha256_digest,
        GitHubReleaseAsset, GitHubReleaseCheck, ReleaseSelection,
    };

    #[test]
    fn parses_github_sha256_digest() {
        let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

        assert_eq!(
            parse_github_sha256_digest(&format!("sha256:{digest}")),
            Some(digest.to_string())
        );
        assert_eq!(parse_github_sha256_digest(digest), Some(digest.to_string()));
        assert!(parse_github_sha256_digest("sha256:not-valid").is_none());
    }

    #[test]
    fn normalizes_release_version_prefix() {
        assert_eq!(normalize_release_version("v2.62.4"), "2.62.4");
        assert_eq!(normalize_release_version("V10.8.7"), "10.8.7");
        assert_eq!(normalize_release_version("19.8.2"), "19.8.2");
    }

    #[test]
    fn missing_digest_returns_manual_release_action() {
        let result = build_update_asset(ReleaseSelection {
            platform: "windows-x86_64".to_string(),
            tag_name: "v2.62.4".to_string(),
            asset: GitHubReleaseAsset {
                name: "QmClient-windows.zip".to_string(),
                browser_download_url: "https://github.com/example/release.zip".to_string(),
                size: 42,
                digest: None,
            },
        })
        .expect("缺少 digest 应返回手动动作")
        .expect("应存在匹配资产");

        match result {
            GitHubReleaseCheck::Manual { version, message } => {
                assert_eq!(version, "2.62.4");
                assert!(message.contains("sha256"));
            }
            GitHubReleaseCheck::Download { .. } => panic!("缺少 digest 不应返回下载动作"),
        }
    }
}
