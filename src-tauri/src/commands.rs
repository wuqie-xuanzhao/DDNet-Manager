use crate::download::DownloadManager;
use crate::models::{
    AppSettings, CheckClientUpdateRequest, ClientHealth, ClientInstallation, ClientUpdateCheck,
    DownloadJob, DownloadJobStatus, InstallHistoryRecord, InstallHistoryStatus, NetworkRouteConfig,
    ScanClientInstallationsOptions, StartUpdateDownloadRequest, UpdateAction,
    UpsertClientInstallationRequest,
};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

type DownloadManagerState<'a> = State<'a, DownloadManager>;

struct InstallContext<'a> {
    app: &'a AppHandle,
    manager: &'a DownloadManager,
    registry: &'a crate::registry::ClientRegistry,
    job_id: &'a str,
    job: &'a DownloadJob,
}

struct DownloadTaskContext {
    app: AppHandle,
    manager: DownloadManager,
    job: DownloadJob,
    cache_path: PathBuf,
    enabled_route_hosts: Vec<String>,
}

struct PreparedUpdateDownload {
    job: DownloadJob,
    enabled_route_hosts: Vec<String>,
}

struct InstallHistoryInput<'a> {
    job: &'a DownloadJob,
    client: &'a ClientInstallation,
    rollback_dir: &'a Path,
    status: InstallHistoryStatus,
    error: Option<String>,
}

/// 验证用户选择的客户端目录，并返回识别出的安装信息。
#[tauri::command]
pub fn validate_client_dir(path: String) -> Result<crate::models::ClientInstallation, String> {
    crate::client_scan::validate_client_dir(Path::new(&path))
}

/// 扫描本机候选客户端安装目录。
#[tauri::command]
pub fn scan_client_installations(
    app: AppHandle,
    options: Option<ScanClientInstallationsOptions>,
) -> Result<Vec<ClientInstallation>, String> {
    let options = options.unwrap_or_default();
    let use_everything = options.roots.is_empty();
    let mut roots: Vec<PathBuf> = if options.roots.is_empty() {
        crate::client_scan::default_scan_roots()
    } else {
        options.roots.iter().map(PathBuf::from).collect()
    };

    if options.include_saved_paths {
        roots.extend(
            registry_for_app(&app)?
                .list_client_installations()?
                .into_iter()
                .map(|client| PathBuf::from(client.install_dir)),
        );
    }
    let settings = registry_for_app(&app)?.load_app_settings()?;

    crate::client_scan::scan_client_installations(&crate::client_scan::ScanOptions {
        roots,
        include_saved_paths: options.include_saved_paths,
        deep: options.deep,
        use_everything: use_everything && settings.use_everything,
        excluded_paths: settings
            .scan_excluded_paths
            .iter()
            .map(PathBuf::from)
            .collect(),
    })
}

/// 保存或更新客户端安装记录。
#[tauri::command]
pub fn upsert_client_installation(
    app: AppHandle,
    request: UpsertClientInstallationRequest,
) -> Result<ClientInstallation, String> {
    let mut client = crate::client_scan::validate_client_dir(Path::new(&request.install_dir))?;
    client.is_default = request.is_default;
    registry_for_app(&app)?.upsert_client_installation(&client)?;
    Ok(client)
}

/// 从注册表移除客户端记录，不删除本地文件。
#[tauri::command]
pub fn remove_client_installation(app: AppHandle, id: String) -> Result<(), String> {
    registry_for_app(&app)?.remove_client_installation(&id)
}

/// 设置默认启动客户端。
#[tauri::command]
pub fn set_default_client(app: AppHandle, id: String) -> Result<(), String> {
    registry_for_app(&app)?.set_default_client(&id)
}

/// 读取所有已保存客户端安装记录。
#[tauri::command]
pub fn list_client_installations(app: AppHandle) -> Result<Vec<ClientInstallation>, String> {
    registry_for_app(&app)?.list_client_installations()
}

/// 读取默认启动客户端。
#[tauri::command]
pub fn get_default_client(app: AppHandle) -> Result<Option<ClientInstallation>, String> {
    registry_for_app(&app)?.get_default_client()
}

