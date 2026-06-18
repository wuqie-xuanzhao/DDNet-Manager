use crate::commands::{
    app_cache_dir, DownloadManagerState, InstallContext, InstallHistoryInput, RegistryState,
};
use crate::error::{IpcError, ManagerError};
use crate::models::{
    ClientHealth, ClientInstallation, DownloadJob, DownloadJobStatus, InstallHistoryRecord,
    InstallHistoryStatus,
};
use crate::registry::fingerprints::FingerprintRecord;
use crate::registry::ClientRegistry;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// 校验并安装已下载的更新包。
///
/// 校验与状态切换在 IPC 调用线程同步执行（轻量、快速失败）；后续的解压、拷贝、
/// rename 等重 IO 通过 `spawn_blocking` 移到 blocking 线程池，避免长时间阻塞
/// Tauri IPC runtime。重 IO 完成后再回到调用线程返回最终 job 状态。
#[tauri::command]
pub async fn install_downloaded_update(
    app: AppHandle,
    manager: DownloadManagerState<'_>,
    registry: RegistryState<'_>,
    job_id: String,
) -> Result<DownloadJob, IpcError> {
    let job = match crate::commands::download::load_download_job_snapshot(
        manager.inner(),
        registry.inner(),
        &job_id,
    )? {
        Some(job) => job,
        None => {
            return Err(IpcError::from(ManagerError::NotFound(format!(
                "download job not found: {job_id}"
            ))));
        }
    };
    if !matches!(
        job.status,
        DownloadJobStatus::Verified | DownloadJobStatus::Failed
    ) {
        return Err(IpcError::from(ManagerError::Internal(format!(
            "download job must be verified before install: {:?}",
            job.status
        ))));
    }
    let recovery =
        crate::download::build_download_job_recovery(&job).map_err(ManagerError::Internal)?;
    if !recovery.can_install {
        return Err(IpcError::from(ManagerError::Internal(format!(
            "download job cache is not installable: {:?}",
            recovery.cache_state
        ))));
    }

    let mut client = match load_install_target(registry.inner(), &job) {
        Ok(client) => client,
        Err(error) => {
            record_install_prepare_failure(registry.inner(), &job, &error.to_string());
            return Err(IpcError::from(error));
        }
    };

    let context = InstallContext {
        app: app.clone(),
        manager: manager.inner().clone(),
        registry: registry.inner().clone(),
        job_id: job_id.clone(),
        job: job.clone(),
    };
    // 同步切换 Installing 快照并通知前端开始，让 UI 立即看到状态变化。
    enter_installing_state(&context)?;

    // 重 IO 移到 blocking 线程，避免占用 tokio worker。
    let blocking_result =
        tokio::task::spawn_blocking(move || run_install_blocking(context, &mut client))
            .await
            .map_err(|join_error| {
                ManagerError::Internal(format!("install blocking task panicked: {join_error}"))
            });

    match blocking_result {
        Ok(Ok(job)) => Ok(job),
        Ok(Err(error)) => {
            // blocking 任务内部已经把 job 标 Failed 并 emit install-failed，这里只
            // 把 ManagerError 转回 IpcError 返回给调用方。
            Err(IpcError::from(error))
        }
        Err(error) => Err(IpcError::from(error)),
    }
}

/// 同步切换 download job 到 Installing 状态，并在切换成功后通知前端。
fn enter_installing_state(context: &InstallContext) -> Result<(), ManagerError> {
    crate::commands::download::enter_installing_snapshot(
        &context.manager,
        &context.registry,
        &context.job_id,
    )?;
    context
        .app
        .emit_to("main", "install-progress", &context.job_id)
        .map_err(|error| {
            ManagerError::Internal(format!("failed to emit install-progress: {error}"))
        })?;
    Ok(())
}

/// blocking 线程内执行的事务主体：解压、校验、安装目录替换与持久化。
fn run_install_blocking(
    context: InstallContext,
    client: &mut ClientInstallation,
) -> Result<DownloadJob, ManagerError> {
    let cache_path = PathBuf::from(&context.job.cache_path);
    let install_id = format!("install-{}", context.job.id);
    let cache_root = app_cache_dir(&context.app).map_err(ManagerError::Internal)?;
    let staging_dir = cache_root.join("staging").join(&install_id);
    let rollback_dir =
        crate::download::install::rollback_dir_for(Path::new(&client.install_dir), &install_id);

    let package_kind = crate::download::package_kind_for_asset_url(&context.job.asset_url);
    let install_result: Result<(), ManagerError> =
        crate::download::auto_install_guard(package_kind)
            .map_err(ManagerError::Internal)
            .and_then(|_| {
                crate::download::verify_downloaded_file(
                    &cache_path,
                    &context.job.sha256,
                    context.job.size,
                )
            })
            .and_then(|_| {
                crate::download::extract_package_to_staging(&cache_path, &staging_dir, package_kind)
                    .map_err(ManagerError::Internal)
            })
            .and_then(|_| {
                crate::download::find_staged_client_dir(&staging_dir)
                    .map_err(ManagerError::Internal)
            })
            .and_then(|staged_client_dir| {
                if crate::process::is_client_running(Path::new(&client.executable_path))? {
                    return Err(ManagerError::ClientRunning(
                        "target client is running; close it before install".to_string(),
                    ));
                }
                crate::download::install::install_staged_client(
                    &staged_client_dir,
                    Path::new(&client.install_dir),
                    &rollback_dir,
                )
                .map_err(ManagerError::Internal)
            });

    match install_result {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&staging_dir);
            finish_install_success(context, client, &rollback_dir)
        }
        Err(error) => finish_install_failure(context, error),
    }
}

