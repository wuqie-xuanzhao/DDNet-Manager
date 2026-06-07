use crate::models::UpdateAsset;
use std::collections::HashMap;
use std::time::Duration;

const DDNET_DOWNLOADS_URL: &str = "https://ddnet.org/downloads/";
const DDNET_SHA256_URL: &str = "https://ddnet.org/downloads/sha256sums.txt";
const USER_AGENT: &str = "DDNet-Manager/0.1.0";

/// 表示 DDNet 官方下载页解析出的候选资产。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DdnetDownloadAsset {
    /// DDNet 版本号。
    pub version: String,
    /// 当前平台标识。
    pub platform: String,
    /// 资产下载地址。
    pub asset_url: String,
    /// 资产 sha256 摘要。
    pub sha256: String,
    /// 资产大小，官网下载页无法稳定提供时为 1。
    pub size: u64,
}

/// 从 DDNet 官网 HTML 和 sha256sums.txt 中选择当前平台资产。
pub fn select_official_asset_from_text(
    downloads_html: &str,
    sha256sums: &str,
    platform: &str,
) -> Result<Option<DdnetDownloadAsset>, String> {
    let Some(file_name) = select_newest_platform_file(downloads_html, platform) else {
        return Ok(None);
    };
    let sha256_by_file = parse_sha256sums(sha256sums);
    let sha256 = sha256_by_file
        .get(&file_name)
        .cloned()
        .ok_or_else(|| format!("missing sha256 for DDNet official asset: {file_name}"))?;
    let version = parse_ddnet_version(&file_name)
        .ok_or_else(|| format!("failed to parse DDNet version from asset: {file_name}"))?;

    Ok(Some(DdnetDownloadAsset {
        version,
        platform: platform.to_string(),
        asset_url: format!("{DDNET_DOWNLOADS_URL}{file_name}"),
        sha256,
        // 官网下载页和 sha256sums 不稳定提供 size；下载阶段仍会按 HTTP
        // Content-Length 与实际文件校验。这里使用 1 表示“非空资产”。
        size: 1,
    }))
}

/// 从 DDNet 官方下载页和 sha256sums.txt 拉取当前平台可校验资产。
pub async fn check_official_download(platform: &str) -> Result<Option<DdnetDownloadAsset>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("failed to create DDNet official client: {error}"))?;
    let downloads_html = fetch_text(&client, DDNET_DOWNLOADS_URL, "DDNet downloads page").await?;
    let sha256sums = fetch_text(&client, DDNET_SHA256_URL, "DDNet sha256sums.txt").await?;
    let Some(mut asset) = select_official_asset_from_text(&downloads_html, &sha256sums, platform)?
    else {
        return Ok(None);
    };
    asset.size = fetch_content_length(&client, &asset.asset_url).await?;
    Ok(Some(asset))
}

async fn fetch_text(client: &reqwest::Client, url: &str, label: &str) -> Result<String, String> {
    client
        .get(url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("failed to fetch {label}: {error}"))?
        .text()
        .await
        .map_err(|error| format!("failed to read {label}: {error}"))
}

async fn fetch_content_length(client: &reqwest::Client, url: &str) -> Result<u64, String> {
    let response = client
        .head(url)
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|error| format!("failed to fetch DDNet asset size: {error}"))?;
    response
        .content_length()
        .filter(|size| *size > 0)
        .ok_or_else(|| "DDNet official asset is missing Content-Length".to_string())
}

fn extract_candidate_file_names(downloads_html: &str) -> Vec<String> {
    let mut names = Vec::new();
    for token in downloads_html.split(['"', '\'', '<', '>', ' ', '\n', '\r', '\t']) {
        if let Some(index) = token.find("DDNet-") {
            let candidate = token[index..]
                .trim_start_matches("/downloads/")
                .trim_start_matches("downloads/");
            if is_supported_ddnet_asset(candidate) {
                let file_name = candidate.to_string();
                if !names.contains(&file_name) {
                    names.push(file_name);
                }
            }
        }
    }
    names
}

fn select_newest_platform_file(downloads_html: &str, platform: &str) -> Option<String> {
    extract_candidate_file_names(downloads_html)
        .into_iter()
        .filter(|file_name| asset_matches_platform(file_name, platform))
        .max_by(|left, right| compare_ddnet_asset_versions(left, right))
}

fn is_supported_ddnet_asset(file_name: &str) -> bool {
    file_name.ends_with(".zip") || file_name.ends_with(".tar.xz") || file_name.ends_with(".dmg")
}

fn parse_sha256sums(input: &str) -> HashMap<String, String> {
    let mut checksums = HashMap::new();
    for line in input.lines() {
        let mut parts = line.split_whitespace();
        let Some(digest) = parts.next() else {
            continue;
        };
        let Some(file_name) = parts.next() else {
            continue;
        };
        if digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            checksums.insert(
                file_name.trim_start_matches('*').to_string(),
                digest.to_ascii_lowercase(),
            );
        }
    }
    checksums
}

fn asset_matches_platform(file_name: &str, platform: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    if platform.starts_with("windows") {
        if platform.contains("arm64") {
            lower.contains("win-arm64")
        } else if platform.contains("x86") && !platform.contains("x86_64") {
            lower.contains("win32")
        } else {
            lower.contains("win64")
        }
    } else if platform.starts_with("linux") {
        if platform.contains("x86_64") || platform == "linux" {
            lower.contains("linux_x86_64")
        } else {
            lower.contains("linux_x86")
        }
    } else if platform.starts_with("macos") || platform == "darwin" {
        lower.contains("macos")
    } else {
        false
    }
}

fn parse_ddnet_version(file_name: &str) -> Option<String> {
    file_name
        .strip_prefix("DDNet-")
        .and_then(|rest| rest.split('-').next())
        .filter(|version| !version.is_empty())
        .map(str::to_string)
}

fn compare_ddnet_asset_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let left_version = parse_ddnet_version(left).unwrap_or_default();
    let right_version = parse_ddnet_version(right).unwrap_or_default();
    compare_version_text(&left_version, &right_version)
}

fn compare_version_text(left: &str, right: &str) -> std::cmp::Ordering {
    let mut left_parts = left.split('.').map(parse_version_part);
    let mut right_parts = right.split('.').map(parse_version_part);
    loop {
        match (left_parts.next(), right_parts.next()) {
            (Some(left), Some(right)) if left != right => return left.cmp(&right),
            (Some(_), Some(_)) => {}
            (Some(left), None) if left != 0 => return std::cmp::Ordering::Greater,
            (None, Some(right)) if right != 0 => return std::cmp::Ordering::Less,
            (Some(_), None) | (None, Some(_)) => {}
            (None, None) => return left.cmp(right),
        }
    }
}

fn parse_version_part(part: &str) -> u64 {
    part.parse::<u64>().unwrap_or(0)
}

impl From<DdnetDownloadAsset> for UpdateAsset {
    fn from(asset: DdnetDownloadAsset) -> Self {
        Self {
            platform: asset.platform,
            asset_url: asset.asset_url,
            sha256: asset.sha256,
            size: asset.size,
        }
    }
}

#[cfg(test)]
#[path = "test/ddnet_source.rs"]
mod tests;
