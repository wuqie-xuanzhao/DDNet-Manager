#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// DDNet Manager 的领域模型定义。
pub mod models;

/// 客户端目录扫描与安装验证。
pub mod client_scan;

/// 客户端进程识别与启动。
pub mod process;

/// 更新 manifest 拉取与解析。
pub mod manifest;

/// cfg bind 解析能力。
pub mod cfg;

/// Workshop 公开 JSON 拉取与适配能力。
pub mod workshop;

/// 下载校验与下载事务基础能力。
pub mod download;

/// Manager 专用 cfg 文本生成与文件事务基础能力。
pub mod file_tx;

mod commands;

use tauri::Manager;

fn main() {
    let run_result = tauri::Builder::default()
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
            commands::check_update,
            commands::start_download,
            commands::launch_game,
            commands::validate_client_dir,
            commands::launch_client,
            commands::load_manifest,
            commands::analyze_cfg_file,
            commands::load_workshop_binds,
            commands::render_manager_bind_cfg
        ])
        .run(tauri::generate_context!());

    if let Err(error) = run_result {
        eprintln!("failed to run DDNet Manager Tauri application: {error}");
        std::process::exit(1);
    }
}
