use crate::error::ManagerError;
use crate::models::{
    CheckClientUpdateRequest, ClientUpdateCheck, ClientUpdateSelector, UpdateAction, UpdateAsset,
    UpdateCheckReason, UpdateSourceKind,
};

struct ManifestUpdateInput<'a> {
    request: &'a CheckClientUpdateRequest,
    current_version: Option<String>,
    platform: String,
    /// manifest 缓存目录。`None` 时禁用本地缓存 fallback。
    cache_dir: Option<&'a std::path::Path>,
}

struct DownloadUpdateInput {
    client_id: String,
    channel: String,
    current_version: Option<String>,
    latest_version: String,
    asset: UpdateAsset,
    source_kind: UpdateSourceKind,
}

struct CatalogUpdateInput<'a> {
    entry: &'a crate::client_catalog::ClientCatalogEntry,
    request: &'a CheckClientUpdateRequest,
    current_version: Option<String>,
    platform: String,
}

/// 根据客户端类型和请求配置检查更新。
///
/// 始终返回 `ClientUpdateCheck`（非 Option）：当 `action == None` 时用 `reason` 字段
/// 区分"已是最新版"与"无法判断/不支持自动更新"等语义，避免前端把后者误判为前者。
pub async fn check_client_update(
    request: &CheckClientUpdateRequest,
    current_version: Option<String>,
    cache_dir: Option<&std::path::Path>,
) -> Result<ClientUpdateCheck, crate::error::ManagerError> {
    // 若用户未配置路由，自动探测并选择最佳网络路径。
    let mut request = request.clone();
    if request.network_route.is_none() {
        if let Some(route) = crate::network_route::auto_select_route(None).await {
            request.network_route = Some(route);
        }
    }

    if request.use_manifest_source {
        return check_manifest_update(ManifestUpdateInput {
            request: &request,
            current_version,
            platform: request.platform.clone().unwrap_or_else(current_platform),
            cache_dir,
        })
        .await;
    }

    let client_id = crate::client_catalog::normalize_client_id(&request.client_id);
    let Some(entry) = crate::client_catalog::catalog_entry_by_id(client_id) else {
        let platform = request.platform.clone().unwrap_or_else(current_platform);
        return Ok(unavailable_check(UnavailableCheckInput {
            client_id: &request.client_id,
            channel: &request.channel,
            current_version: current_version.as_deref(),
            platform: &platform,
            reason: UpdateCheckReason::ClientNotInCatalog,
            source_kind: UpdateSourceKind::None,
            message: "客户端不在内置 catalog 中，无法检查更新。".to_string(),
        }));
    };
    let platform = request.platform.clone().unwrap_or_else(current_platform);

    check_catalog_update(CatalogUpdateInput {
        entry,
        request: &request,
        current_version,
        platform,
    })
    .await
}

async fn check_catalog_update(
    input: CatalogUpdateInput<'_>,
) -> Result<ClientUpdateCheck, ManagerError> {
    match input.entry.update_source {
        crate::client_catalog::UpdateSourceDescriptor::GithubRelease { .. } => {
            check_github_release_update(input).await
        }
        crate::client_catalog::UpdateSourceDescriptor::DdnetOfficial => {
            check_ddnet_official_update(input).await
        }
        crate::client_catalog::UpdateSourceDescriptor::Website { url } => Ok(manual_update(
            ManualUpdateInput {
                client_id: input.entry.client_id.to_string(),
                channel: input.request.channel.clone(),
                latest_version: String::new(),
                platform: input.platform,
                source_kind: UpdateSourceKind::Website,
                action_url: Some(url.to_string()),
                message: "该客户端当前仅支持打开官网手动下载。".to_string(),
            },
            input.current_version,
        )),
        crate::client_catalog::UpdateSourceDescriptor::None => {
            Ok(unavailable_check(UnavailableCheckInput {
                client_id: input.entry.client_id,
                channel: &input.request.channel,
                current_version: input.current_version.as_deref(),
                platform: &input.platform,
                reason: UpdateCheckReason::AutoUpdateDisabled,
                source_kind: UpdateSourceKind::None,
                message: "该客户端不支持自动更新。".to_string(),
            }))
        }
    }
}

