#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 后端结构化错误类型与稳定 IPC 错误码。
pub mod error;

/// DDNet Manager 的领域模型定义。
pub mod models;

/// 客户端目录扫描与安装验证。
pub mod client_scan;

/// 内置 DDNet 兼容客户端目录。
pub mod client_catalog;

/// 客户端进程识别与启动。
pub mod process;

/// 本地 smoke 受控放行策略。
pub mod local_smoke;

/// 更新 manifest 拉取与解析。
pub mod manifest;

/// 按客户端 catalog 分派更新来源。
pub mod update_source;

/// GitHub Release 更新源适配器。
pub mod github_release;

/// DDNet 官方下载页与 sha256sums 适配器。
pub mod ddnet_source;

/// 下载校验与下载事务基础能力。
pub mod download;

/// 公共反代前缀拼装与候选 URL 组装（仅下载执行层使用）。
pub mod mirror;

/// 网络路由候选选择与探测结果聚合。
pub mod network_route;

/// 客户端注册表持久化能力。
pub mod registry;

/// 版本号比较与更新判断。
pub mod version;

/// 系统托盘图标与托盘菜单事件处理。
pub mod tray;

mod commands;

use tauri::Manager;

fn main() {
    let db_path = app_data_dir_for_main().join("ddnet-manager.sqlite");
    let registry = registry::ClientRegistry::open(&db_path).unwrap_or_else(|error| {
        eprintln!(
            "failed to open client registry at {}: {error}",
            db_path.display()
        );
        std::process::exit(1);
    });
    let manager = download::DownloadManager::default();
    if let Err(error) = manager.recover_from_registry(&registry) {
        eprintln!("failed to recover download jobs from registry: {error}");
    }
    let run_result = tauri::Builder::default()
        .manage(manager)
        .manage(registry)
        .manage(commands::scan::ScanCancelState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .on_window_event(handle_main_window_event)
        .setup(setup_app)
        .invoke_handler(tauri::generate_handler![
            commands::__webview_console,
            commands::validate_client_dir,
            commands::scan::scan_clients_via_mft,
            commands::scan::cancel_scan_clients,
            commands::upsert_client_installation,
            commands::remove_client_installation,
            commands::set_default_client,
            commands::list_client_installations,
            commands::get_default_client,
            commands::load_app_settings,
            commands::save_app_settings,
            commands::report_local_smoke_result,
            commands::list_install_history,
            commands::launch_client,
            commands::launch_default_client,
            commands::is_client_running,
            commands::load_manifest,
            commands::check_client_update,
            commands::download::start_update_download,
            commands::download::cancel_download,
            commands::download::get_download_job,
            commands::download::list_download_job_recoveries,
            commands::install::install_downloaded_update,
            commands::install::rollback_client_installation,
            commands::check_app_update,
            commands::get_app_version,
            commands::get_client_catalog,
            commands::probe_disk,
            commands::create_shortcuts
        ])
        .run(tauri::generate_context!());

    if let Err(error) = run_result {
        eprintln!("failed to run DDNet Manager Tauri application: {error}");
        std::process::exit(1);
    }
}

/// 主窗口事件处理：仅在用户主动关闭主窗口时退出 app 并清理托盘图标。
///
/// 用 `CloseRequested` 而非 `Destroyed`：`Destroyed` 在 transparent + decorations:false
/// 窗口上可能被 spawn 子进程（启动游戏）/ 焦点切换 / DWM 合成异常误触发，导致启动
/// 游戏后启动器意外退出。`CloseRequested` 只在用户主动点 X 或调 `close()` 时触发。
/// minimize / hide / unminimize / focus 切换都不影响。
fn handle_main_window_event(window: &tauri::Window, event: &tauri::WindowEvent) {
    if window.label() == "main" {
        if let tauri::WindowEvent::CloseRequested { .. } = event {
            // 显式销毁托盘图标（review issue #5），不依赖 OS 进程退出回收。
            let _ = window
                .app_handle()
                .remove_tray_by_id(crate::tray::MAIN_TRAY_ID);
            window.app_handle().exit(0);
        }
    }
}

/// 应用启动初始化：清理 staging 残留、设置窗口阴影、构建托盘。
fn setup_app(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // 启动时清理上次崩溃留下的 staging 残留（每次安装事务在
    // <app_cache>/staging/install-<job_id>/ 下解压）。
    if let Ok(cache_dir) = app.path().app_cache_dir() {
        if let Err(error) = download::install::cleanup_stale_staging(&cache_dir.join("staging")) {
            eprintln!("failed to cleanup stale staging: {error}");
        }
    }

    // 启动时清理安装事务崩溃残留的 rollback/replacement/restore-failed 目录。
    // 这些目录在客户端 install_dir 的同级创建，崩溃后永久残留，需按已注册客户端的
    // 安装目录父级扫描清理。与 staging 清理同样在启动时（无活跃安装任务）执行。
    cleanup_stale_install_artifacts_for_registered_clients(app);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_shadow(true);
    }

    tray::setup_tray(app)?;

    println!("DDNet Manager shell initialized.");
    Ok(())
}

