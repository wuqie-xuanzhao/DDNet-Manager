/// 下载与安装子命令。
pub mod download;

/// 安装事务子命令。
pub mod install;

use crate::error::IpcError;
use crate::models::{
    AppSettings, CheckClientUpdateRequest, ClientHealth, ClientInstallation, ClientUpdateCheck,
    DownloadJob, InstallHistoryRecord, InstallHistoryStatus, LocalSmokeResultReport,
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
pub fn validate_client_dir(path: String) -> Result<crate::models::ClientInstallation, IpcError> {
    crate::client_scan::validate_client_dir(Path::new(&path)).map_err(IpcError::from)
}

/// `scan_clients_via_mft` 默认最多收集多少条候选。NTFS 全盘扫很容易超过这个数
/// （多版本/多客户端机器），命中后扫描提前停止；如需放宽，把它做成 settings 字段。
const DEFAULT_SCAN_MAX_RESULTS: usize = 50;

/// `scan_clients_via_mft` 软超时（秒）。普通用户无 Mft/Usn 权限时整盘 Walkdir
/// 较慢，ntfs-search 默认 60s 对 C 盘不够；放宽到 180s 给业务足够时间。
const DEFAULT_SCAN_TIMEOUT_SECS: u64 = 180;

/// 使用 ntfs-search crate 全量扫盘找 DDNet.exe 兼容客户端。
///
/// 后端自动按平台/权限选 Mft / Usn / Walkdir（admin > 普通 > fallback），失败自动降级。
/// 扫描期间实时 emit `scan-progress` 事件（[`ntfs_search::ProgressEvent`]），
/// 前端按 `kind` discriminated union 渲染进度。
///
/// **两阶段扫描**：用户未显式指定 roots 时，先扫 priority（Steam / Program Files /
/// 用户目录），命中秒级返回；未命中再 fallback 全盘。这样典型用户场景（Steam
/// 安装的 DDNet）只需扫少数子树，避免大盘（HDD 几 T）长时间扫描。
#[tauri::command]
pub async fn scan_clients_via_mft(
    registry: RegistryState<'_>,
    options: Option<ScanClientInstallationsOptions>,
    app: AppHandle,
) -> Result<Vec<ClientInstallation>, IpcError> {
    let options = options.unwrap_or_default();
    let settings = registry.load_app_settings()?;

    let excluded: Vec<PathBuf> = settings
        .scan_excluded_paths
        .iter()
        .map(PathBuf::from)
        .collect();

    // 两阶段扫描：用户未指定 roots 时先扫 priority 命中即返回
    if options.roots.is_empty() {
        let priority_roots = collect_priority_roots();
        if !priority_roots.is_empty() {
            let installations = run_scan(
                priority_roots,
                &excluded,
                DEFAULT_SCAN_TIMEOUT_SECS / 3, // priority 阶段给 1/3 时间预算
                app.clone(),
            )
            .await?;
            if !installations.is_empty() {
                return Ok(installations);
            }
        }
    }

    // Fallback / 用户显式指定 roots：全盘扫描
    let mut roots: Vec<PathBuf> = if options.roots.is_empty() {
        collect_default_drive_roots()
    } else {
        options.roots.iter().map(PathBuf::from).collect()
    };

    if options.include_saved_paths {
        roots.extend(
            registry
                .list_client_installations()?
                .into_iter()
                .filter_map(|c| PathBuf::from(c.install_dir).parent().map(Path::to_path_buf)),
        );
    }

    let installations = run_scan(roots, &excluded, DEFAULT_SCAN_TIMEOUT_SECS, app).await?;
    Ok(installations)
}

/// 单次扫描的封装：构建 opts + 调 find_files + 转 ClientInstallation。
async fn run_scan(
    roots: Vec<PathBuf>,
    excluded: &[PathBuf],
    timeout_secs: u64,
    app: AppHandle,
) -> Result<Vec<ClientInstallation>, IpcError> {
    let opts = ntfs_search::NtfsScanOptions::new(|name| {
        ["DDNet.exe", "ddnet.exe"]
            .iter()
            .any(|expected| name.eq_ignore_ascii_case(expected))
    })
    .with_roots(roots)
    .with_max_results(DEFAULT_SCAN_MAX_RESULTS)
    .with_timeout(std::time::Duration::from_secs(timeout_secs));

    let progress = std::sync::Arc::new(TauriScanSink::new(app));
    let cancel = tokio_util::sync::CancellationToken::new();

    let entries = ntfs_search::find_files(opts, progress, cancel)
        .await
        .map_err(crate::error::ManagerError::from)?;

    let mut installations: Vec<ClientInstallation> = Vec::new();
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for entry in entries {
        let Some(parent) = entry.path.parent() else {
            continue;
        };
        if crate::client_scan::is_local_smoke_tmp_path(parent) {
            continue;
        }
        if excluded
            .iter()
            .any(|ex| crate::client_scan::normalize_for_compare(ex) == crate::client_scan::normalize_for_compare(parent))
        {
            continue;
        }
        let installation = crate::client_scan::validate_client_dir(parent)?;
        if seen_ids.insert(installation.id.clone()) {
            installations.push(installation);
        }
    }

    installations.sort_by(|a, b| a.install_dir.cmp(&b.install_dir));
    Ok(installations)
}

/// Priority roots：DDNet 客户端最可能安装的位置。典型用户场景命中即返回，
/// 避免大盘（HDD 几 T）长时间扫描。
///
/// 包含：
/// - Steam library（默认 Program Files + 各盘符根下 \Steam）
/// - Program Files / Program Files (x86)
/// - 用户目录（Downloads / Desktop / Documents / Games）
/// - LOCALAPPDATA（部分客户端装这里）
/// - 各盘 \Games 子目录（玩家常用）
fn collect_priority_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    let push = |roots: &mut Vec<PathBuf>, p: PathBuf| {
        if p.is_dir() && !roots.contains(&p) {
            roots.push(p);
        }
    };

    // Steam libraries：默认安装位置 + 各盘符根下 \Steam
    push(
        &mut roots,
        PathBuf::from(r"C:\Program Files (x86)\Steam").join("steamapps").join("common"),
    );
    push(
        &mut roots,
        PathBuf::from(r"C:\Program Files\Steam").join("steamapps").join("common"),
    );
    for letter in b'C'..=b'Z' {
        push(
            &mut roots,
            PathBuf::from(format!("{}:\\Steam", letter as char))
                .join("steamapps")
                .join("common"),
        );
    }

    // Program Files
    for env in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(p) = std::env::var_os(env) {
            push(&mut roots, PathBuf::from(p));
        }
    }

    // User dirs
    if let Some(p) = std::env::var_os("USERPROFILE") {
        let user = PathBuf::from(p);
        for sub in ["Downloads", "Desktop", "Documents", "Games"] {
            push(&mut roots, user.join(sub));
        }
    }
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        push(&mut roots, PathBuf::from(p));
    }

    // 各盘 \Games 子目录（玩家常用）
    for letter in b'C'..=b'Z' {
        push(&mut roots, PathBuf::from(format!("{}:\\Games", letter as char)));
    }

    roots
}

