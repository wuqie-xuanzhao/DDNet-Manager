use super::{
    ClientCompatibility, ClientConfidence, ClientHealth, ClientInstallSource, ClientInstallation,
    CompatibilityStatus, DownloadCacheState, DownloadJob, DownloadJobRecovery, DownloadJobStatus,
};

#[test]
fn serializes_client_installation_with_snake_case_fields_and_ok_health() {
    let installation = ClientInstallation {
        id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        display_name: "QmClient".to_string(),
        install_dir: "D:/Games/QmClient".to_string(),
        executable_path: "D:/Games/QmClient/DDNet.exe".to_string(),
        storage_cfg_path: "D:/Games/QmClient/storage.cfg".to_string(),
        data_dir: "D:/Games/QmClient/data".to_string(),
        user_data_dir: Some("C:/Users/Player/AppData/Roaming/DDNet".to_string()),
        version: Some("18.9.1".to_string()),
        is_default: true,
        health: ClientHealth::Ok,
        missing_items: Vec::new(),
        install_source: ClientInstallSource::Manual,
        confidence: ClientConfidence::Compatible,
        manager_owned: false,
        compatibility: ClientCompatibility {
            status: CompatibilityStatus::Unknown,
            can_launch: true,
            launch_verified: false,
            reasons: Vec::new(),
            last_launch_result: None,
        },
        upstream_url: None,
        last_scanned_at: Some("2026-06-06T12:00:00Z".to_string()),
    };

    let serialized = serde_json::to_value(installation).expect("测试序列化应成功");

    assert_eq!(serialized["client_id"], "qmclient");
    assert_eq!(serialized["display_name"], "QmClient");
    assert_eq!(serialized["install_dir"], "D:/Games/QmClient");
    assert_eq!(serialized["executable_path"], "D:/Games/QmClient/DDNet.exe");
    assert_eq!(
        serialized["storage_cfg_path"],
        "D:/Games/QmClient/storage.cfg"
    );
    assert_eq!(serialized["data_dir"], "D:/Games/QmClient/data");
    assert_eq!(
        serialized["user_data_dir"],
        "C:/Users/Player/AppData/Roaming/DDNet"
    );
    assert_eq!(serialized["is_default"], true);
    assert_eq!(serialized["health"], "ok");
}

#[test]
fn serializes_download_job_recovery_with_snake_case_cache_state() {
    let recovery = DownloadJobRecovery {
        job: DownloadJob {
            id: "download-1".to_string(),
            client_installation_id: "qmclient-main".to_string(),
            client_id: "qmclient".to_string(),
            channel: "stable".to_string(),
            version: "2.62.4".to_string(),
            asset_url:
                "https://github.com/wxj881027/QmClient/releases/download/v2.62.4/QmClient-windows.zip"
                    .to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_string(),
            size: 1024,
            status: DownloadJobStatus::Verified,
            downloaded_bytes: 1024,
            cache_path: "D:/Cache/download-1.zip".to_string(),
            error: None,
        },
        cache_state: DownloadCacheState::Verified,
        can_install: true,
        can_retry: false,
        user_message: "缓存文件已校验，可直接安装。".to_string(),
    };

    let serialized = serde_json::to_value(recovery).expect("恢复摘要序列化应成功");

    assert_eq!(serialized["cache_state"], "verified");
    assert_eq!(serialized["can_install"], true);
    assert_eq!(serialized["can_retry"], false);
    assert_eq!(serialized["job"]["status"], "verified");
}
