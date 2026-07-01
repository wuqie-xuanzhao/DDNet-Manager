//! ClientRegistry 持久化层测试：安装记录 / 默认客户端 / 下载任务 / 安装历史。
//!
//! lint-WARN(C1): 文件约 607 行 > 600，存量；按项目约定 C1 仅在 > 800 行时强制
//! 拆分，当前规模未触发阈值。registry 行为覆盖面广（CRUD + 默认客户端 + 指纹 +
//! 历史回滚），拆成多文件会让共享 fixture 重复定义。

use crate::models::{
    AppSettings, ClientCompatibility, ClientConfidence, ClientHealth, ClientInstallSource,
    ClientInstallation, CompatibilityStatus, DownloadJob, DownloadJobStatus, InstallHistoryRecord,
    InstallHistoryStatus, NetworkRouteConfig, NetworkRouteMode,
};
use crate::registry::{ClientRegistry, LaunchProbeStatus};

fn test_client(id: &str, is_default: bool) -> ClientInstallation {
    ClientInstallation {
        id: id.to_string(),
        client_id: "qmclient".to_string(),
        display_name: "QmClient".to_string(),
        install_dir: format!("C:/Games/{id}"),
        executable_path: format!("C:/Games/{id}/DDNet.exe"),
        storage_cfg_path: format!("C:/Games/{id}/storage.cfg"),
        data_dir: format!("C:/Games/{id}/data"),
        user_data_dir: Some("C:/Users/Player/AppData/Roaming/DDNet".to_string()),
        version: None,
        is_default,
        health: ClientHealth::Ok,
        missing_items: Vec::new(),
        install_source: ClientInstallSource::Manual,
        confidence: ClientConfidence::Compatible,
        manager_owned: false,
        compatibility: ClientCompatibility {
            can_launch: true,
            ..ClientCompatibility::default()
        },
        upstream_url: None,
        pe_company_name: None,
        pe_product_name: None,
        pe_file_version: None,
        exe_sha256: None,
        last_scanned_at: Some("2026-06-06T12:00:00Z".to_string()),
    }
}

fn test_download_job(id: &str, status: DownloadJobStatus) -> DownloadJob {
    DownloadJob {
        id: id.to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        version: "2.62.4".to_string(),
        asset_url:
            "https://github.com/wxj881027/QmClient/releases/download/v2.62.4/QmClient-windows.zip"
                .to_string(),
        sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        size: 2048,
        status,
        downloaded_bytes: 1024,
        cache_path: format!("C:/Cache/{id}.zip"),
        error: None,
    }
}

#[test]
fn registry_upserts_lists_and_sets_single_default_client() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");

    registry
        .upsert_client_installation(&test_client("qmclient-main", true))
        .expect("第一条客户端应保存成功");
    registry
        .upsert_client_installation(&test_client("ddnet-vanilla", false))
        .expect("第二条客户端应保存成功");
    registry
        .set_default_client("ddnet-vanilla")
        .expect("默认客户端应设置成功");

    let clients = registry
        .list_client_installations()
        .expect("客户端列表应读取成功");
    let default_client = registry
        .get_default_client()
        .expect("默认客户端查询应成功")
        .expect("应存在默认客户端");

    assert_eq!(clients.len(), 2);
    assert_eq!(default_client.id, "ddnet-vanilla");
    assert_eq!(clients.iter().filter(|client| client.is_default).count(), 1);
}

#[test]
fn registry_remove_does_not_delete_files_and_clears_default() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");

    registry
        .upsert_client_installation(&test_client("qmclient-main", true))
        .expect("客户端应保存成功");
    registry
        .remove_client_installation("qmclient-main")
        .expect("客户端记录应移除成功");

    assert!(registry
        .get_default_client()
        .expect("默认客户端查询应成功")
        .is_none());
    assert!(registry
        .list_client_installations()
        .expect("客户端列表应读取成功")
        .is_empty());
}

#[test]
fn registry_get_default_client_removes_local_smoke_records() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let mut smoke_client = test_client("qmclient-smoke", true);
    smoke_client.install_dir =
        "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient"
            .to_string();
    smoke_client.executable_path =
        "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/DDNet.exe"
            .to_string();
    smoke_client.version = Some("9.9.9-smoke".to_string());

    registry
        .upsert_client_installation(&smoke_client)
        .expect("smoke 客户端应可写入旧注册表状态");

    assert!(registry
        .get_default_client()
        .expect("默认客户端查询应成功")
        .is_none());
    assert!(registry
        .list_client_installations()
        .expect("客户端列表应读取成功")
        .is_empty());
}

