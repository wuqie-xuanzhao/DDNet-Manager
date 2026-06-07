#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// DDNet Manager 的领域模型定义。
pub mod models;

/// 客户端目录扫描与安装验证。
pub mod client_scan;

/// 内置 DDNet 兼容客户端目录。
pub mod client_catalog;

/// 客户端进程识别与启动。
pub mod process;

/// 更新 manifest 拉取与解析。
pub mod manifest;

/// 按客户端 catalog 分派更新来源。
pub mod update_source;

/// GitHub Release 更新源适配器。
pub mod github_release;

/// DDNet 官方下载页与 sha256sums 适配器。
pub mod ddnet_source;

/// cfg bind 解析能力。
pub mod cfg;

/// Workshop 公开 JSON 拉取与适配能力。
pub mod workshop;

/// 下载校验与下载事务基础能力。
pub mod download;

/// 网络路由候选选择与探测结果聚合。
pub mod network_route;

/// 客户端注册表持久化能力。
pub mod registry;

/// Manager 专用 cfg 文本生成与文件事务基础能力。
pub mod file_tx;

/// 版本号比较与更新判断。
pub mod version;

mod commands;

use tauri::Manager;

fn main() {
    let run_result = tauri::Builder::default()
        .manage(download::DownloadManager::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_shadow(true);
            }

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
            commands::list_install_history,
            commands::launch_client,
            commands::launch_default_client,
            commands::is_client_running,
            commands::load_manifest,
            commands::check_client_update,
            commands::start_update_download,
            commands::cancel_download,
            commands::get_download_job,
            commands::install_downloaded_update
        ])
        .run(tauri::generate_context!());

    if let Err(error) = run_result {
        eprintln!("failed to run DDNet Manager Tauri application: {error}");
        std::process::exit(1);
    }
}
