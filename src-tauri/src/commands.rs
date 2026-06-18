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

/// `scan_clients_via_mft` 默认最多收集多少条候选。NTFS 全盘扫很容易超过这个数
/// （多版本/多客户端机器），命中后扫描提前停止；如需放宽，把它做成 settings 字段。
const DEFAULT_SCAN_MAX_RESULTS: usize = 50;

/// `scan_clients_via_mft` 软超时（秒）。普通用户无 Mft/Usn 权限时整盘 Walkdir
/// 较慢，ntfs-search 默认 60s 对 C 盘不够；放宽到 180s 给业务足够时间。
const DEFAULT_SCAN_TIMEOUT_SECS: u64 = 180;

/// 扫描取消 token 的全局共享状态。
///
/// `scan_clients_via_mft` 开始时存入 master token，结束时清理；
/// `cancel_scan_clients` command 拿 token 调 cancel() 让正在跑的扫描尽快返回。
#[derive(Default)]
pub struct ScanCancelState(pub std::sync::Mutex<Option<tokio_util::sync::CancellationToken>>);

impl ScanCancelState {
    fn set(&self, token: tokio_util::sync::CancellationToken) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = Some(token);
        }
    }

    fn clear(&self) {
        if let Ok(mut guard) = self.0.lock() {
            *guard = None;
        }
    }

    /// 触发当前扫描的取消。返回 false 表示当前没有扫描在跑。
    fn cancel(&self) -> bool {
        if let Ok(mut guard) = self.0.lock() {
            if let Some(token) = guard.take() {
                token.cancel();
                return true;
            }
        }
        false
    }
}

