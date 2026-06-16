use crate::commands::{
    app_cache_dir, DownloadManagerState, DownloadTaskContext, PreparedUpdateDownload,
    RegistryState, LOCAL_SMOKE_RESULT_PATH_ENV,
};
use crate::download::DownloadManager;
use crate::models::{
    CheckClientUpdateRequest, DownloadJob, DownloadJobRecovery, DownloadJobStatus, IpcError,
    ManagerError, StartUpdateDownloadRequest, UpdateAction,
};
use crate::registry::ClientRegistry;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

/// 创建下载任务并开始真实下载更新包。
#[tauri::command]
pub async fn start_update_download(
    app: AppHandle,
    manager: DownloadManagerState<'_>,
    registry: RegistryState<'_>,
    request: StartUpdateDownloadRequest,
) -> Result<DownloadJob, IpcError> {
    let prepared = prepare_update_download_job(&registry, &app, request).await?;
    let job = prepared.job;
    let cache_path = PathBuf::from(&job.cache_path);
    registry.upsert_download_job(&job)?;
    manager.insert(job.clone())?;
    spawn_download_task(DownloadTaskContext {
        app,
        registry: registry.inner().clone(),
        manager: manager.inner().clone(),
        job: job.clone(),
        cache_path,
        route: prepared.route,
    });

    Ok(job)
}

async fn prepare_update_download_job(
    registry: &ClientRegistry,
    app: &AppHandle,
    request: StartUpdateDownloadRequest,
) -> Result<PreparedUpdateDownload, ManagerError> {
    let client_installation_id = request.client_installation_id.clone();
    let network_route = request.network_route.clone();
    let client = registry
        .list_client_installations()
        .map_err(ManagerError::Internal)?
        .into_iter()
        .find(|client| client.id == client_installation_id)
        .ok_or_else(|| {
            ManagerError::NotFound(format!(
                "client installation not found: {}",
                client_installation_id
            ))
        })?;
    let update_request = CheckClientUpdateRequest {
        client_id: client.client_id.clone(),
        channel: request.channel,
        manifest_url: request.manifest_url,
        platform: request.platform,
        network_route: network_route.clone(),
        use_manifest_source: request.use_manifest_source,
    };
    let update = crate::update_source::check_client_update(&update_request, client.version)
        .await
        .map_err(ManagerError::Internal)?
        .ok_or_else(|| {
            ManagerError::Internal(
                "no downloadable update is available for this client".to_string(),
            )
        })?;
    if update.action != UpdateAction::Download {
        return Err(ManagerError::Internal(
            update.message.clone().unwrap_or_else(|| {
                "update source does not provide a downloadable asset".to_string()
            }),
        ));
    }
    let downloads_dir = app_cache_dir(app)
        .map_err(ManagerError::Internal)?
        .join("downloads");
    let mut job =
        crate::download::create_download_job(&client_installation_id, &update, &downloads_dir);
    job.status = DownloadJobStatus::Downloading;
    Ok(PreparedUpdateDownload {
        job,
        route: network_route,
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

/// 返回本地 smoke 结果文件路径，要求显式通过环境变量配置。
pub(crate) fn required_local_smoke_result_path() -> Result<PathBuf, String> {
    std::env::var(LOCAL_SMOKE_RESULT_PATH_ENV)
        .ok()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "local smoke result path is not configured".to_string())
}

/// 返回本地 smoke 结果写入使用的同目录临时文件路径。
pub(crate) fn local_smoke_result_temp_path(output_path: &Path) -> Result<PathBuf, String> {
    let file_name = output_path
        .file_name()
        .ok_or_else(|| "local smoke result path must include a file name".to_string())?;
    let mut temp_file_name = file_name.to_os_string();
    temp_file_name.push(".tmp");

    Ok(output_path.with_file_name(temp_file_name))
}

/// 将本地 smoke 验收结果写入 JSON 文件，仅允许在已启用 local smoke 时调用。
pub(crate) fn write_local_smoke_result_report(
    result: &crate::models::LocalSmokeResultReport,
) -> Result<(), String> {
    if !crate::local_smoke::is_local_smoke_enabled() {
        return Err("local smoke reporting is not enabled".to_string());
    }

    let output_path = required_local_smoke_result_path()?;
    if let Some(parent) = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create local smoke result dir: {error}"))?;
    }

    let payload = serde_json::to_string_pretty(result)
        .map_err(|error| format!("failed to serialize local smoke result: {error}"))?;
    let temp_path = local_smoke_result_temp_path(&output_path)?;
    fs::write(&temp_path, payload).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        format!("failed to write local smoke result: {error}")
    })?;
    fs::rename(&temp_path, &output_path).map_err(|error| {
        let _ = fs::remove_file(&temp_path);
        format!("failed to replace local smoke result: {error}")
    })
}