/// GithubRelease 来源的更新检查：拉 release by channel，区分 Download / Manual 两种动作。
async fn check_github_release_update(
    input: CatalogUpdateInput<'_>,
) -> Result<ClientUpdateCheck, ManagerError> {
    let Some(check) = crate::github_release::check_release_by_channel(
        input.entry,
        &input.platform,
        &input.request.channel,
        input.request.network_route.as_ref(),
    )
    .await?
    else {
        return Ok(unavailable_check(UnavailableCheckInput {
            client_id: input.entry.client_id,
            channel: &input.request.channel,
            current_version: input.current_version.as_deref(),
            platform: &input.platform,
            reason: UpdateCheckReason::NoReleaseForChannel,
            source_kind: UpdateSourceKind::GithubRelease,
            message: "该渠道下无匹配的 release（可能未配置 nightly tag 或仓库无该 tag）。"
                .to_string(),
        }));
    };
    match check {
        crate::github_release::GitHubReleaseCheck::Download { version, asset } => {
            Ok(download_update(DownloadUpdateInput {
                client_id: input.entry.client_id.to_string(),
                channel: input.request.channel.clone(),
                current_version: input.current_version,
                latest_version: version,
                asset,
                source_kind: UpdateSourceKind::GithubRelease,
            }))
        }
        crate::github_release::GitHubReleaseCheck::Manual { version, message } => {
            Ok(manual_update(
                ManualUpdateInput {
                    client_id: input.entry.client_id.to_string(),
                    channel: input.request.channel.clone(),
                    latest_version: version,
                    platform: input.platform,
                    source_kind: UpdateSourceKind::GithubRelease,
                    action_url: input.entry.upstream_url.map(str::to_string),
                    message,
                },
                input.current_version,
            ))
        }
    }
}

/// DDNet 官方来源的更新检查：解析官方下载页，匹配当前平台资产。
async fn check_ddnet_official_update(
    input: CatalogUpdateInput<'_>,
) -> Result<ClientUpdateCheck, ManagerError> {
    let Some(asset) = crate::ddnet_source::check_official_download(
        &input.platform,
        input.request.network_route.as_ref(),
    )
    .await?
    else {
        return Ok(unavailable_check(UnavailableCheckInput {
            client_id: "ddnet",
            channel: &input.request.channel,
            current_version: input.current_version.as_deref(),
            platform: &input.platform,
            reason: UpdateCheckReason::NoAssetForPlatform,
            source_kind: UpdateSourceKind::DdnetOfficial,
            message: "DDNet 官方下载页无当前平台的匹配资产。".to_string(),
        }));
    };
    Ok(download_update(DownloadUpdateInput {
        client_id: "ddnet".to_string(),
        channel: input.request.channel.clone(),
        current_version: input.current_version,
        latest_version: asset.version.clone(),
        asset: asset.into(),
        source_kind: UpdateSourceKind::DdnetOfficial,
    }))
}

struct ManualUpdateInput {
    client_id: String,
    channel: String,
    latest_version: String,
    platform: String,
    source_kind: UpdateSourceKind,
    action_url: Option<String>,
    message: String,
}

fn manual_update(input: ManualUpdateInput, current_version: Option<String>) -> ClientUpdateCheck {
    ClientUpdateCheck {
        client_id: input.client_id,
        channel: input.channel,
        current_version,
        latest_version: input.latest_version,
        asset: empty_asset(&input.platform),
        needs_update: false,
        source_kind: input.source_kind,
        action: UpdateAction::OpenUrl,
        action_url: input.action_url,
        message: Some(input.message),
        reason: UpdateCheckReason::None,
    }
}

