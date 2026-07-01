use crate::commands::{
    app_cache_dir, DownloadManagerState, DownloadTaskContext, PreparedUpdateDownload,
    RegistryState, LOCAL_SMOKE_RESULT_PATH_ENV,
};
use crate::download::verify::VerifyProgressSink;
use crate::download::DownloadManager;
use crate::error::{IpcError, ManagerError};
use crate::models::{
    CheckClientUpdateRequest, DownloadJob, DownloadJobRecovery, DownloadJobStatus,
    NetworkRouteConfig, StartUpdateDownloadRequest, UpdateAction,
};
use crate::registry::ClientRegistry;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

/// verify-progress 事件 payload。前端 useClientInstaller 监听后渲染校验进度条，
/// ratio = bytes_read / total（total 为 0 时 ratio = 0）。
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct VerifyProgressPayload {
    /// 下载任务关联的客户端安装记录 id（和 DownloadJob.client_installation_id 一致），
    /// 让前端 useClientInstaller 通过 client.id 判断事件归属（和 download-progress 同样模式）。
    client_installation_id: String,
    bytes_read: u64,
    total: u64,
}

/// TauriVerifySink 把 verify 进度 emit 到前端的 verify-progress 事件。
/// 与 TauriScanSink 模式一致：持有 AppHandle + 关联 id，每次 emit 调
/// `app.emit_to("main", "verify-progress", payload)`。emit 失败静默——前端
/// 已卸载监听时不应阻断校验。
///
/// 内部用 `Mutex<VerifyThrottle>` 节流（review issue M2）：100MB 文件约 400 个
/// 256KB 块，不节流会让前端 re-render 抖动 + IPC 序列化拖慢校验吞吐。节流策略：
/// 100ms 时间窗 + 总是 emit 第一次和最后一次（最终态由 verify 完成切 state 兜底）。
struct TauriVerifySink {
    app: AppHandle,
    client_installation_id: String,
    throttle: std::sync::Mutex<VerifyThrottle>,
}

impl TauriVerifySink {
    fn new(app: AppHandle, client_installation_id: String) -> Self {
        Self {
            app,
            client_installation_id,
            throttle: std::sync::Mutex::new(VerifyThrottle::new()),
        }
    }
}

impl VerifyProgressSink for TauriVerifySink {
    fn emit(&self, bytes_read: u64, total: u64) {
        // 节流：100ms 时间窗内只 emit 一次。最后一次 emit 由 verify 完成后
        // 前端切到 installed/failed state 兜底，不需要 here 强制 emit total。
        let should_emit = self
            .throttle
            .lock()
            .map(|mut t| t.should_emit(bytes_read, total))
            .unwrap_or(true);
        if !should_emit {
            return;
        }
        let payload = VerifyProgressPayload {
            client_installation_id: self.client_installation_id.clone(),
            bytes_read,
            total,
        };
        let _ = self.app.emit_to("main", "verify-progress", payload);
    }
}

/// verify 进度节流（review issue M2）：100ms 时间窗内只放行一次 emit。
struct VerifyThrottle {
    last_emit_at: Option<std::time::Instant>,
}

impl VerifyThrottle {
    fn new() -> Self {
        Self { last_emit_at: None }
    }

    fn should_emit(&mut self, _bytes_read: u64, _total: u64) -> bool {
        let now = std::time::Instant::now();
        let should = self
            .last_emit_at
            .map(|last| now.duration_since(last) >= std::time::Duration::from_millis(100))
            .unwrap_or(true);
        if should {
            self.last_emit_at = Some(now);
        }
        should
    }
}