/// 启动指定路径的客户端可执行文件。
#[tauri::command]
pub fn launch_client(path: String) -> Result<(), String> {
    crate::process::launch_executable(&path)
}

/// 重新验证并启动默认客户端。
#[tauri::command]
pub fn launch_default_client(app: AppHandle) -> Result<(), String> {
    let registry = registry_for_app(&app)?;
    let client = registry
        .get_default_client()?
        .ok_or_else(|| "default client is not configured".to_string())?;
    let verified = crate::client_scan::validate_client_dir(Path::new(&client.install_dir))?;
    if verified.health != ClientHealth::Ok {
        return Err(format!(
            "default client is not healthy before launch: {:?}",
            verified.health
        ));
    }
    if !verified.compatibility.can_launch {
        return Err("default client is not compatible with this machine".to_string());
    }

    let probe = crate::process::launch_executable_with_probe(
        &verified.executable_path,
        Duration::from_secs(2),
    )?;
    registry.record_launch_probe_result(crate::registry::LaunchProbeRecord {
        client_installation_id: &client.id,
        status: probe.status,
        message: &probe.message,
    })?;
    Ok(())
}

/// 读取 MVP 应用设置。
#[tauri::command]
pub fn load_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    registry_for_app(&app)?.load_app_settings()
}

/// 保存 MVP 应用设置，并立即成为后续后端命令使用的配置。
#[tauri::command]
pub fn save_app_settings(app: AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    registry_for_app(&app)?.save_app_settings(&settings)?;
    Ok(settings)
}

/// 读取指定客户端的安装历史。
#[tauri::command]
pub fn list_install_history(
    app: AppHandle,
    client_installation_id: String,
) -> Result<Vec<InstallHistoryRecord>, String> {
    registry_for_app(&app)?.list_install_history(&client_installation_id)
}

/// 判断指定客户端可执行文件是否正在运行。
#[tauri::command]
pub fn is_client_running(path: String) -> Result<bool, String> {
    crate::process::is_client_running(Path::new(&path))
}

/// 从指定 URL 加载更新 manifest，并返回已校验的 manifest 内容。
#[tauri::command]
pub async fn load_manifest(
    url: String,
    network_route: Option<NetworkRouteConfig>,
) -> Result<crate::models::UpdateManifest, String> {
    crate::manifest::fetch_manifest_with_route(&url, network_route.as_ref()).await
}

/// 检查指定客户端和渠道是否存在可用更新。
#[tauri::command]
pub async fn check_client_update(
    app: AppHandle,
    request: CheckClientUpdateRequest,
) -> Result<Option<ClientUpdateCheck>, String> {
    if request_requires_manifest_url(&request) {
        required_manifest_url(request.manifest_url.as_deref())?;
    }
    let current_version = registry_for_app(&app)?
        .list_client_installations()?
        .into_iter()
        .find(|client| {
            crate::client_catalog::normalize_client_id(&client.client_id)
                == crate::client_catalog::normalize_client_id(&request.client_id)
                && client.is_default
        })
        .and_then(|client| client.version);

    crate::update_source::check_client_update(&request, current_version).await
}

/// 创建下载任务并开始真实下载更新包。
#[tauri::command]
pub async fn start_update_download(
    app: AppHandle,
    manager: DownloadManagerState<'_>,
    request: StartUpdateDownloadRequest,
) -> Result<DownloadJob, String> {
    let prepared = prepare_update_download_job(&app, request).await?;
    let job = prepared.job;
    let cache_path = PathBuf::from(&job.cache_path);
    manager.insert(job.clone())?;
    spawn_download_task(DownloadTaskContext {
        app,
        manager: manager.inner().clone(),
        job: job.clone(),
        cache_path,
        enabled_route_hosts: prepared.enabled_route_hosts,
    });

    Ok(job)
}

