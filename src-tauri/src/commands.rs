/// 下载与安装子命令。
pub mod download;

/// 安装事务子命令。
pub mod install;

use crate::models::{
    AppSettings, CheckClientUpdateRequest, ClientHealth, ClientInstallation, ClientUpdateCheck,
    DownloadJob, InstallHistoryRecord, InstallHistoryStatus, IpcError, LocalSmokeResultReport,
    NetworkRouteConfig, ScanClientInstallationsOptions, UpsertClientInstallationRequest,
};
use crate::registry::ClientRegistry;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager, State};

/// 下载管理器的 Tauri managed 状态类型别名。
pub(crate) type DownloadManagerState<'a> = State<'a, crate::download::DownloadManager>;

/// 客户端注册表的 Tauri managed 状态类型别名。
pub(crate) type RegistryState<'a> = State<'a, ClientRegistry>;

/// 安装事务运行时的共享上下文。
///
/// 所有字段都 own，便于把整个 context move 进 `tokio::task::spawn_blocking`，
/// 避免在 IPC 调用线程上执行重 IO（SHA-256 校验、解压、目录拷贝）。
pub(crate) struct InstallContext {
    pub(crate) app: AppHandle,
    pub(crate) manager: crate::download::DownloadManager,
    pub(crate) registry: ClientRegistry,
    pub(crate) job_id: String,
    pub(crate) job: DownloadJob,
}

/// 后台下载任务的全局上下文，进入 tokio::spawn 前组装。
pub(crate) struct DownloadTaskContext {
    pub(crate) app: AppHandle,
    pub(crate) registry: ClientRegistry,
    pub(crate) manager: crate::download::DownloadManager,
    pub(crate) job: DownloadJob,
    pub(crate) cache_path: PathBuf,
    pub(crate) route: Option<NetworkRouteConfig>,
}

/// 下载任务准备阶段的输出结构。
pub(crate) struct PreparedUpdateDownload {
    pub(crate) job: DownloadJob,
    pub(crate) route: Option<NetworkRouteConfig>,
}

/// 安装历史记录写入的输入参数聚合。
pub(crate) struct InstallHistoryInput<'a> {
    pub(crate) job: &'a DownloadJob,
    pub(crate) client: &'a ClientInstallation,
    pub(crate) rollback_dir: &'a Path,
    pub(crate) status: InstallHistoryStatus,
    pub(crate) error: Option<String>,
}

/// 本地 smoke 结果路径环境变量名。
pub(crate) const LOCAL_SMOKE_RESULT_PATH_ENV: &str = "DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH";

/// 验证用户选择的客户端目录，并返回识别出的安装信息。
#[tauri::command]
pub fn validate_client_dir(path: String) -> Result<crate::models::ClientInstallation, String> {
    crate::client_scan::validate_client_dir(Path::new(&path))
}

/// 扫描本机候选客户端安装目录。
#[tauri::command]
pub fn scan_client_installations(
    registry: RegistryState<'_>,
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
            registry
                .list_client_installations()?
                .into_iter()
                .map(|client| PathBuf::from(client.install_dir)),
        );
    }
    let settings = registry.load_app_settings()?;

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
    registry: RegistryState<'_>,
    request: UpsertClientInstallationRequest,
) -> Result<ClientInstallation, String> {
    let mut client = crate::client_scan::validate_client_dir(Path::new(&request.install_dir))?;
    if request.is_default
        && crate::client_scan::is_local_smoke_tmp_path(Path::new(&client.install_dir))
    {
        return Err("local smoke client cannot be saved as default".to_string());
    }
    client.is_default = request.is_default;
    registry.upsert_client_installation(&client)?;
    Ok(client)
}

/// 从注册表移除客户端记录，不删除本地文件。
#[tauri::command]
pub fn remove_client_installation(registry: RegistryState<'_>, id: String) -> Result<(), String> {
    registry.remove_client_installation(&id)
}

/// 设置默认启动客户端。
#[tauri::command]
pub fn set_default_client(registry: RegistryState<'_>, id: String) -> Result<(), String> {
    registry.set_default_client(&id)
}

/// 读取所有已保存客户端安装记录。
#[tauri::command]
pub fn list_client_installations(
    registry: RegistryState<'_>,
) -> Result<Vec<ClientInstallation>, String> {
    registry.list_client_installations()
}

/// 读取默认启动客户端。
#[tauri::command]
pub fn get_default_client(
    registry: RegistryState<'_>,
) -> Result<Option<ClientInstallation>, String> {
    registry.get_default_client()
}

/// 读取 MVP 应用设置。
#[tauri::command]
pub fn load_app_settings(registry: RegistryState<'_>) -> Result<AppSettings, String> {
    registry.load_app_settings()
}

/// 保存 MVP 应用设置，并立即成为后续后端命令使用的配置。
#[tauri::command]
pub fn save_app_settings(
    registry: RegistryState<'_>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    registry.save_app_settings(&settings)?;
    #[cfg(target_os = "windows")]
    {
        set_autostart_registry(settings.autostart)?;
    }
    Ok(settings)
}

#[cfg(target_os = "windows")]
fn set_autostart_registry(enabled: bool) -> Result<(), String> {
    let exe_path =
        std::env::current_exe().map_err(|e| format!("Failed to get current exe path: {}", e))?;
    let exe_str = exe_path.to_string_lossy().to_string();

    let status = if enabled {
        std::process::Command::new("reg")
            .args([
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "DDNetManager",
                "/t",
                "REG_SZ",
                "/d",
                &exe_str,
                "/f",
            ])
            .status()
    } else {
        std::process::Command::new("reg")
            .args([
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "DDNetManager",
                "/f",
            ])
            .status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err("reg command returned non-zero status".to_string()),
        Err(e) => Err(format!("Failed to run reg command: {}", e)),
    }
}