async fn check_manifest_update(
    input: ManifestUpdateInput<'_>,
) -> Result<ClientUpdateCheck, ManagerError> {
    let manifest = crate::manifest::fetch_manifest_with_route(
        crate::commands::download::required_manifest_url(input.request.manifest_url.as_deref())?,
        input.request.network_route.as_ref(),
        input.cache_dir,
    )
    .await?;
    let selector = ClientUpdateSelector {
        client_id: crate::client_catalog::normalize_client_id(&input.request.client_id).to_string(),
        channel: input.request.channel.clone(),
        platform: input.platform,
    };
    let mut update = match crate::manifest::select_client_update(&manifest, &selector)? {
        Some(update) => update,
        None => {
            // input.platform 已 move 进 selector，这里复用 selector.platform
            // 避免引入额外 clone。
            return Ok(unavailable_check(UnavailableCheckInput {
                client_id: &input.request.client_id,
                channel: &input.request.channel,
                current_version: input.current_version.as_deref(),
                platform: &selector.platform,
                reason: UpdateCheckReason::ManifestEntryMissing,
                source_kind: UpdateSourceKind::Manifest,
                message: "manifest 中无该客户端+渠道+平台的条目。".to_string(),
            }));
        }
    };
    update.needs_update =
        crate::version::is_update_needed(input.current_version.as_deref(), &update.latest_version);
    update.current_version = input.current_version;
    Ok(update)
}

fn download_update(input: DownloadUpdateInput) -> ClientUpdateCheck {
    let needs_update =
        crate::version::is_update_needed(input.current_version.as_deref(), &input.latest_version);
    ClientUpdateCheck {
        client_id: input.client_id,
        channel: input.channel,
        current_version: input.current_version,
        latest_version: input.latest_version,
        asset: input.asset,
        needs_update,
        source_kind: input.source_kind,
        action: UpdateAction::Download,
        action_url: None,
        message: None,
        reason: UpdateCheckReason::None,
    }
}

/// `unavailable_check` 的参数包，避免函数参数超过 4 个。
struct UnavailableCheckInput<'a> {
    client_id: &'a str,
    channel: &'a str,
    current_version: Option<&'a str>,
    platform: &'a str,
    reason: UpdateCheckReason,
    source_kind: UpdateSourceKind,
    message: String,
}

/// 构造一个"不可用更新检查"结果：`action=None` + 具体原因。
///
/// 用于替代原来的 `Ok(None)`，让前端能通过 `reason` 字段区分
/// "已是最新版"（`reason=None`）与"无法判断/不支持自动更新"等其他语义。
fn unavailable_check(input: UnavailableCheckInput<'_>) -> ClientUpdateCheck {
    ClientUpdateCheck {
        client_id: input.client_id.to_string(),
        channel: input.channel.to_string(),
        current_version: input.current_version.map(str::to_string),
        latest_version: String::new(),
        asset: empty_asset(input.platform),
        needs_update: false,
        source_kind: input.source_kind,
        action: UpdateAction::None,
        action_url: None,
        message: Some(input.message),
        reason: input.reason,
    }
}

fn empty_asset(platform: &str) -> UpdateAsset {
    UpdateAsset {
        platform: platform.to_string(),
        asset_url: String::new(),
        sha256: String::new(),
        size: 0,
    }
}

fn current_platform() -> String {
    platform_from_os_arch(std::env::consts::OS, std::env::consts::ARCH)
}

fn platform_from_os_arch(os: &str, arch: &str) -> String {
    match os {
        "windows" if arch == "x86_64" => "windows-x86_64".to_string(),
        "windows" if arch == "aarch64" => "windows-arm64".to_string(),
        "windows" if arch == "x86" => "windows-x86".to_string(),
        "windows" => "windows".to_string(),
        "macos" => "macos".to_string(),
        "linux" => format!("linux-{arch}"),
        other => other.to_string(),
    }
}

#[cfg(test)]
#[path = "test/update_source.rs"]
mod tests;