/// 创建下载任务并开始真实下载更新包。
#[tauri::command]
pub async fn start_update_download(
    app: AppHandle,
    manager: DownloadManagerState<'_>,
    registry: RegistryState<'_>,
    request: StartUpdateDownloadRequest,
) -> Result<DownloadJob, IpcError> {
    let prepared = prepare_update_download_job(&registry, &app, request).await?;
    // 加载 AppSettings 取反代前缀和额外可信 host；设置缺失时用默认空列表
    let settings = registry.load_app_settings().unwrap_or_default();
    let mirror_prefixes = crate::mirror::resolve_prefixes(&settings.mirror_prefixes);
    // review C1：从反代前缀提取 host 自动合并进 extra_hosts，让默认反代域名
    // （gh-proxy.com 等）通过 SSRF 白名单校验。否则竞速胜出的反代源会被
    // validate_download_url 拒绝，导致裸连用户无法下载。
    let mut extra_hosts = settings.extra_trusted_hosts.clone();
    for host in crate::mirror::extract_mirror_hosts(&mirror_prefixes) {
        if !extra_hosts.iter().any(|h| h.eq_ignore_ascii_case(&host)) {
            extra_hosts.push(host);
        }
    }
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
        mirror_prefixes,
        extra_hosts,
    });

    Ok(job)
}

async fn prepare_update_download_job(
    registry: &ClientRegistry,
    app: &AppHandle,
    request: StartUpdateDownloadRequest,
) -> Result<PreparedUpdateDownload, ManagerError> {
    let client_installation_id = request.client_installation_id.clone();
    // 若用户未配置路由，自动探测并选择最佳网络路径（direct → auto_detect → local_proxy）。
    let network_route = match request.network_route.clone() {
        Some(route) => Some(route),
        None => crate::network_route::auto_select_route(None).await,
    };
    let client = registry
        .list_client_installations()?
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
    let cache_dir = app.path().cache_dir().ok();
    let update = crate::update_source::check_client_update(
        &update_request,
        client.version,
        cache_dir.as_deref(),
    )
    .await?;
    // update_source 现在始终返回 ClientUpdateCheck，无可用下载时 action != Download。
    // 用 reason 不可用时附带的 message 直接报错。
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
    // 提前取出 cancel token，让下载循环在 select! 中即时感知取消信号，
    // 而非等到下一个 chunk 边界。
    let cancel_token = context.manager.cancel_token_clone(&job_id);
    let runtime = DownloadTaskRuntime {
        app: context.app,
        manager: context.manager,
        registry: context.registry,
        job: context.job,
        cache_path: context.cache_path,
        route: context.route,
        mirror_prefixes: context.mirror_prefixes,
        extra_hosts: context.extra_hosts,
        cancel_token,
    };
    tokio::spawn(async move {
        let result = run_download_loop(&runtime).await;
        handle_download_result(&runtime, result).await;
    });
}

/// 下载后台任务运行时，所有字段 own 后整块 move 进 `tokio::spawn`。
struct DownloadTaskRuntime {
    app: AppHandle,
    manager: DownloadManager,
    registry: ClientRegistry,
    job: DownloadJob,
    cache_path: PathBuf,
    route: Option<NetworkRouteConfig>,
    /// 已 resolve 的反代前缀列表（空配置已 fallback 到默认列表）。
    mirror_prefixes: Vec<String>,
    /// 用户显式信任的额外下载 host（公共反代域名）。
    extra_hosts: Vec<String>,
    cancel_token: Option<CancellationToken>,
}