/// 把 ntfs-search 的 ProgressEvent 转 Tauri event 推到前端。
struct TauriScanSink {
    app: AppHandle,
}

impl TauriScanSink {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl ntfs_search::ProgressSink for TauriScanSink {
    fn emit(&self, event: ntfs_search::ProgressEvent) {
        use tauri::Emitter;
        let _ = self.app.emit("scan-progress", &event);
    }
}

/// Windows 默认固定盘符 roots（C: 永远在，D-Z 按存在性添加）。
fn collect_default_drive_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("C:\\")];
    #[cfg(windows)]
    {
        for letter in b'D'..=b'Z' {
            let path = PathBuf::from(format!("{}:\\", letter as char));
            if path.exists() {
                roots.push(path);
            }
        }
    }
    roots
}

/// 保存或更新客户端安装记录。
#[tauri::command]
pub fn upsert_client_installation(
    registry: RegistryState<'_>,
    request: UpsertClientInstallationRequest,
) -> Result<ClientInstallation, IpcError> {
    let mut client = crate::client_scan::validate_client_dir(Path::new(&request.install_dir))
        .map_err(IpcError::from)?;
    if request.is_default
        && crate::client_scan::is_local_smoke_tmp_path(Path::new(&client.install_dir))
    {
        return Err(IpcError::from(
            "local smoke client cannot be saved as default".to_string(),
        ));
    }
    client.is_default = request.is_default;
    registry.upsert_client_installation(&client)?;
    Ok(client)
}