fn spawn_download_task(context: DownloadTaskContext) {
    let job_id = context.job.id.clone();
    let job_for_task = context.job.clone();
    let app = context.app;
    let manager = context.manager;
    let registry = context.registry;
    let cache_path = context.cache_path;
    let route = context.route;
    // 提前取出 cancel token，让下载循环在 select! 中即时感知取消信号，
    // 而非等到下一个 chunk 边界。
    let cancel_token = manager.cancel_token_clone(&job_id);

    tokio::spawn(async move {
        let result = crate::download::download_asset_to_file(
            crate::download::DownloadFileRequest {
                asset_url: &job_for_task.asset_url,
                cache_path: &cache_path,
                expected_size: job_for_task.size,
                route: route.as_ref(),
            },
            cancel_token.clone(),
            |downloaded_bytes| {
                let Ok(job) = manager.update(&job_id, |job| {
                    job.downloaded_bytes = downloaded_bytes;
                }) else {
                    return false;
                };
                persist_download_job_snapshot(&registry, &job);
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
            .map_err(|error| error.to_string())
        });

        match result {
            Ok(()) => {
                if let Ok(job) = manager.update(&job_id, |job| {
                    job.status = DownloadJobStatus::Verified;
                    job.downloaded_bytes = job.size;
                    job.error = None;
                }) {
                    match persist_download_job_snapshot_result(&registry, &job) {
                        Ok(()) => {
                            let _ = app.emit_to("main", "download-completed", job);
                        }
                        Err(error) => {
                            if let Ok(job) = manager.update(&job_id, |job| {
                                job.status = DownloadJobStatus::Failed;
                                job.error = Some(error);
                            }) {
                                persist_download_job_snapshot(&registry, &job);
                                let _ = app.emit_to("main", "download-failed", job);
                            }
                        }
                    }
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
                    persist_download_job_snapshot(&registry, &job);
                    let _ = app.emit_to("main", "download-failed", job);
                }
            }
        }
    });
}

/// 取消下载任务。
#[tauri::command]
pub fn cancel_download(
    registry: RegistryState<'_>,
    manager: DownloadManagerState<'_>,
    job_id: String,
) -> Result<DownloadJob, IpcError> {
    let job = manager.cancel(&job_id)?;
    registry.upsert_download_job(&job)?;
    Ok(job)
}

/// 查询下载任务状态。
#[tauri::command]
pub fn get_download_job(
    registry: RegistryState<'_>,
    manager: DownloadManagerState<'_>,
    job_id: String,
) -> Result<Option<DownloadJob>, IpcError> {
    load_download_job_snapshot(manager.inner(), registry.inner(), &job_id).map_err(IpcError::from)
}

/// 返回指定客户端当前可恢复的下载任务摘要。
#[tauri::command]
pub fn list_download_job_recoveries(
    registry: RegistryState<'_>,
    client_installation_id: Option<String>,
) -> Result<Vec<DownloadJobRecovery>, IpcError> {
    list_download_job_recoveries_from_registry(registry.inner(), client_installation_id.as_deref())
        .map_err(IpcError::from)
}

/// 把下载任务快照写入注册表；事件由调用方按当前阶段统一发射，避免重复事件。
fn persist_download_job_snapshot(registry: &ClientRegistry, job: &DownloadJob) {
    let _ = registry.upsert_download_job(job);
}

fn persist_download_job_snapshot_result(
    registry: &ClientRegistry,
    job: &DownloadJob,
) -> Result<(), String> {
    registry.upsert_download_job(job)
}

/// 将下载任务切换为安装中状态，并在文件替换前持久化该快照。
pub(crate) fn enter_installing_snapshot(
    manager: &DownloadManager,
    registry: &ClientRegistry,
    job_id: &str,
) -> Result<DownloadJob, String> {
    let previous = manager
        .get(job_id)?
        .ok_or_else(|| format!("download job not found: {job_id}"))?;
    let job = manager.update(job_id, |job| {
        job.status = DownloadJobStatus::Installing;
    })?;
    if let Err(error) = registry.upsert_download_job(&job) {
        let rollback_result = manager.update(job_id, |job| {
            *job = previous;
        });
        if let Err(rollback_error) = rollback_result {
            return Err(format!(
                "{error}; failed to restore in-memory download job: {rollback_error}"
            ));
        }
        return Err(error);
    }
    Ok(job)
}

/// 将下载任务切换为已完成状态，并在安装历史写入前持久化主状态。
pub(crate) fn complete_download_job_snapshot(
    manager: &DownloadManager,
    registry: &ClientRegistry,
    job_id: &str,
) -> Result<DownloadJob, String> {
    let job = manager.update(job_id, |job| {
        job.status = DownloadJobStatus::Completed;
        job.error = None;
    })?;
    registry.upsert_download_job(&job)?;
    Ok(job)
}

/// 从内存管理器或注册表读取下载任务快照，注册表中存在但内存中不存在时自动恢复。
pub(crate) fn load_download_job_snapshot(
    manager: &DownloadManager,
    registry: &ClientRegistry,
    job_id: &str,
) -> Result<Option<DownloadJob>, String> {
    if let Some(job) = manager.get(job_id)? {
        return Ok(Some(job));
    }
    let Some(job) = registry.download_job_by_id(job_id)? else {
        return Ok(None);
    };
    manager.insert(job.clone())?;
    Ok(Some(job))
}

fn list_download_job_recoveries_from_registry(
    registry: &ClientRegistry,
    client_installation_id: Option<&str>,
) -> Result<Vec<DownloadJobRecovery>, String> {
    registry
        .list_download_jobs(client_installation_id)?
        .into_iter()
        .map(|job| crate::download::build_download_job_recovery(&job))
        .collect()
}

#[cfg(test)]
#[path = "../test/commands.rs"]
mod tests;