async fn prepare_update_download_job(
    app: &AppHandle,
    request: StartUpdateDownloadRequest,
) -> Result<PreparedUpdateDownload, String> {
    let client_installation_id = request.client_installation_id.clone();
    let network_route = request.network_route.clone();
    let registry = registry_for_app(app)?;
    let client = registry
        .list_client_installations()?
        .into_iter()
        .find(|client| client.id == client_installation_id)
        .ok_or_else(|| format!("client installation not found: {}", client_installation_id))?;
    let update_request = CheckClientUpdateRequest {
        client_id: client.client_id.clone(),
        channel: request.channel,
        manifest_url: request.manifest_url,
        platform: request.platform,
        network_route: network_route.clone(),
        use_manifest_source: request.use_manifest_source,
    };
    let mut update = crate::update_source::check_client_update(&update_request, client.version)
        .await?
        .ok_or_else(|| "no downloadable update is available for this client".to_string())?;
    if update.action != UpdateAction::Download {
        return Err(update
            .message
            .clone()
            .unwrap_or_else(|| "update source does not provide a downloadable asset".to_string()));
    }
    update.asset.asset_url = crate::manifest::build_asset_url_with_route(
        &update.asset.asset_url,
        network_route.as_ref(),
    )?
    .to_string();
    let enabled_route_hosts = network_route
        .as_ref()
        .map(|route| route.enabled_hosts.clone())
        .unwrap_or_default();

    let downloads_dir = app_cache_dir(app)?.join("downloads");
    let mut job =
        crate::download::create_download_job(&client_installation_id, &update, &downloads_dir);
    job.status = DownloadJobStatus::Downloading;
    Ok(PreparedUpdateDownload {
        job,
        enabled_route_hosts,
    })
}

/// 返回调用方显式配置的自维护 manifest 地址，未配置时拒绝继续请求。
pub(crate) fn required_manifest_url(input: Option<&str>) -> Result<&str, String> {
    input
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| "manifest url is not configured".to_string())
}

/// 判断当前更新检查请求是否必须显式提供 manifest URL。
pub(crate) fn request_requires_manifest_url(request: &CheckClientUpdateRequest) -> bool {
    request.use_manifest_source
}

fn spawn_download_task(context: DownloadTaskContext) {
    let job_id = context.job.id.clone();
    let job_for_task = context.job.clone();
    let app = context.app;
    let manager = context.manager;
    let cache_path = context.cache_path;
    let enabled_route_hosts = context.enabled_route_hosts;

    tokio::spawn(async move {
        let result = crate::download::download_asset_to_file(
            crate::download::DownloadFileRequest {
                asset_url: &job_for_task.asset_url,
                cache_path: &cache_path,
                expected_size: job_for_task.size,
                enabled_route_hosts: &enabled_route_hosts,
            },
            |downloaded_bytes| {
                let Ok(job) = manager.update(&job_id, |job| {
                    job.downloaded_bytes = downloaded_bytes;
                }) else {
                    return false;
                };
                let keep_running = job.status != DownloadJobStatus::Canceled;
                let _ = app.emit_to("main", "download-progress", job);
                keep_running
            },
        )
        .await
        .and_then(|_| {
            crate::download::verify_downloaded_file(
                &cache_path,
                &job_for_task.sha256,
                job_for_task.size,
            )
        });

        match result {
            Ok(()) => {
                if let Ok(job) = manager.update(&job_id, |job| {
                    job.status = DownloadJobStatus::Verified;
                    job.downloaded_bytes = job.size;
                    job.error = None;
                }) {
                    let _ = app.emit_to("main", "download-completed", job);
                }
            }
            Err(error) => {
                if manager.get(&job_id).is_ok_and(|job| {
                    job.is_some_and(|job| job.status == DownloadJobStatus::Canceled)
                }) {
                    return;
                }
                let _ = std::fs::remove_file(&cache_path);
                if let Ok(job) = manager.update(&job_id, |job| {
                    job.status = DownloadJobStatus::Failed;
                    job.error = Some(error);
                }) {
                    let _ = app.emit_to("main", "download-failed", job);
                }
            }
        }
    });
}