/// 执行下载 + 校验，进度回调把 downloaded_bytes 实时同步到 manager 与 registry。
///
/// C2: progress callback 节流到 100ms 或 1% 进度变化才 emit + persist。每个 chunk
/// 仍同步更新 manager 内存状态（轻量），但 sqlite 持久化 + IPC emit 受节流保护。
/// 最后一个 chunk 的 emit 由 download-completed 事件兜底（即使这里跳过，下载完成
/// 后 handle_download_success 仍会 emit 一次最终状态）。
async fn run_download_loop(runtime: &DownloadTaskRuntime) -> Result<(), String> {
    let mut throttle = DownloadThrottle::new(runtime.job.size);
    // 组装竞速候选 URL：原始 GitHub URL + 反代 URL（反代前缀已 resolve）
    let candidate_urls =
        crate::mirror::build_candidate_urls(&runtime.job.asset_url, &runtime.mirror_prefixes);
    crate::download::download_asset_to_file(
        crate::download::DownloadFileRequest {
            asset_url: &runtime.job.asset_url,
            cache_path: &runtime.cache_path,
            expected_size: runtime.job.size,
            route: runtime.route.as_ref(),
            candidate_urls: &candidate_urls,
            extra_hosts: &runtime.extra_hosts,
        },
        runtime.cancel_token.clone(),
        |downloaded_bytes| {
            let Ok(job) = runtime.manager.update(&runtime.job.id, |job| {
                job.downloaded_bytes = downloaded_bytes;
            }) else {
                return false;
            };
            let keep_running = job.status != DownloadJobStatus::Canceled;
            if throttle.should_emit(downloaded_bytes) {
                persist_download_job_snapshot(&runtime.registry, &job);
                let _ = runtime.app.emit_to("main", "download-progress", job);
            }
            keep_running
        },
    )
    .await
    .and_then(|_| {
        // C1: 校验阶段也 emit 进度，让 DownloadButton 在 verify 大文件时显示
        // 进度条而不是 spinner。client_installation_id 让前端按当前 client.id
        // 过滤事件归属（和 download-progress 同模式）。
        let sink = TauriVerifySink::new(
            runtime.app.clone(),
            runtime.job.client_installation_id.clone(),
        );
        crate::download::verify::verify_downloaded_file_with_progress(
            &runtime.cache_path,
            &runtime.job.sha256,
            runtime.job.size,
            &sink,
        )
        .map_err(|error| error.to_string())
    })
}

/// 下载进度节流阈值：100ms 时间窗或 1% 进度变化，任一满足才 emit + persist。
/// 避免高频 chunk（GitHub release 几 MB/s，每 8-32KB 一个 chunk）让 sqlite 写
/// 和 IPC emit 爆掉。
const DOWNLOAD_EMIT_MIN_INTERVAL_MS: u64 = 100;
const DOWNLOAD_EMIT_MIN_PROGRESS_PERCENT: u64 = 1;

/// 下载进度节流状态。download callback 每次调用 ask `should_emit`，返回 true
/// 时才做 persist + emit，并更新内部 last 时间戳和字节数。
pub(crate) struct DownloadThrottle {
    last_emit_at: std::time::Instant,
    last_emit_bytes: u64,
    total: u64,
}

impl DownloadThrottle {
    /// 用文件总字节数构造。total=0 时只靠时间分支节流（progress 分支跳过）。
    pub(crate) fn new(total: u64) -> Self {
        Self {
            last_emit_at: std::time::Instant::now(),
            last_emit_bytes: 0,
            total,
        }
    }

    /// 判断当前 chunk 是否应该 emit + persist。返回 true 时内部状态已更新，
    /// 调用方负责真做 persist_download_job_snapshot + app.emit。
    pub(crate) fn should_emit(&mut self, downloaded_bytes: u64) -> bool {
        let now = std::time::Instant::now();
        let time_for_emit = now.duration_since(self.last_emit_at)
            >= std::time::Duration::from_millis(DOWNLOAD_EMIT_MIN_INTERVAL_MS);
        let bytes_delta = downloaded_bytes.saturating_sub(self.last_emit_bytes);
        let progress_for_emit =
            self.total > 0 && bytes_delta * 100 / self.total >= DOWNLOAD_EMIT_MIN_PROGRESS_PERCENT;
        if time_for_emit || progress_for_emit {
            self.last_emit_at = now;
            self.last_emit_bytes = downloaded_bytes;
            true
        } else {
            false
        }
    }
}

/// 处理下载/校验结果：成功切 Verified + emit completed；失败切 Failed + emit failed。
async fn handle_download_result(runtime: &DownloadTaskRuntime, result: Result<(), String>) {
    match result {
        Ok(()) => handle_download_success(runtime).await,
        Err(error) => handle_download_failure(runtime, error).await,
    }
}