#[test]
fn registry_rejects_setting_local_smoke_record_as_default() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let mut smoke_client = test_client("qmclient-smoke", false);
    smoke_client.install_dir =
        "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient"
            .to_string();
    smoke_client.executable_path =
        "E:/Coding/DDNet/DDNet-Manager/tmp/tauri-update-smoke/run/client-install/QmClient/DDNet.exe"
            .to_string();

    registry
        .upsert_client_installation(&smoke_client)
        .expect("smoke 客户端可作为非默认临时记录保存");

    assert_eq!(
        registry
            .set_default_client("qmclient-smoke")
            .expect_err("smoke 临时记录不能被设为默认客户端")
            .to_string(),
        "local smoke client cannot be set as default"
    );
    assert!(registry
        .get_default_client()
        .expect("默认客户端查询应成功")
        .is_none());
}

#[test]
fn registry_normalizes_legacy_ddnet_vanilla_records() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let legacy_json = serde_json::json!({
        "id": "ddnet-legacy",
        "client_id": "ddnet_vanilla",
        "display_name": "DDNet",
        "install_dir": "D:/Games/DDNet",
        "executable_path": "D:/Games/DDNet/DDNet.exe",
        "storage_cfg_path": "D:/Games/DDNet/storage.cfg",
        "data_dir": "D:/Games/DDNet/data",
        "user_data_dir": null,
        "version": null,
        "is_default": true,
        "health": "ok",
        "last_scanned_at": "2026-06-06T12:00:00Z"
    })
    .to_string();

    registry
        .lock_conn()
        .expect("注册表连接应可获取")
        .execute(
            "INSERT INTO client_installations (
                    id, client_id, display_name, install_dir, executable_path,
                    storage_cfg_path, data_dir, user_data_dir, version, health,
                    last_scanned_at, is_default, client_json
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8, ?9, 1, ?10)",
            rusqlite::params![
                "ddnet-legacy",
                "ddnet_vanilla",
                "DDNet",
                "D:/Games/DDNet",
                "D:/Games/DDNet/DDNet.exe",
                "D:/Games/DDNet/storage.cfg",
                "D:/Games/DDNet/data",
                "\"ok\"",
                "2026-06-06T12:00:00Z",
                legacy_json
            ],
        )
        .expect("旧记录应写入成功");

    let clients = registry
        .list_client_installations()
        .expect("旧记录应读取成功");

    assert_eq!(clients[0].client_id, "ddnet");
}

#[test]
fn registry_persists_app_settings() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let settings = AppSettings {
        network_route: Some(NetworkRouteConfig {
            mode: NetworkRouteMode::LocalProxy,
            local_proxy_url: Some("http://127.0.0.1:7890".to_string()),
        }),
        scan_excluded_paths: vec!["D:/Archive".to_string()],
        scan_max_results: None,
        scan_timeout_secs: None,
        close_panel_after_launch: true,
        auto_check_updates: true,
        autostart: false,
        exit_game_show_launcher: true,
        close_behavior: "minimize_to_tray".to_string(),
        allow_silent_update: true,
        extra_trusted_hosts: Vec::new(),
        mirror_prefixes: Vec::new(),
        has_scanned_clients: false,
    };

    registry
        .save_app_settings(&settings)
        .expect("设置应持久化成功");
    let reloaded = crate::registry::ClientRegistry::open(&db_path)
        .expect("注册表应重新打开成功")
        .load_app_settings()
        .expect("设置应读取成功");

    assert_eq!(reloaded, settings);
}

