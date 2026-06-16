#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

/// 网络路由候选选择与探测结果聚合。
pub mod network_route;

/// 客户端注册表持久化能力。
pub mod registry;

/// 版本号比较与更新判断。
pub mod version;

mod commands;

use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Listener, Manager,
};

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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 启动时清理上次崩溃留下的 staging 残留（每次安装事务在
            // <app_cache>/staging/install-<job_id>/ 下解压）。
            if let Ok(cache_dir) = app.path().app_cache_dir() {
                if let Err(error) =
                    download::install::cleanup_stale_staging(&cache_dir.join("staging"))
                {
                    eprintln!("failed to cleanup stale staging: {error}");
                }
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_shadow(true);
            }

            let mut tray_builder = TrayIconBuilder::new().tooltip("DDNet Manager");
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }
            let _tray = tray_builder
                .on_tray_icon_event(|tray, event| {
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } => {
                            if let Some(window) = tray.app_handle().get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        TrayIconEvent::Click {
                            button: MouseButton::Right,
                            button_state: MouseButtonState::Up,
                            position,
                            ..
                        } => {
                            let app = tray.app_handle();
                            if let (Some(tray_menu), Some(main)) = (
                                app.get_webview_window("tray-menu"),
                                app.get_webview_window("main"),
                            ) {
                                // 根据主窗口缩放因子，将托盘图标物理坐标转为逻辑坐标来定位菜单
                                if let Ok(scale) = main.scale_factor() {
                                    let logical: tauri::LogicalPosition<f64> =
                                        position.to_logical(scale);
                                    let _ = tray_menu.set_position(tauri::Position::Logical(
                                        tauri::LogicalPosition::new(
                                            logical.x - 60.0,
                                            logical.y - 130.0,
                                        ),
                                    ));
                                }
                                let _ = tray_menu.show();
                                let _ = tray_menu.set_focus();
                            }
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // Listen for tray menu actions
            let app_handle = app.handle().clone();
            app.listen("tray-menu-action", move |event| {
                // payload 可能为 JSON 序列化的 "show" (含引号) 或裸字符串 show
                match event.payload().trim_matches('"') {
                    "show" => {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app_handle.exit(0);
                    }
                    _ => {}
                }
            });

            println!("DDNet Manager shell initialized.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::validate_client_dir,
            commands::scan_client_installations,
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
            commands::check_app_update,
            commands::get_app_version
        ])
        .run(tauri::generate_context!());

    if let Err(error) = run_result {
        eprintln!("failed to run DDNet Manager Tauri application: {error}");
        std::process::exit(1);
    }
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