/// 取消下载任务。
#[tauri::command]
pub fn cancel_download(
    manager: DownloadManagerState<'_>,
    job_id: String,
) -> Result<DownloadJob, String> {
    manager.cancel(&job_id)
}

/// 查询下载任务状态。
#[tauri::command]
pub fn get_download_job(
    manager: DownloadManagerState<'_>,
    job_id: String,
) -> Result<Option<DownloadJob>, String> {
    manager.get(&job_id)
}

/// 校验并安装已下载的更新包。
#[tauri::command]
pub fn install_downloaded_update(
    app: AppHandle,
    manager: DownloadManagerState<'_>,
    job_id: String,
) -> Result<DownloadJob, String> {
    let job = manager
        .get(&job_id)?
        .ok_or_else(|| format!("download job not found: {job_id}"))?;
    if job.status != DownloadJobStatus::Verified {
        return Err(format!(
            "download job must be verified before install: {:?}",
            job.status
        ));
    }

    let registry = registry_for_app(&app)?;
    let mut client = match load_install_target(&registry, &job) {
        Ok(client) => client,
        Err(error) => {
            record_install_prepare_failure(&registry, &job, &error);
            return Err(error);
        }
    };
    run_install_transaction(
        InstallContext {
            app: &app,
            manager: manager.inner(),
            registry: &registry,
            job_id: &job_id,
            job: &job,
        },
        &mut client,
    )
}

fn record_install_prepare_failure(
    registry: &crate::registry::ClientRegistry,
    job: &DownloadJob,
    error: &str,
) {
    let Ok(Some(client)) = registry.client_installation_by_id(&job.client_installation_id) else {
        return;
    };
    let rollback_dir = crate::download::rollback_dir_for(
        Path::new(&client.install_dir),
        &format!("install-{}", job.id),
    );
    let _ = registry.record_install_history(&install_history_record(InstallHistoryInput {
        job,
        client: &client,
        rollback_dir: &rollback_dir,
        status: InstallHistoryStatus::Failed,
        error: Some(error.to_string()),
    }));
}

fn load_install_target(
    registry: &crate::registry::ClientRegistry,
    job: &DownloadJob,
) -> Result<ClientInstallation, String> {
    let mut client = registry
        .list_client_installations()?
        .into_iter()
        .find(|client| client.id == job.client_installation_id)
        .ok_or_else(|| {
            format!(
                "client installation not found: {}",
                job.client_installation_id
            )
        })?;
    let target_client = crate::client_scan::validate_client_dir(Path::new(&client.install_dir))?;
    if target_client.health != ClientHealth::Ok {
        return Err(format!(
            "target client is not healthy before install: {:?}",
            target_client.health
        ));
    }
    if crate::process::is_client_running(Path::new(&target_client.executable_path))? {
        return Err("target client is running; close it before install".to_string());
    }
    client.install_dir = target_client.install_dir;
    client.executable_path = target_client.executable_path;
    Ok(client)
}

fn run_install_transaction(
    context: InstallContext<'_>,
    client: &mut ClientInstallation,
) -> Result<DownloadJob, String> {
    let cache_path = PathBuf::from(&context.job.cache_path);
    let install_id = format!("install-{}", context.job.id);
    let cache_root = app_cache_dir(context.app)?;
    let staging_dir = cache_root.join("staging").join(&install_id);
    let rollback_dir =
        crate::download::rollback_dir_for(Path::new(&client.install_dir), &install_id);

    context.manager.update(context.job_id, |job| {
        job.status = DownloadJobStatus::Installing;
    })?;
    context
        .app
        .emit_to("main", "install-progress", context.job_id)
        .map_err(|error| format!("failed to emit install-progress: {error}"))?;

    let package_kind = crate::download::package_kind_for_asset_url(&context.job.asset_url);
    let install_result = crate::download::auto_install_guard(package_kind)
        .and_then(|_| {
            crate::download::verify_downloaded_file(
                &cache_path,
                &context.job.sha256,
                context.job.size,
            )
        })
        .and_then(|_| {
            crate::download::extract_package_to_staging(&cache_path, &staging_dir, package_kind)
        })
        .and_then(|_| crate::download::find_staged_client_dir(&staging_dir))
        .and_then(|staged_client_dir| {
            if crate::process::is_client_running(Path::new(&client.executable_path))? {
                return Err("target client is running; close it before install".to_string());
            }
            crate::download::install_staged_client(
                &staged_client_dir,
                Path::new(&client.install_dir),
                &rollback_dir,
            )
        });

    match install_result {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&staging_dir);
            finish_install_success(context, client, &rollback_dir)
        }
        Err(error) => finish_install_failure(context, error),
    }
}