#[test]
fn registry_does_not_persist_github_token_in_app_settings_json() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let settings = AppSettings {
        network_route: None,
        scan_excluded_paths: Vec::new(),
        scan_max_results: None,
        scan_timeout_secs: None,
        close_panel_after_launch: true,
        auto_check_updates: false,
        autostart: false,
        exit_game_show_launcher: true,
        close_behavior: "minimize_to_tray".to_string(),
        allow_silent_update: true,
        extra_trusted_hosts: Vec::new(),
        mirror_prefixes: Vec::new(),
        has_scanned_clients: false,
    };

    registry
        .save_app_settings(&settings)
        .expect("设置应持久化成功");

    let raw_json: String = registry
        .lock_conn()
        .expect("注册表连接应可获取")
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'settings' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("设置 JSON 应能读取");

    assert!(!raw_json.contains("github_token"));
}

#[test]
fn registry_scrubs_legacy_github_token_when_loading_app_settings() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let legacy = serde_json::json!({
        "network_route": null,
        "scan_excluded_paths": [],
        "close_panel_after_launch": true,
        "auto_check_updates": false,
        "github_token": "ghp_legacy_secret",
        "advanced_manifest_url": null
    })
    .to_string();

    registry
        .lock_conn()
        .expect("注册表连接应可获取")
        .execute(
            "INSERT INTO app_settings (key, value) VALUES ('settings', ?1)",
            rusqlite::params![legacy],
        )
        .expect("旧设置 JSON 应写入成功");

    registry.load_app_settings().expect("旧设置应读取成功");

    let raw_json: String = registry
        .lock_conn()
        .expect("注册表连接应可获取")
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'settings' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("设置 JSON 应能读取");

    assert!(!raw_json.contains("ghp_legacy_secret"));
    assert!(!raw_json.contains("github_token"));
}

#[test]
fn registry_uses_launch_setting_defaults_for_legacy_app_settings_json() {
    let legacy = serde_json::json!({
        "network_route": null,
        "scan_excluded_paths": [],
        "github_token": null,
        "advanced_manifest_url": null
    })
    .to_string();

    let settings: AppSettings =
        serde_json::from_str(&legacy).expect("旧设置 JSON 应兼容新增启动设置字段");

    assert!(settings.close_panel_after_launch);
    assert!(!settings.auto_check_updates);
}

#[test]
fn registry_persists_download_jobs() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let mut verified = test_download_job("download-verified", DownloadJobStatus::Verified);
    verified.downloaded_bytes = verified.size;
    let failed = DownloadJob {
        error: Some("download sha256 mismatch".to_string()),
        ..test_download_job("download-failed", DownloadJobStatus::Failed)
    };

    registry
        .upsert_download_job(&verified)
        .expect("已校验下载任务应持久化成功");
    registry
        .upsert_download_job(&failed)
        .expect("失败下载任务应持久化成功");

    let jobs = registry
        .list_download_jobs(Some("qmclient-main"))
        .expect("下载任务列表应读取成功");
    let by_id = registry
        .download_job_by_id("download-verified")
        .expect("单条下载任务应读取成功")
        .expect("应存在已保存的下载任务");

    assert_eq!(jobs.len(), 2);
    assert_eq!(by_id, verified);
    assert_eq!(jobs[0].client_installation_id, "qmclient-main");
}

#[test]
fn registry_removes_download_job_record() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let job = test_download_job("download-remove", DownloadJobStatus::Failed);

    registry
        .upsert_download_job(&job)
        .expect("下载任务应先保存成功");
    registry
        .remove_download_job(&job.id)
        .expect("下载任务记录应删除成功");

    assert!(registry
        .download_job_by_id(&job.id)
        .expect("删除后查询应成功")
        .is_none());
}

#[test]
fn registry_records_install_history() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let record = InstallHistoryRecord {
        id: "install-download-1".to_string(),
        job_id: "download-1".to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        version: "2.62.4".to_string(),
        asset_url:
            "https://github.com/wxj881027/QmClient/releases/download/v2.62.4/QmClient-windows.zip"
                .to_string(),
        package_kind: "zip".to_string(),
        status: InstallHistoryStatus::Completed,
        rollback_path: Some("D:/Games/QmClient.rollback".to_string()),
        error: None,
        completed_at: Some("2026-06-07T12:00:00Z".to_string()),
    };

    registry
        .record_install_history(&record)
        .expect("安装历史应持久化成功");
    let history = registry
        .list_install_history("qmclient-main")
        .expect("安装历史应读取成功");

    assert_eq!(history, vec![record]);
}

