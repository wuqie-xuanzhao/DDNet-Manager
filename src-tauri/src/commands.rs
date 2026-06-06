use crate::models::{ClientHealth, ClientInstallation};
use serde::Serialize;
use std::path::Path;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
/// 表示更新检查命令返回给前端的版本摘要。
pub(crate) struct VersionInfo {
    current_version: String,
    latest_version: String,
    needs_update: bool,
    channel: String,
    package_size_mb: f64,
    notes: Vec<String>,
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    progress: u8,
    downloaded_mb: f64,
    total_mb: f64,
    speed_mb_s: f64,
    status: String,
}

/// 返回首版启动器使用的默认模拟客户端安装记录。
pub fn mocked_default_client() -> ClientInstallation {
    ClientInstallation {
        id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        display_name: "QmClient".to_string(),
        install_dir: "D:/Games/QmClient".to_string(),
        executable_path: "D:/Games/QmClient/DDNet.exe".to_string(),
        storage_cfg_path: "D:/Games/QmClient/storage.cfg".to_string(),
        data_dir: "D:/Games/QmClient/data".to_string(),
        user_data_dir: Some("C:/Users/Player/AppData/Roaming/DDNet".to_string()),
        version: Some("18.8.0".to_string()),
        is_default: true,
        health: ClientHealth::Ok,
    }
}

/// 返回首版启动器使用的模拟更新信息，后续会替换为 manifest 查询结果。
#[tauri::command]
pub async fn check_update() -> Result<VersionInfo, String> {
    let default_client = mocked_default_client();

    Ok(VersionInfo {
        current_version: default_client
            .version
            .unwrap_or_else(|| "18.8.0".to_string()),
        latest_version: "18.9.1".to_string(),
        needs_update: true,
        channel: "stable".to_string(),
        package_size_mb: 936.0,
        notes: vec![
            "Refined launcher asset verification pipeline.".to_string(),
            "Prepared DDNet client profile handoff metadata.".to_string(),
            "Improved mirror selection diagnostics.".to_string(),
        ],
    })
}

/// 启动模拟下载任务，并通过窗口事件持续推送下载进度。
#[tauri::command]
pub async fn start_download(app: AppHandle) -> Result<(), String> {
    tokio::spawn(async move {
        let total_mb = 936.0_f64;

        for progress in 0_u8..=100_u8 {
            let progress_ratio = f64::from(progress) / 100.0;
            let downloaded_mb = (total_mb * progress_ratio * 10.0).round() / 10.0;
            let speed_wave = f64::from((progress % 17) as u16) * 0.37;
            let speed_mb_s = ((7.8 + speed_wave) * 10.0).round() / 10.0;

            let status = if progress < 18 {
                "NEGOTIATING MIRROR"
            } else if progress < 72 {
                "STREAMING PACKAGE"
            } else if progress < 96 {
                "VERIFYING BLOCKS"
            } else {
                "FINALIZING"
            };

            let payload = DownloadProgress {
                progress,
                downloaded_mb,
                total_mb,
                speed_mb_s,
                status: status.to_string(),
            };

            if let Err(error) = app.emit_to("main", "download-progress", payload) {
                eprintln!("failed to emit download progress: {error}");
            }

            tokio::time::sleep(std::time::Duration::from_millis(85)).await;
        }
    });

    Ok(())
}

/// 接收前端启动请求，后续会替换为真实客户端进程拉起逻辑。
#[tauri::command]
pub fn launch_game() -> Result<(), String> {
    println!("DDNet Manager launch request accepted. Preparing DDNet client handoff.");
    Ok(())
}

/// 验证用户选择的客户端目录，并返回识别出的安装信息。
#[tauri::command]
pub fn validate_client_dir(path: String) -> Result<crate::models::ClientInstallation, String> {
    crate::client_scan::validate_client_dir(Path::new(&path))
}

/// 启动指定路径的客户端可执行文件。
#[tauri::command]
pub fn launch_client(path: String) -> Result<(), String> {
    crate::process::launch_executable(&path)
}

/// 从指定 URL 加载更新 manifest，并返回已校验的 manifest 内容。
#[tauri::command]
pub async fn load_manifest(
    url: String,
    proxy_base_url: Option<String>,
) -> Result<crate::models::UpdateManifest, String> {
    crate::manifest::fetch_manifest(&url, proxy_base_url.as_deref()).await
}

/// 读取并分析指定 cfg 文件中的 bind、unbind、exec 与按键冲突信息。
#[tauri::command]
pub fn analyze_cfg_file(path: String) -> Result<crate::models::CfgAnalysis, String> {
    crate::cfg::analyze_cfg_file(Path::new(&path))
}

/// 从指定 URL 加载 Workshop 公开 bind 列表。
#[tauri::command]
pub async fn load_workshop_binds(url: String) -> Result<Vec<crate::models::WorkshopBind>, String> {
    crate::workshop::fetch_workshop_binds(&url).await
}

/// 渲染 DDNet Manager 专用的 bind 配置区块文本。
#[tauri::command]
pub fn render_manager_bind_cfg(commands: Vec<String>) -> Result<String, String> {
    crate::file_tx::validate_manager_commands(&commands)?;
    Ok(crate::file_tx::render_manager_cfg(&commands))
}

#[cfg(test)]
mod tests {
    use crate::models::ClientHealth;

    #[test]
    fn mocked_default_client_returns_qmclient_with_ok_health() {
        let client = super::mocked_default_client();

        assert_eq!(client.client_id, "qmclient");
        assert_eq!(client.health, ClientHealth::Ok);
    }

    #[test]
    fn render_manager_bind_cfg_rejects_multiline_command() {
        let error = super::render_manager_bind_cfg(vec![
            "bind f1 \"say hi\"\nbind f2 \"say bye\"".to_string()
        ])
        .expect_err("multiline should fail");

        assert!(error.contains("must not contain newline"));
    }

    #[test]
    fn render_manager_bind_cfg_rejects_manager_marker_injection() {
        let error = super::render_manager_bind_cfg(vec![format!(
            "bind f1 \"echo {}\"",
            crate::file_tx::MANAGER_BEGIN
        )])
        .expect_err("marker should fail");

        assert!(error.contains("must not contain manager markers"));
    }

    #[test]
    fn render_manager_bind_cfg_returns_rendered_block_for_valid_commands() {
        let output = super::render_manager_bind_cfg(vec![
            " bind f1 \"say hi\" ".to_string(),
            "bind mouse5 \"toggle cl_showfps 0 1\"".to_string(),
        ])
        .expect("valid commands");

        assert_eq!(
            output,
            "# DDNET_MANAGER_BEGIN\n# This block is managed by DDNet Manager.\nbind f1 \"say hi\"\nbind mouse5 \"toggle cl_showfps 0 1\"\n# DDNET_MANAGER_END\n"
        );
    }
}