fn finish_install_success(
    context: InstallContext<'_>,
    client: &mut ClientInstallation,
    rollback_dir: &Path,
) -> Result<DownloadJob, String> {
    client.version = Some(context.job.version.clone());
    client.health = ClientHealth::Ok;
    if let Err(error) = context.registry.upsert_client_installation(client) {
        let restore_message =
            match crate::download::restore_rollback(Path::new(&client.install_dir), rollback_dir) {
                Ok(()) => "rollback restored".to_string(),
                Err(restore_error) => format!("rollback restore failed: {restore_error}"),
            };
        return finish_install_failure(
            context,
            format!(
                "registry update failed after file replacement: {error}; {restore_message}; rollback_dir={}",
                rollback_dir.display()
            ),
        );
    }
    context
        .registry
        .record_install_history(&install_history_record(InstallHistoryInput {
            job: context.job,
            client,
            rollback_dir,
            status: InstallHistoryStatus::Completed,
            error: None,
        }))?;
    let job = context.manager.update(context.job_id, |job| {
        job.status = DownloadJobStatus::Completed;
        job.error = None;
    })?;
    context
        .app
        .emit_to("main", "install-completed", &job)
        .map_err(|error| format!("failed to emit install-completed: {error}"))?;
    Ok(job)
}

fn finish_install_failure(
    context: InstallContext<'_>,
    error: String,
) -> Result<DownloadJob, String> {
    if let Ok(client) = load_install_target(context.registry, context.job) {
        let rollback_dir = crate::download::rollback_dir_for(
            Path::new(&client.install_dir),
            &format!("install-{}", context.job.id),
        );
        let _ = context
            .registry
            .record_install_history(&install_history_record(InstallHistoryInput {
                job: context.job,
                client: &client,
                rollback_dir: &rollback_dir,
                status: InstallHistoryStatus::Failed,
                error: Some(error.clone()),
            }));
    }
    let job = context.manager.update(context.job_id, |job| {
        job.status = DownloadJobStatus::Failed;
        job.error = Some(error);
    })?;
    context
        .app
        .emit_to("main", "install-failed", &job)
        .map_err(|emit_error| format!("failed to emit install-failed: {emit_error}"))?;
    Ok(job)
}

fn install_history_record(input: InstallHistoryInput<'_>) -> InstallHistoryRecord {
    let completed_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .ok()
        .or_else(|| Some("1970-01-01T00:00:00Z".to_string()));
    InstallHistoryRecord {
        id: format!("install-{}", input.job.id),
        job_id: input.job.id.clone(),
        client_installation_id: input.client.id.clone(),
        client_id: input.job.client_id.clone(),
        version: input.job.version.clone(),
        asset_url: input.job.asset_url.clone(),
        package_kind: crate::download::package_kind_for_asset_url(&input.job.asset_url)
            .as_str()
            .to_string(),
        status: input.status,
        rollback_path: Some(input.rollback_dir.to_string_lossy().replace('\\', "/")),
        error: input.error,
        completed_at,
    }
}

fn registry_for_app(app: &AppHandle) -> Result<crate::registry::ClientRegistry, String> {
    let db_path = app_data_dir(app)?.join("ddnet-manager.sqlite");
    crate::registry::ClientRegistry::open(&db_path)
}

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data dir: {error}"))
}

fn app_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map_err(|error| format!("failed to resolve app cache dir: {error}"))
}

#[cfg(test)]
#[path = "test/commands.rs"]
mod tests;