/// 在 debug + 显式 env 开关下，把本地 smoke 自动验收结果写回脚本约定路径。
#[tauri::command]
pub fn report_local_smoke_result(result: LocalSmokeResultReport) -> Result<(), String> {
    crate::commands::download::write_local_smoke_result_report(&result)
}

/// 读取指定客户端的安装历史。
#[tauri::command]
pub fn list_install_history(
    registry: RegistryState<'_>,
    client_installation_id: String,
) -> Result<Vec<InstallHistoryRecord>, String> {
    registry.list_install_history(&client_installation_id)
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
) -> Result<crate::models::UpdateManifest, IpcError> {
    crate::manifest::fetch_manifest_with_route(&url, network_route.as_ref())
        .await
        .map_err(IpcError::from)
}

/// 检查指定客户端和渠道是否存在可用更新。
#[tauri::command]
pub async fn check_client_update(
    registry: RegistryState<'_>,
    request: CheckClientUpdateRequest,
) -> Result<Option<ClientUpdateCheck>, IpcError> {
    if crate::commands::download::request_requires_manifest_url(&request) {
        crate::commands::download::required_manifest_url(request.manifest_url.as_deref())?;
    }
    let current_version = registry
        .list_client_installations()?
        .into_iter()
        .find(|client| {
            crate::client_catalog::normalize_client_id(&client.client_id)
                == crate::client_catalog::normalize_client_id(&request.client_id)
                && client.is_default
        })
        .and_then(|client| client.version);

    crate::update_source::check_client_update(&request, current_version)
        .await
        .map_err(IpcError::from)
}

/// 启动指定路径的客户端可执行文件。
#[tauri::command]
pub fn launch_client(app: AppHandle, path: String) -> Result<(), String> {
    crate::process::launch_executable(&path)?;
    monitor_client_exit(app, path);
    Ok(())
}

/// 重新验证并启动默认客户端。
#[tauri::command]
pub fn launch_default_client(app: AppHandle, registry: RegistryState<'_>) -> Result<(), String> {
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

    monitor_client_exit(app, verified.executable_path.clone());
    Ok(())
}

/// 客户端启动后等待首次出现的最大轮询次数。
const MONITOR_STARTUP_MAX_POLLS: usize = 60;
/// 客户端启动后等待首次出现的轮询间隔（毫秒）。
const MONITOR_STARTUP_POLL_INTERVAL_MS: u64 = 500;
/// 客户端启动后等待进程文件就绪的初始延迟（秒）。
const MONITOR_STARTUP_INITIAL_DELAY_SECS: u64 = 2;
/// 客户端运行中轮询间隔（秒）。
const MONITOR_RUNNING_POLL_INTERVAL_SECS: u64 = 1;

fn monitor_client_exit(app: AppHandle, executable_path: String) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(MONITOR_STARTUP_INITIAL_DELAY_SECS)).await;

        let path = std::path::PathBuf::from(&executable_path);

        let mut was_running = false;
        for _ in 0..MONITOR_STARTUP_MAX_POLLS {
            if let Ok(true) = crate::process::is_client_running(&path) {
                was_running = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(MONITOR_STARTUP_POLL_INTERVAL_MS)).await;
        }

        if !was_running {
            return;
        }

        loop {
            tokio::time::sleep(Duration::from_secs(MONITOR_RUNNING_POLL_INTERVAL_SECS)).await;
            match crate::process::is_client_running(&path) {
                Ok(false) => break,
                Err(_) => break,
                _ => {}
            }
        }

        let registry = app.state::<ClientRegistry>();
        if let Ok(settings) = registry.load_app_settings() {
            if settings.exit_game_show_launcher {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
        }
    });
}

/// 表示软件自身更新检查的返回结构。
#[derive(Debug, Clone, serde::Serialize)]
pub struct AppUpdateCheck {
    /// 当前版本。
    pub current_version: String,
    /// 最新版本。
    pub latest_version: String,
    /// 是否需要更新。
    pub has_update: bool,
    /// 更新发布页面 URL。
    pub release_url: String,
    /// 更新说明文本。
    pub release_notes: Option<String>,
}

/// 检查 DDNet Manager 自身是否存在可用更新。
#[tauri::command]
pub async fn check_app_update(
    registry: RegistryState<'_>,
    app: AppHandle,
) -> Result<AppUpdateCheck, String> {
    let current_version = app.package_info().version.to_string();
    let settings = registry.load_app_settings()?;

    let release = crate::github_release::fetch_latest_github_release(
        "wuqie-xuanzhao",
        "DDNet-Manager",
        settings.network_route.as_ref(),
    )
    .await?;

    let latest_version = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    let has_update = crate::version::is_update_needed(Some(&current_version), &latest_version);

    Ok(AppUpdateCheck {
        current_version,
        latest_version,
        has_update,
        release_url: release.html_url,
        release_notes: release.body,
    })
}

/// 获取当前应用的版本号。
#[tauri::command]
pub fn get_app_version(app: AppHandle) -> Result<String, String> {
    Ok(app.package_info().version.to_string())
}

/// 返回 Tauri 应用缓存目录。
pub(crate) fn app_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map_err(|error| format!("failed to resolve app cache dir: {error}"))
}