fn record_install_prepare_failure(registry: &ClientRegistry, job: &DownloadJob, error: &str) {
    let Ok(Some(client)) = registry.client_installation_by_id(&job.client_installation_id) else {
        return;
    };
    let rollback_dir = crate::download::install::rollback_dir_for(
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
    registry: &ClientRegistry,
    job: &DownloadJob,
) -> Result<ClientInstallation, ManagerError> {
    let mut client = registry
        .list_client_installations()?
        .into_iter()
        .find(|client| client.id == job.client_installation_id)
        .ok_or_else(|| {
            ManagerError::NotFound(format!(
                "client installation not found: {}",
                job.client_installation_id
            ))
        })?;
    let target_client = crate::client_scan::validate_client_dir(Path::new(&client.install_dir))?;
    if target_client.health != ClientHealth::Ok {
        return Err(ManagerError::Internal(format!(
            "target client is not healthy before install: {:?}",
            target_client.health
        )));
    }
    if crate::process::is_client_running(Path::new(&target_client.executable_path))? {
        return Err(ManagerError::ClientRunning(
            "target client is running; close it before install".to_string(),
        ));
    }
    client.install_dir = target_client.install_dir;
    client.executable_path = target_client.executable_path;
    Ok(client)
}

fn finish_install_success(
    context: InstallContext,
    client: &mut ClientInstallation,
    rollback_dir: &Path,
) -> Result<DownloadJob, ManagerError> {
    // 保留事务前的 version / executable_path 快照。若 upsert 失败后我们调用
    // restore_rollback 把磁盘回滚到旧版本，内存 client 也必须同步回到旧值，
    // 否则后续 record_install_history 会把"version=新 但磁盘=旧"的不一致状态
    // 写入历史记录。
    let previous_version = client.version.clone();
    let previous_executable_path = client.executable_path.clone();

    client.version = Some(context.job.version.clone());
    client.health = ClientHealth::Ok;

    // 记录 sha256 指纹到 registry：下载包的 sha256 已经过 verify_downloaded_file
    // 校验通过，可信；解析刚落地 exe 的 PE 元信息一并存入，供后续扫描升级识别 +
    // 设置页展示。失败不阻断 install 主流程（指纹缺失只会让扫描识别降级）。
    let _ = record_fingerprint_after_install(&context, client);

    if let Err(error) = context.registry.upsert_client_installation(client) {
        let restore_message = match crate::download::install::restore_rollback(
            Path::new(&client.install_dir),
            rollback_dir,
        ) {
            Ok(()) => {
                // 磁盘已回滚到旧版本，内存 client 必须同步，避免 history 记录错版本。
                client.version = previous_version.clone();
                client.executable_path = previous_executable_path.clone();
                "rollback restored".to_string()
            }
            Err(restore_error) => format!("rollback restore failed: {restore_error}"),
        };
        return finish_install_failure(
            context,
            ManagerError::Internal(format!(
                "registry update failed after file replacement: {error}; {restore_message}; rollback_dir={}",
                rollback_dir.display()
            )),
        );
    }
    let job = crate::commands::download::complete_download_job_snapshot(
        &context.manager,
        &context.registry,
        &context.job_id,
    )?;
    let _ = context
        .registry
        .record_install_history(&install_history_record(InstallHistoryInput {
            job: &context.job,
            client,
            rollback_dir,
            status: InstallHistoryStatus::Completed,
            error: None,
        }));
    context
        .app
        .emit_to("main", "install-completed", &job)
        .map_err(|error| {
            ManagerError::Internal(format!("failed to emit install-completed: {error}"))
        })?;
    Ok(job)
}

/// 下载安装成功后记录 sha256 指纹。从刚落地的 exe 读 PE VS_VERSION_INFO，
/// 把 (sha256, client_id, version, company, product) 一起写到 registry。
/// 失败不阻断 install 主流程。
fn record_fingerprint_after_install(
    context: &InstallContext,
    client: &ClientInstallation,
) -> Result<(), ManagerError> {
    let exe_path = Path::new(&client.executable_path);
    let (company, product) = match ntfs_search::read_version_info(exe_path) {
        Ok(vi) => (vi.company_name, vi.product_name),
        Err(_) => (None, None),
    };
    let version_str = context.job.version.as_str();
    context.registry.record_client_fingerprint(FingerprintRecord {
        sha256: &context.job.sha256,
        client_id: &client.client_id,
        display_name: &client.display_name,
        version: Some(version_str),
        company_name: company.as_deref(),
        product_name: product.as_deref(),
    })
}

fn finish_install_failure(
    context: InstallContext,
    error: ManagerError,
) -> Result<DownloadJob, ManagerError> {
    if let Ok(client) = load_install_target(&context.registry, &context.job) {
        let rollback_dir = crate::download::install::rollback_dir_for(
            Path::new(&client.install_dir),
            &format!("install-{}", context.job.id),
        );
        let _ = context
            .registry
            .record_install_history(&install_history_record(InstallHistoryInput {
                job: &context.job,
                client: &client,
                rollback_dir: &rollback_dir,
                status: InstallHistoryStatus::Failed,
                error: Some(error.to_string()),
            }));
    }
    let error_message = error.to_string();
    let job = context
        .manager
        .update(&context.job_id, |job| {
            job.status = DownloadJobStatus::Failed;
            job.error = Some(error_message);
        })
        .map_err(ManagerError::Internal)?;
    context.registry.upsert_download_job(&job)?;
    context
        .app
        .emit_to("main", "install-failed", &job)
        .map_err(|emit_error| {
            ManagerError::Internal(format!("failed to emit install-failed: {emit_error}"))
        })?;
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