/// 取消正在进行的 scan_clients_via_mft 扫描。返回是否成功取消。
#[tauri::command]
pub fn cancel_scan_clients(
    state: tauri::State<'_, ScanCancelState>,
) -> Result<bool, IpcError> {
    Ok(state.cancel())
}

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
    cancel_state: tauri::State<'_, ScanCancelState>,
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

    let max_results = settings.scan_max_results.unwrap_or(DEFAULT_SCAN_MAX_RESULTS);
    let total_timeout = settings.scan_timeout_secs.unwrap_or(DEFAULT_SCAN_TIMEOUT_SECS);

    // master cancel token：priority 和 fallback 阶段共享；存到全局 state 让
    // cancel_scan_clients command 能从外部触发取消。
    let master_cancel = tokio_util::sync::CancellationToken::new();
    cancel_state.set(master_cancel.clone());

    // collect_priority_roots 含 30-50 次同步 is_dir，移到 spawn_blocking 不阻塞 executor
    let priority_roots = if options.roots.is_empty() {
        tokio::task::spawn_blocking(collect_priority_roots)
            .await
            .map_err(|e| {
                crate::error::ManagerError::Internal(format!("priority_roots join: {e}"))
            })?
    } else {
        Vec::new()
    };

    let result = async {
        let mut all_installations: Vec<ClientInstallation> = Vec::new();
        let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

        // 两阶段扫描：priority 先扫常见安装位置，命中数 < max_results 时继续 fallback 找全
        if !priority_roots.is_empty() {
            let priority_installations = run_scan(
                priority_roots,
                &excluded,
                max_results,
                total_timeout / 3, // priority 阶段给 1/3 时间预算
                options.include_unhealthy,
                &registry,
                app.clone(),
                master_cancel.clone(),
            )
            .await?;
            for inst in priority_installations {
                if seen_ids.insert(inst.id.clone()) {
                    all_installations.push(inst);
                }
            }
            // priority 已找到 max_results 个，提前返回；否则继续 fallback
            if all_installations.len() >= max_results {
                return Ok(all_installations);
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

        let fallback_installations = run_scan(
            roots,
            &excluded,
            max_results,
            total_timeout,
            options.include_unhealthy,
            &registry,
            app,
            master_cancel.clone(),
        )
        .await?;
        for inst in fallback_installations {
            if seen_ids.insert(inst.id.clone()) {
                all_installations.push(inst);
            }
        }

        all_installations.sort_by(|a, b| a.install_dir.cmp(&b.install_dir));
        Ok(all_installations)
    }
    .await;

    cancel_state.clear();
    result
}

/// 单次扫描的封装：构建 opts + 调 find_files + 转 ClientInstallation。
async fn run_scan(
    roots: Vec<PathBuf>,
    excluded: &[PathBuf],
    max_results: usize,
    timeout_secs: u64,
    include_unhealthy: bool,
    registry: &crate::registry::ClientRegistry,
    app: AppHandle,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<Vec<ClientInstallation>, IpcError> {
    let opts = ntfs_search::NtfsScanOptions::new(|name| {
        ["DDNet.exe", "ddnet.exe"]
            .iter()
            .any(|expected| name.eq_ignore_ascii_case(expected))
    })
    .with_roots(roots)
    .with_max_results(max_results)
    .with_timeout(std::time::Duration::from_secs(timeout_secs));

    let progress = std::sync::Arc::new(TauriScanSink::new(app));

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
        let mut installation = crate::client_scan::validate_client_dir(parent)?;
        // 默认过滤残缺客户端（缺 data 目录、storage.cfg 等）。这类候选通常是
        // QQ 文件夹里的 ddnet.exe 单文件、开发仓库里的 build 产物等噪音。
        // 前端"显示残缺客户端"开关透传到 include_unhealthy。
        if !include_unhealthy && installation.health != crate::models::ClientHealth::Ok {
            continue;
        }
        // 用 registry 指纹库升级识别：exe_sha256 命中用户下载记录时覆盖路径/PE
        // 匹配结果，置信度升到 Verified（不可伪造）。exe_sha256 为 None 时跳过。
        upgrade_identity_with_registry_fingerprint(&mut installation, registry)?;
        if seen_ids.insert(installation.id.clone()) {
            installations.push(installation);
        }
    }

    installations.sort_by(|a, b| a.install_dir.cmp(&b.install_dir));
    Ok(installations)
}

/// 用 registry 中的用户下载指纹升级识别结果。命中时覆盖 client_id/display_name/
/// version/upstream_url/confidence。失败不阻断扫描（指纹缺失只让识别降级）。
fn upgrade_identity_with_registry_fingerprint(
    client: &mut ClientInstallation,
    registry: &crate::registry::ClientRegistry,
) -> Result<(), IpcError> {
    let Some(hash) = client.exe_sha256.as_ref() else {
        return Ok(());
    };
    let fp = registry
        .lookup_fingerprint_by_hash(hash)
        .map_err(IpcError::from)?;
    if let Some(fp) = fp {
        client.client_id = fp.client_id;
        client.display_name = fp.display_name;
        if let Some(version) = fp.version {
            client.version = Some(version);
        }
        client.confidence = crate::models::ClientConfidence::Verified;
        if let Some(entry) = crate::client_catalog::catalog_entry_by_id(&client.client_id) {
            client.upstream_url = entry.upstream_url.map(str::to_string);
        }
    }
    Ok(())
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
#[tauri::command]
pub fn probe_disk(path: String) -> Result<DiskProbe, IpcError> {
    use sysinfo::Disks;
    let target = std::path::Path::new(&path);
    let disks = Disks::new_with_refreshed_list();

    // 找最匹配的磁盘：target 必须 starts_with mount_point，取 components 数最多的（最具体）
    let best = disks
        .list()
        .iter()
        .filter(|d| target.starts_with(d.mount_point()))
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
        if request.desktop {
            create_windows_shortcut(
                &request.executable_path,
                &request.working_dir,
                display_name,
                windows_desktop_dir()?,
            )?;
        }
        if request.start_menu {
            create_windows_shortcut(
                &request.executable_path,
                &request.working_dir,
                display_name,
                windows_start_menu_dir()?,
            )?;
        }
        return Ok(());
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
    Ok(appdata.join("Microsoft").join("Windows").join("Start Menu").join("Programs"))
}

#[cfg(target_os = "windows")]
fn create_windows_shortcut(
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
    let lnk_path = target_dir.join(format!("{name}.lnk"));
    // 用 PowerShell 调 WScript.Shell COM 创建 .lnk。
    // 单引号转义：路径里可能含空格，PowerShell 字符串用双引号包裹。
    let script = format!(
        "$ws = New-Object -ComObject WScript.Shell; \
         $s = $ws.CreateShortcut(\"{}\"); \
         $s.TargetPath = \"{}\"; \
         $s.WorkingDirectory = \"{}\"; \
         $s.Save()",
        lnk_path.to_string_lossy().replace('"', "`\""),
        exe.replace('"', "`\""),
        workdir.replace('"', "`\"")
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| {
            IpcError::from(crate::error::ManagerError::Internal(format!(
                "powershell launch failed: {e}"
            )))
        })?;
    if !output.status.success() {
        return Err(IpcError::from(crate::error::ManagerError::Internal(format!(
            "powershell exit {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))));
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
pub(crate) fn app_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_cache_dir()
        .map_err(|error| format!("failed to resolve app cache dir: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_cancel_state_cancel_returns_false_when_no_active_scan() {
        let state = ScanCancelState::default();
        assert!(!state.cancel(), "无扫描在跑时 cancel 应返回 false");
    }

    #[test]
    fn scan_cancel_state_set_then_cancel_roundtrip() {
        let state = ScanCancelState::default();
        let token = tokio_util::sync::CancellationToken::new();
        state.set(token.clone());

        assert!(!token.is_cancelled(), "set 后 token 不应已取消");
        assert!(state.cancel(), "有扫描时 cancel 应返回 true");
        assert!(token.is_cancelled(), "cancel 后 token 应被触发");
        assert!(!state.cancel(), "二次 cancel 应返回 false（已 take）");
    }

    #[test]
    fn scan_cancel_state_clear_drops_active_token() {
        let state = ScanCancelState::default();
        let token = tokio_util::sync::CancellationToken::new();
        state.set(token);
        state.clear();
        assert!(!state.cancel(), "clear 后 cancel 应返回 false");
    }

    /// collect_priority_roots 在 Windows 上应包含 Program Files + 用户目录 + Steam。
    /// 在 Linux/macOS 上所有 Windows 路径都不存在，返回空 Vec（不 panic）。
    #[test]
    fn collect_priority_roots_does_not_panic() {
        let roots = collect_priority_roots();
        #[cfg(windows)]
        {
            // Windows 应至少找到一些 priority 路径（除非真的全没装）
            // 这里只断言函数能跑通不 panic，不强制非空（CI 环境可能 stripped）
            eprintln!("priority roots found: {} entries", roots.len());
        }
        #[cfg(not(windows))]
        {
            // Linux/macOS 上 Windows 路径都不存在
            assert!(roots.is_empty(), "Unix 上 collect_priority_roots 应返回空");
        }
    }

    /// Windows 上 Program Files 几乎一定存在，验证 priority 收集包含它。
    #[cfg(windows)]
    #[test]
    fn collect_priority_roots_includes_program_files_when_present() {
        let roots = collect_priority_roots();
        let pf = std::env::var_os("ProgramFiles").map(PathBuf::from);
        if let Some(pf) = pf {
            if pf.is_dir() {
                assert!(
                    roots.iter().any(|r| r == &pf),
                    "priority roots 应包含 Program Files: {}",
                    pf.display()
                );
            }
        }
    }

    // run_scan / scan_clients_via_mft 的集成测试需要 AppHandle + Tauri runtime，
    // 单元测试难以构造。已通过 vitest + invoke mock 在 useClientScanner.test.tsx
    // 中覆盖端到端流程（包括 cancel 链路）。
}