#[test]
fn registry_install_history_by_id_returns_record() {
    // 守卫 rollback IPC 反查路径：必须能按 history_id 拿到原始记录，
    // 否则 rollback_client_installation 无法校验 status / 取 rollback_path。
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let record = InstallHistoryRecord {
        id: "install-lookup-1".to_string(),
        job_id: "download-1".to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        version: "2.62.4".to_string(),
        asset_url: "https://example.com/QmClient.zip".to_string(),
        package_kind: "zip".to_string(),
        status: InstallHistoryStatus::Completed,
        rollback_path: Some(
            "D:/Games/QmClient.ddnet-manager-rollback-install-lookup-1".to_string(),
        ),
        error: None,
        completed_at: Some("2026-07-01T00:00:00Z".to_string()),
    };
    registry
        .record_install_history(&record)
        .expect("安装历史应持久化成功");

    let fetched = registry
        .install_history_by_id("install-lookup-1")
        .expect("按 ID 查询历史应成功");
    assert_eq!(fetched, Some(record));

    let missing = registry
        .install_history_by_id("install-missing")
        .expect("查询不存在的历史应返回 Ok(None)");
    assert!(missing.is_none());
}

#[test]
fn registry_update_install_history_status_marks_rolled_back() {
    // 守卫 rollback 成功后状态更新：必须把 Completed 改为 RolledBack，
    // 否则前端会继续给已回滚的安装暴露 rollback 入口。
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let record = InstallHistoryRecord {
        id: "install-rollback-1".to_string(),
        job_id: "download-1".to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        version: "2.62.4".to_string(),
        asset_url: "https://example.com/QmClient.zip".to_string(),
        package_kind: "zip".to_string(),
        status: InstallHistoryStatus::Completed,
        rollback_path: Some(
            "D:/Games/QmClient.ddnet-manager-rollback-install-rollback-1".to_string(),
        ),
        error: None,
        completed_at: Some("2026-07-01T00:00:00Z".to_string()),
    };
    registry
        .record_install_history(&record)
        .expect("安装历史应持久化成功");

    registry
        .update_install_history_status("install-rollback-1", &InstallHistoryStatus::RolledBack)
        .expect("状态更新应成功");

    let updated = registry
        .install_history_by_id("install-rollback-1")
        .expect("查询应成功")
        .expect("记录应存在");
    assert_eq!(updated.status, InstallHistoryStatus::RolledBack);
    // 其他字段不应被状态更新改动。
    assert_eq!(updated.version, "2.62.4");
    assert_eq!(
        updated.rollback_path.as_deref(),
        Some("D:/Games/QmClient.ddnet-manager-rollback-install-rollback-1")
    );

    // 更新不存在的 history_id 应返回 NotFound。
    let err = registry
        .update_install_history_status("install-missing", &InstallHistoryStatus::RolledBack)
        .expect_err("更新不存在的记录应失败");
    assert!(matches!(err, crate::error::ManagerError::NotFound(_)));
}

#[test]
fn registry_updates_launch_probe_result_on_client_json() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");

    registry
        .upsert_client_installation(&test_client("qmclient-main", true))
        .expect("客户端应保存成功");
    registry
        .record_launch_probe_result(crate::registry::LaunchProbeRecord {
            client_installation_id: "qmclient-main",
            status: LaunchProbeStatus::Verified,
            message: "进程已启动",
        })
        .expect("启动探测结果应写回成功");

    let client = registry
        .get_default_client()
        .expect("默认客户端应读取成功")
        .expect("默认客户端应存在");

    assert_eq!(client.compatibility.status, CompatibilityStatus::Verified);
    assert!(client.compatibility.launch_verified);
    assert_eq!(
        client.compatibility.last_launch_result.as_deref(),
        Some("进程已启动")
    );
}

#[test]
fn registry_records_unobserved_launch_without_downgrading_status() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let registry = crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功");

    registry
        .upsert_client_installation(&test_client("qmclient-main", true))
        .expect("客户端应保存成功");
    registry
        .record_launch_probe_result(crate::registry::LaunchProbeRecord {
            client_installation_id: "qmclient-main",
            status: LaunchProbeStatus::Unobserved,
            message: "未在限定时间内观察到进程",
        })
        .expect("启动探测结果应写回成功");

    let client = registry
        .get_default_client()
        .expect("默认客户端应读取成功")
        .expect("默认客户端应存在");

    assert_eq!(client.compatibility.status, CompatibilityStatus::Unknown);
    assert!(!client.compatibility.launch_verified);
    assert!(client.compatibility.can_launch);
    assert_eq!(
        client.compatibility.last_launch_result.as_deref(),
        Some("未在限定时间内观察到进程")
    );
}