/// 从注册表移除客户端记录，不删除本地文件。
#[tauri::command]
pub fn remove_client_installation(registry: RegistryState<'_>, id: String) -> Result<(), IpcError> {
    registry.remove_client_installation(&id)?;
    Ok(())
}

/// 设置默认启动客户端。
#[tauri::command]
pub fn set_default_client(registry: RegistryState<'_>, id: String) -> Result<(), IpcError> {
    registry.set_default_client(&id)?;
    Ok(())
}

/// 读取所有已保存客户端安装记录。
#[tauri::command]
pub fn list_client_installations(
    registry: RegistryState<'_>,
) -> Result<Vec<ClientInstallation>, IpcError> {
    registry.list_client_installations().map_err(IpcError::from)
}

/// 读取默认启动客户端。
#[tauri::command]
pub fn get_default_client(
    registry: RegistryState<'_>,
) -> Result<Option<ClientInstallation>, IpcError> {
    registry.get_default_client().map_err(IpcError::from)
}

/// 读取 MVP 应用设置。
#[tauri::command]
pub fn load_app_settings(registry: RegistryState<'_>) -> Result<AppSettings, IpcError> {
    registry.load_app_settings().map_err(IpcError::from)
}

/// 保存 MVP 应用设置，并立即成为后续后端命令使用的配置。
#[tauri::command]
pub fn save_app_settings(
    registry: RegistryState<'_>,
    settings: AppSettings,
) -> Result<AppSettings, IpcError> {
    registry.save_app_settings(&settings)?;
    #[cfg(target_os = "windows")]
    {
        set_autostart_registry(settings.autostart).map_err(IpcError::from)?;
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
pub fn report_local_smoke_result(result: LocalSmokeResultReport) -> Result<(), IpcError> {
    crate::commands::download::write_local_smoke_result_report(&result).map_err(IpcError::from)
}

/// 读取指定客户端的安装历史。
#[tauri::command]
pub fn list_install_history(
    registry: RegistryState<'_>,
    client_installation_id: String,
) -> Result<Vec<InstallHistoryRecord>, IpcError> {
    registry
        .list_install_history(&client_installation_id)
        .map_err(IpcError::from)
}

/// 判断指定客户端可执行文件是否正在运行。
#[tauri::command]
pub fn is_client_running(path: String) -> Result<bool, IpcError> {
    crate::process::is_client_running(Path::new(&path)).map_err(IpcError::from)
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
pub fn launch_client(app: AppHandle, path: String) -> Result<(), IpcError> {
    crate::process::launch_executable(&path).map_err(IpcError::from)?;
    monitor_client_exit(app, path);
    Ok(())
}

/// 重新验证并启动默认客户端。
#[tauri::command]
pub fn launch_default_client(app: AppHandle, registry: RegistryState<'_>) -> Result<(), IpcError> {
    let client = registry
        .get_default_client()?
        .ok_or_else(|| IpcError::from("default client is not configured".to_string()))?;
    let verified = crate::client_scan::validate_client_dir(Path::new(&client.install_dir))
        .map_err(IpcError::from)?;
    if verified.health != ClientHealth::Ok {
        return Err(IpcError::from(format!(
            "default client is not healthy before launch: {:?}",
            verified.health
        )));
    }
    if !verified.compatibility.can_launch {
        return Err(IpcError::from(
            "default client is not compatible with this machine".to_string(),
        ));
    }

    let probe = crate::process::launch_executable_with_probe(
        &verified.executable_path,
        Duration::from_secs(2),
    )
    .map_err(IpcError::from)?;
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
) -> Result<AppUpdateCheck, IpcError> {
    let current_version = app.package_info().version.to_string();
    let settings = registry.load_app_settings()?;

    let release = crate::github_release::fetch_latest_github_release(
        "wuqie-xuanzhao",
        "DDNet-Manager",
        settings.network_route.as_ref(),
    )
    .await
    .map_err(IpcError::from)?;

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
pub fn get_app_version(app: AppHandle) -> Result<String, IpcError> {
    Ok(app.package_info().version.to_string())
}

/// 返回 Tauri 应用缓存目录。
pub(crate) fn app_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map_err(|error| format!("failed to resolve app cache dir: {error}"))
}