async fn handle_download_success(runtime: &DownloadTaskRuntime) {
    let job_id = runtime.job.id.clone();
    let Ok(job) = runtime.manager.update(&job_id, |job| {
        job.status = DownloadJobStatus::Verified;
        job.downloaded_bytes = job.size;
        job.error = None;
    }) else {
        return;
    };
    match persist_download_job_snapshot_result(&runtime.registry, &job) {
        Ok(()) => {
            // review issue M1：节流可能跳过最后一个 chunk 的 emit，前端 downloading
            // 进度会卡在 <100% 直接跳到 verifying 0%。emit 一次最终 download-progress
            // （status 临时改 Downloading，前端 isMine 只看 id 不看 status）把进度
            // 推到 100%，再 emit download-completed 切到 verifying 阶段。
            let mut final_progress = job.clone();
            final_progress.status = DownloadJobStatus::Downloading;
            let _ = runtime
                .app
                .emit_to("main", "download-progress", &final_progress);
            let _ = runtime.app.emit_to("main", "download-completed", job);
        }
        Err(error) => {
            if let Ok(job) = runtime.manager.update(&job_id, |job| {
                job.status = DownloadJobStatus::Failed;
                job.error = Some(error.to_string());
            }) {
                persist_download_job_snapshot(&runtime.registry, &job);
                let _ = runtime.app.emit_to("main", "download-failed", job);
            }
        }
    }
}

async fn handle_download_failure(runtime: &DownloadTaskRuntime, error: String) {
    let job_id = runtime.job.id.clone();
    // 取消导致的失败不写 Failed 状态：cancel 路径已经把 status=Canceled 持久化。
    if runtime
        .manager
        .get(&job_id)
        .is_ok_and(|job| job.is_some_and(|job| job.status == DownloadJobStatus::Canceled))
    {
        return;
    }
    let _ = std::fs::remove_file(&runtime.cache_path);
    if let Ok(job) = runtime.manager.update(&job_id, |job| {
        job.status = DownloadJobStatus::Failed;
        job.error = Some(error);
    }) {
        persist_download_job_snapshot(&runtime.registry, &job);
        let _ = runtime.app.emit_to("main", "download-failed", job);
    }
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
) -> Result<(), ManagerError> {
    registry.upsert_download_job(job)
}

/// 将下载任务切换为安装中状态，并在文件替换前持久化该快照。
pub(crate) fn enter_installing_snapshot(
    manager: &DownloadManager,
    registry: &ClientRegistry,
    job_id: &str,
) -> Result<DownloadJob, ManagerError> {
    let previous = manager
        .get(job_id)
        .map_err(ManagerError::Internal)?
        .ok_or_else(|| ManagerError::NotFound(format!("download job not found: {job_id}")))?;
    let job = manager
        .update(job_id, |job| {
            job.status = DownloadJobStatus::Installing;
        })
        .map_err(ManagerError::Internal)?;
    if let Err(error) = registry.upsert_download_job(&job) {
        let rollback_result = manager.update(job_id, |job| {
            *job = previous;
        });
        if let Err(rollback_error) = rollback_result {
            return Err(ManagerError::Internal(format!(
                "{error}; failed to restore in-memory download job: {rollback_error}"
            )));
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
) -> Result<DownloadJob, ManagerError> {
    let job = manager
        .update(job_id, |job| {
            job.status = DownloadJobStatus::Completed;
            job.error = None;
        })
        .map_err(ManagerError::Internal)?;
    registry.upsert_download_job(&job)?;
    Ok(job)
}

/// 从内存管理器或注册表读取下载任务快照，注册表中存在但内存中不存在时自动恢复。
pub(crate) fn load_download_job_snapshot(
    manager: &DownloadManager,
    registry: &ClientRegistry,
    job_id: &str,
) -> Result<Option<DownloadJob>, ManagerError> {
    if let Some(job) = manager.get(job_id).map_err(ManagerError::Internal)? {
        return Ok(Some(job));
    }
    let Some(job) = registry.download_job_by_id(job_id)? else {
        return Ok(None);
    };
    manager
        .insert(job.clone())
        .map_err(ManagerError::Internal)?;
    Ok(Some(job))
}

fn list_download_job_recoveries_from_registry(
    registry: &ClientRegistry,
    client_installation_id: Option<&str>,
) -> Result<Vec<DownloadJobRecovery>, ManagerError> {
    registry
        .list_download_jobs(client_installation_id)?
        .into_iter()
        .map(|job| {
            crate::download::build_download_job_recovery(&job).map_err(ManagerError::Internal)
        })
        .collect()
}

#[cfg(test)]
#[path = "../test/commands.rs"]
mod tests;