// ===== client_fingerprints（用户下载指纹库）测试 =====

fn open_test_registry() -> ClientRegistry {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    crate::registry::ClientRegistry::open(&db_path).expect("注册表应打开成功")
    // temp_dir 在函数返回时 drop，但 SQLite 连接已打开并持锁，db 文件在 temp_dir
    // 被清理前一直可访问；测试结束连接关闭后 temp_dir 才被实际清理。
}

#[test]
fn record_and_lookup_fingerprint_roundtrip() {
    let registry = open_test_registry();
    crate::registry::fingerprints::FingerprintRecord {
        sha256: "abc123def456",
        client_id: "qmclient",
        display_name: "QmClient",
        version: Some("1.2.3"),
        company_name: Some("wxj881027"),
        product_name: Some("QmClient"),
    }
    .record_via(&registry);

    let fp = registry
        .lookup_fingerprint_by_hash("abc123def456")
        .expect("查询应成功")
        .expect("应能查到刚写入的指纹");
    assert_eq!(fp.client_id, "qmclient");
    assert_eq!(fp.display_name, "QmClient");
    assert_eq!(fp.version.as_deref(), Some("1.2.3"));
    assert_eq!(fp.company_name.as_deref(), Some("wxj881027"));
}

#[test]
fn lookup_fingerprint_is_case_insensitive() {
    let registry = open_test_registry();
    crate::registry::fingerprints::FingerprintRecord {
        sha256: "ABCDEF1234567890",
        client_id: "bestclient",
        display_name: "BestClient",
        version: Some("2.0"),
        company_name: None,
        product_name: None,
    }
    .record_via(&registry);

    // 写入是大写，查询用小写也应命中（内部统一小写存储）
    let fp = registry
        .lookup_fingerprint_by_hash("abcdef1234567890")
        .expect("查询应成功")
        .expect("大小写不敏感查询应命中");
    assert_eq!(fp.client_id, "bestclient");
}

#[test]
fn lookup_fingerprint_returns_none_for_unknown_hash() {
    let registry = open_test_registry();
    assert!(registry
        .lookup_fingerprint_by_hash("nonexistent-hash")
        .expect("查询应成功")
        .is_none());
}

#[test]
fn record_fingerprint_overwrites_same_hash() {
    let registry = open_test_registry();
    // 第一次写入 qmclient v1.0
    crate::registry::fingerprints::FingerprintRecord {
        sha256: "same-hash",
        client_id: "qmclient",
        display_name: "QmClient",
        version: Some("1.0"),
        company_name: None,
        product_name: None,
    }
    .record_via(&registry);
    // 同 hash 覆盖为 v2.0
    crate::registry::fingerprints::FingerprintRecord {
        sha256: "same-hash",
        client_id: "qmclient",
        display_name: "QmClient",
        version: Some("2.0"),
        company_name: Some("wxj881027"),
        product_name: None,
    }
    .record_via(&registry);

    let fp = registry
        .lookup_fingerprint_by_hash("same-hash")
        .expect("查询应成功")
        .expect("应查到最新版本");
    assert_eq!(fp.version.as_deref(), Some("2.0"));
    assert_eq!(fp.company_name.as_deref(), Some("wxj881027"));
}

// 辅助 trait：让 FingerprintRecord 可以链式调用 registry.record_client_fingerprint，
// 测试代码可读性更好。生产代码不需要。
trait RecordVia {
    fn record_via(&self, registry: &ClientRegistry);
}

impl RecordVia for crate::registry::fingerprints::FingerprintRecord<'_> {
    fn record_via(&self, registry: &ClientRegistry) {
        registry
            .record_client_fingerprint(crate::registry::fingerprints::FingerprintRecord {
                sha256: self.sha256,
                client_id: self.client_id,
                display_name: self.display_name,
                version: self.version,
                company_name: self.company_name,
                product_name: self.product_name,
            })
            .expect("指纹写入应成功");
    }
}
