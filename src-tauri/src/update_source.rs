use crate::models::{
    CheckClientUpdateRequest, ClientUpdateCheck, ClientUpdateSelector, UpdateAction, UpdateAsset,
    UpdateSourceKind,
};

struct ManifestUpdateInput<'a> {
    request: &'a CheckClientUpdateRequest,
    current_version: Option<String>,
    platform: String,
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
pub async fn check_client_update(
    request: &CheckClientUpdateRequest,
    current_version: Option<String>,
) -> Result<Option<ClientUpdateCheck>, String> {
    if request.use_manifest_source {
        return check_manifest_update(ManifestUpdateInput {
            request,
            current_version,
            platform: request.platform.clone().unwrap_or_else(current_platform),
        })
        .await;
    }

    let client_id = crate::client_catalog::normalize_client_id(&request.client_id);
    let Some(entry) = crate::client_catalog::catalog_entry_by_id(client_id) else {
        return Ok(None);
    };
    let platform = request.platform.clone().unwrap_or_else(current_platform);

    check_catalog_update(CatalogUpdateInput {
        entry,
        request,
        current_version,
        platform,
    })
    .await
}

async fn check_catalog_update(
    input: CatalogUpdateInput<'_>,
) -> Result<Option<ClientUpdateCheck>, String> {
    match input.entry.update_source {
        crate::client_catalog::UpdateSourceDescriptor::GithubRelease { .. } => {
            let Some(check) =
                crate::github_release::check_latest_release(input.entry, &input.platform).await?
            else {
                return Ok(None);
            };
            match check {
                crate::github_release::GitHubReleaseCheck::Download { version, asset } => {
                    Ok(Some(download_update(DownloadUpdateInput {
                        client_id: input.entry.client_id.to_string(),
                        channel: input.request.channel.clone(),
                        current_version: input.current_version,
                        latest_version: version,
                        asset,
                        source_kind: UpdateSourceKind::GithubRelease,
                    })))
                }
                crate::github_release::GitHubReleaseCheck::Manual { version, message } => {
                    Ok(Some(manual_update(
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
                    )))
                }
            }
        }
        crate::client_catalog::UpdateSourceDescriptor::DdnetOfficial => Ok(Some(manual_update(
            ManualUpdateInput {
                client_id: "ddnet".to_string(),
                channel: input.request.channel.clone(),
                latest_version: String::new(),
                platform: input.platform,
                source_kind: UpdateSourceKind::Website,
                action_url: input.entry.upstream_url.map(str::to_string),
                message:
                    "DDNet 官方二进制更新源需要解析官网 sha256sums.txt，当前请打开官网下载页手动处理。"
                        .to_string(),
            },
            input.current_version,
        ))),
        crate::client_catalog::UpdateSourceDescriptor::Website { url } => Ok(Some(manual_update(
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
        ))),
        crate::client_catalog::UpdateSourceDescriptor::None => Ok(None),
    }
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
    }
}

async fn check_manifest_update(
    input: ManifestUpdateInput<'_>,
) -> Result<Option<ClientUpdateCheck>, String> {
    let manifest = crate::manifest::fetch_manifest_with_route(
        crate::commands::required_manifest_url(input.request.manifest_url.as_deref())?,
        input.request.network_route.as_ref(),
    )
    .await?;
    let selector = ClientUpdateSelector {
        client_id: crate::client_catalog::normalize_client_id(&input.request.client_id).to_string(),
        channel: input.request.channel.clone(),
        platform: input.platform,
    };
    let mut update = match crate::manifest::select_client_update(&manifest, &selector)? {
        Some(update) => update,
        None => return Ok(None),
    };
    update.needs_update =
        crate::version::is_update_needed(input.current_version.as_deref(), &update.latest_version);
    update.current_version = input.current_version;
    Ok(Some(update))
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
    if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "windows-x86_64".to_string()
    } else if cfg!(target_os = "windows") {
        "windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macos".to_string()
    } else {
        std::env::consts::OS.to_string()
    }
}