/// 清理已注册客户端安装目录下的崩溃残留（rollback/replacement/restore-failed）。
///
/// `cleanup_stale_staging` 只清理 `<cache>/staging/install-*` 下载解压残留；
/// 安装事务在 install_dir 同级创建的 `*.ddnet-manager-rollback-*`、
/// `*.ddnet-manager-replacement[.app]`、`*.ddnet-manager-restore-failed` 目录
/// 需要按已注册客户端的 install_dir 父级扫描清理。父级去重避免同一目录下多个
/// 客户端重复扫描。失败只记录不中断启动。
///
/// `protected_paths` 由所有 `Completed` 状态安装历史的 `rollback_path` 组装，
/// 让 cleanup 跳过成功安装后保留的可回滚 rollback 目录，避免清掉用户主动
/// rollback IPC 所需的磁盘材料。
fn cleanup_stale_install_artifacts_for_registered_clients(app: &tauri::App) {
    let Some(registry) = app.try_state::<registry::ClientRegistry>() else {
        return;
    };
    let installations = match registry.list_client_installations() {
        Ok(list) => list,
        Err(error) => {
            eprintln!("failed to list client installations for cleanup: {error}");
            return;
        }
    };
    let protected_paths = collect_protected_rollback_paths(&registry, &installations);
    let mut parent_dirs = std::collections::HashSet::new();
    for installation in installations {
        if let Some(parent) = std::path::Path::new(&installation.install_dir).parent() {
            parent_dirs.insert(parent.to_path_buf());
        }
    }
    for parent_dir in parent_dirs {
        // 跳过盘符根/UNC 根等过宽目录，避免在系统关键路径上扫描。
        // install_dir 一般在多层子目录下，parent 不会是根；若用户的 install_dir
        // 直接挂在盘符根下（如 D:\QmClient），其崩溃残留需手动清理。
        if parent_dir.parent().is_none() {
            eprintln!("skip cleanup for root parent dir: {}", parent_dir.display());
            continue;
        }
        if let Err(error) =
            download::install::cleanup_stale_install_artifacts(&parent_dir, &protected_paths)
        {
            eprintln!(
                "failed to cleanup stale install artifacts at {}: {error}",
                parent_dir.display()
            );
        }
    }
}

/// 收集所有 `Completed` 状态安装历史的 `rollback_path`，作为 cleanup 的受保护集合。
///
/// 跨客户端聚合：rollback IPC 通过 history_id 反查，与 client_installation 无强耦合，
/// 因此一次性把所有客户端的 Completed rollback_path 都加入 protected 集合，让
/// `cleanup_stale_install_artifacts` 跳过它们。
fn collect_protected_rollback_paths(
    registry: &registry::ClientRegistry,
    installations: &[models::ClientInstallation],
) -> std::collections::HashSet<std::path::PathBuf> {
    let mut protected = std::collections::HashSet::new();
    for installation in installations {
        let Ok(history) = registry.list_install_history(&installation.id) else {
            continue;
        };
        for record in history {
            if !matches!(record.status, models::InstallHistoryStatus::Completed) {
                continue;
            }
            if let Some(path) = record.rollback_path {
                protected.insert(std::path::PathBuf::from(path));
            }
        }
    }
    protected
}

fn app_data_dir_for_main() -> std::path::PathBuf {
    let exe_dir = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("failed to get current exe path: {error}");
            std::process::exit(1);
        }
    };
    let exe_parent = match exe_dir.parent() {
        Some(dir) => dir.to_path_buf(),
        None => {
            eprintln!("exe path must have a parent directory");
            std::process::exit(1);
        }
    };
    let portable_marker = exe_parent.join(".portable");
    if portable_marker.exists() {
        return exe_parent.join("data");
    }
    match dirs::data_dir() {
        Some(dir) => dir,
        None => {
            eprintln!("failed to resolve system app data directory");
            std::process::exit(1);
        }
    }
}
