//! IPC 命令聚合层：声明各命令子模块并注册剩余未分组命令。
//!
//! lint-WARN(C1): 文件约 694 行 > 600，存量；按项目约定 C1 仅在 > 800 行时强制
//! 拆分，当前规模未触发阈值。部分 command 仍留在此处未拆分到子模块，后续按
//! scan/download 模式继续分组即可收敛。

/// 下载与安装子命令。
pub mod download;

/// 安装事务子命令。
pub mod install;

/// 客户端扫描子命令（scan_clients_via_mft / cancel_scan_clients / ScanCancelState）。
pub mod scan;

use crate::error::{IpcError, ManagerError};
use crate::models::{
    AppSettings, CheckClientUpdateRequest, ClientHealth, ClientInstallation, ClientUpdateCheck,
    DownloadJob, InstallHistoryRecord, InstallHistoryStatus, LocalSmokeResultReport,
    NetworkRouteConfig, UpsertClientInstallationRequest,
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
    /// 已 resolve 的反代前缀列表（空配置已 fallback 到默认列表）。
    pub(crate) mirror_prefixes: Vec<String>,
    /// 用户显式信任的额外下载 host（公共反代域名）。
    pub(crate) extra_hosts: Vec<String>,
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

/// 内部命令：接收 webview 端 init_script 拦截的 console / error 输出，
/// 通过 stderr 打到 `tauri dev` 终端。仅用于 webview 渲染调试。
#[tauri::command]
pub fn __webview_console(level: String, msg: String) {
    let tag = match level.as_str() {
        "error" => "❌ [webview]",
        "warn" => "⚠️  [webview]",
        _ => "   [webview]",
    };
    eprintln!("{tag} {msg}");
}

/// 验证用户选择的客户端目录，并返回识别出的安装信息。
#[tauri::command]
pub fn validate_client_dir(path: String) -> Result<crate::models::ClientInstallation, IpcError> {
    crate::client_scan::validate_client_dir(Path::new(&path)).map_err(IpcError::from)
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
fn set_autostart_registry(enabled: bool) -> Result<(), ManagerError> {
    let exe_path = std::env::current_exe()
        .map_err(|e| ManagerError::Internal(format!("Failed to get current exe path: {e}")))?;
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
        Ok(_) => Err(ManagerError::Internal(
            "reg command returned non-zero status".to_string(),
        )),
        Err(e) => Err(ManagerError::Internal(format!(
            "Failed to run reg command: {e}"
        ))),
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
///
/// 网络失败时若本地有缓存则回退到缓存版本，避免纯离线环境下 manifest 路径完全不可用。
#[tauri::command]
pub async fn load_manifest(
    app: AppHandle,
    url: String,
    network_route: Option<NetworkRouteConfig>,
) -> Result<crate::models::UpdateManifest, IpcError> {
    let cache_dir = app_cache_dir(&app).ok();
    crate::manifest::fetch_manifest_with_route(&url, network_route.as_ref(), cache_dir.as_deref())
        .await
        .map_err(IpcError::from)
}

/// 检查指定客户端和渠道是否存在可用更新。
///
/// 始终返回 `ClientUpdateCheck`（非 Option）：当 `action == None` 时通过 `reason`
/// 字段区分"已是最新版"与"无法判断/不支持自动更新"等语义，避免前端把后者误判为前者。
#[tauri::command]
pub async fn check_client_update(
    app: AppHandle,
    registry: RegistryState<'_>,
    request: CheckClientUpdateRequest,
) -> Result<ClientUpdateCheck, IpcError> {
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

    let cache_dir = app.path().cache_dir().ok();
    crate::update_source::check_client_update(&request, current_version, cache_dir.as_deref())
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

/// 返回内置客户端 catalog（6 个客户端定义），供前端动态生成 game tab。
///
/// catalog 是编译期静态数据，不读磁盘，无 IPC 状态依赖，可任意调用。
/// 前端 GAMES_DATA 在启动时调用一次，缓存到 React Context。
#[tauri::command]
pub fn get_client_catalog() -> Vec<crate::client_catalog::ClientCatalogEntry> {
    crate::client_catalog::catalog_entries().to_vec()
}

/// 磁盘探测结果：剩余空间、总空间、是否 SSD。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiskProbe {
    /// 剩余可用字节数。
    pub free_bytes: u64,
    /// 磁盘总字节数。
    pub total_bytes: u64,
    /// 是否 SSD。`None` 表示平台不支持判断或 sysinfo 未识别（NAS / 网络盘等）。
    pub is_ssd: Option<bool>,
    /// 匹配到的磁盘挂载点（前端展示用）。
    pub mount_point: String,
}

/// 探测给定路径所在磁盘的剩余空间、总空间、是否 SSD。
///
/// 用 sysinfo crate 跨平台（Windows / Linux / macOS），按 mount_point 前缀匹配
/// 找最具体的磁盘。未匹配返回 NotFound。
///
/// 边界处理（review issue #7）：
/// - 多 mount_point 同前缀时取 components 数最多的（最具体）
/// - 如果 path 能 canonicalize（已存在），用 canonical 后的路径再匹配一次，
///   避免符号链接 / 大小写差异导致的误匹配
/// - 不能 canonicalize（路径还不存在，安装弹窗常见）时退回原始 path 匹配
#[tauri::command]
pub fn probe_disk(path: String) -> Result<DiskProbe, IpcError> {
    use sysinfo::Disks;
    let raw_target = std::path::Path::new(&path);
    // 优先用 canonicalize 后的绝对路径匹配；不存在则用原始
    let canonical = raw_target
        .canonicalize()
        .unwrap_or_else(|_| raw_target.to_path_buf());
    let disks = Disks::new_with_refreshed_list();

    let best = disks
        .list()
        .iter()
        .filter(|d| canonical.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().components().count());

    let disk = best.ok_or_else(|| {
        IpcError::from(crate::error::ManagerError::NotFound(format!(
            "no disk matches path: {}",
            path
        )))
    })?;

    let is_ssd = match disk.kind() {
        sysinfo::DiskKind::HDD => Some(false),
        sysinfo::DiskKind::SSD => Some(true),
        sysinfo::DiskKind::Unknown(_) => None,
    };

    Ok(DiskProbe {
        free_bytes: disk.available_space(),
        total_bytes: disk.total_space(),
        is_ssd,
        mount_point: disk.mount_point().to_string_lossy().to_string(),
    })
}

/// 快捷方式创建请求：目标 exe 路径 + 工作目录 + 显示名 + 是否创建桌面/开始菜单。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateShortcutsRequest {
    /// 目标可执行文件路径（如客户端 DDNet.exe）。
    pub executable_path: String,
    /// 工作目录（通常是客户端 install_dir）。
    pub working_dir: String,
    /// 快捷方式显示名（如 "QmClient"）。
    pub display_name: String,
    /// 是否创建桌面快捷方式。
    pub desktop: bool,
    /// 是否创建开始菜单 / 应用列表快捷方式。
    pub start_menu: bool,
}

/// 创建桌面和开始菜单快捷方式。
///
/// - Windows：用 PowerShell 调 WScript.Shell COM 创建 .lnk
/// - Linux：写 .desktop 文件到 ~/.local/share/applications 和 ~/Desktop
/// - macOS：跳过（dock 已是事实标准），返回 Ok 但不创建
#[tauri::command]
pub fn create_shortcuts(request: CreateShortcutsRequest) -> Result<(), IpcError> {
    let display_name = request.display_name.trim();
    if display_name.is_empty() {
        return Err(IpcError::from(crate::error::ManagerError::Internal(
            "display_name must not be empty".to_string(),
        )));
    }

    #[cfg(target_os = "windows")]
    {
        // review issue #8：合并 desktop + start_menu 为单次 PowerShell 调用，
        // 两个 CreateShortcut 写在同一个脚本里，省一次 powershell.exe spawn（~150ms）。
        let mut lnk_targets: Vec<std::path::PathBuf> = Vec::new();
        if request.desktop {
            lnk_targets.push(windows_desktop_dir()?);
        }
        if request.start_menu {
            lnk_targets.push(windows_start_menu_dir()?);
        }
        if lnk_targets.is_empty() {
            return Ok(());
        }
        create_windows_shortcuts_batch(
            &request.executable_path,
            &request.working_dir,
            display_name,
            &lnk_targets,
        )?;
        Ok(())
    }

    #[cfg(target_os = "linux")]
    {
        if request.desktop {
            create_linux_desktop_file(
                &request.executable_path,
                &request.working_dir,
                display_name,
                linux_desktop_dir()?,
            )?;
        }
        if request.start_menu {
            create_linux_desktop_file(
                &request.executable_path,
                &request.working_dir,
                display_name,
                linux_applications_dir()?,
            )?;
        }
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        // macOS dock 是事实标准，跳过快捷方式创建
        let _ = request;
        Ok(())
    }

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        let _ = request;
        Err(IpcError::from(crate::error::ManagerError::Internal(
            "create_shortcuts: unsupported platform".to_string(),
        )))
    }
}

#[cfg(target_os = "windows")]
fn windows_desktop_dir() -> Result<std::path::PathBuf, IpcError> {
    dirs::desktop_dir().ok_or_else(|| {
        IpcError::from(crate::error::ManagerError::Internal(
            "cannot resolve Windows desktop dir".to_string(),
        ))
    })
}

#[cfg(target_os = "windows")]
fn windows_start_menu_dir() -> Result<std::path::PathBuf, IpcError> {
    // %APPDATA%\Microsoft\Windows\Start Menu\Programs
    let appdata = dirs::config_dir().ok_or_else(|| {
        IpcError::from(crate::error::ManagerError::Internal(
            "cannot resolve Windows config dir for start menu".to_string(),
        ))
    })?;
    Ok(appdata
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs"))
}

#[cfg(target_os = "windows")]
fn create_windows_shortcuts_batch(
    exe: &str,
    workdir: &str,
    name: &str,
    target_dirs: &[std::path::PathBuf],
) -> Result<(), IpcError> {
    // 拼接 PowerShell 脚本：为每个 target_dir 创建一个 .lnk
    let mut script = String::from("$ws = New-Object -ComObject WScript.Shell; ");
    for dir in target_dirs {
        std::fs::create_dir_all(dir).map_err(|e| {
            IpcError::from(crate::error::ManagerError::Internal(format!(
                "create target dir failed: {e}"
            )))
        })?;
        let lnk_path = dir.join(format!("{name}.lnk"));
        script.push_str(&format!(
            "$s = $ws.CreateShortcut(\"{}\"); $s.TargetPath = \"{}\"; $s.WorkingDirectory = \"{}\"; $s.Save(); ",
            lnk_path.to_string_lossy().replace('"', "`\""),
            exe.replace('"', "`\""),
            workdir.replace('"', "`\"")
        ));
    }
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| {
            IpcError::from(crate::error::ManagerError::Internal(format!(
                "powershell launch failed: {e}"
            )))
        })?;
    if !output.status.success() {
        return Err(IpcError::from(crate::error::ManagerError::Internal(
            format!(
                "powershell exit {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ),
        )));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn linux_desktop_dir() -> Result<std::path::PathBuf, IpcError> {
    let home = dirs::home_dir().ok_or_else(|| {
        IpcError::from(crate::error::ManagerError::Internal(
            "cannot resolve home dir".to_string(),
        ))
    })?;
    Ok(home.join("Desktop"))
}

#[cfg(target_os = "linux")]
fn linux_applications_dir() -> Result<std::path::PathBuf, IpcError> {
    let home = dirs::home_dir().ok_or_else(|| {
        IpcError::from(crate::error::ManagerError::Internal(
            "cannot resolve home dir".to_string(),
        ))
    })?;
    Ok(home.join(".local").join("share").join("applications"))
}

#[cfg(target_os = "linux")]
fn create_linux_desktop_file(
    exe: &str,
    workdir: &str,
    name: &str,
    target_dir: std::path::PathBuf,
) -> Result<(), IpcError> {
    std::fs::create_dir_all(&target_dir).map_err(|e| {
        IpcError::from(crate::error::ManagerError::Internal(format!(
            "create target dir failed: {e}"
        )))
    })?;
    let file_name = name
        .to_ascii_lowercase()
        .replace(|c: char| !c.is_alphanumeric(), "-");
    let path = target_dir.join(format!("{file_name}.desktop"));
    let content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name={name}\n\
         Exec={exe}\n\
         Path={workdir}\n\
         Terminal=false\n\
         Categories=Game;\n"
    );
    std::fs::write(&path, content).map_err(|e| {
        IpcError::from(crate::error::ManagerError::Internal(format!(
            "write .desktop failed: {e}"
        )))
    })?;
    // 设置可执行权限（Linux 桌面环境要求 .desktop 文件可执行才会出现在应用列表）
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
    }
    Ok(())
}

/// 返回 Tauri 应用缓存目录。
pub(crate) fn app_cache_dir(app: &AppHandle) -> Result<PathBuf, ManagerError> {
    app.path().app_cache_dir().map_err(|error| {
        ManagerError::Internal(format!("failed to resolve app cache dir: {error}"))
    })
}
