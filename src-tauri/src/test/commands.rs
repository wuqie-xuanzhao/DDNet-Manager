use crate::commands::{
    complete_download_job_snapshot, enter_installing_snapshot,
    list_download_job_recoveries_from_registry, load_download_job_snapshot,
    local_smoke_result_temp_path, request_requires_manifest_url, required_local_smoke_result_path,
    required_manifest_url, write_local_smoke_result_report,
};
use crate::download::{sha256_hex, DownloadManager};
use crate::models::{
    CheckClientUpdateRequest, DownloadJob, DownloadJobStatus, LocalSmokeResultReport,
    LocalSmokeResultStatus,
};
use crate::registry::ClientRegistry;
use std::ffi::OsString;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

fn with_local_smoke_result_path_env<T>(value: Option<&str>, callback: impl FnOnce() -> T) -> T {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let guard = ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("smoke result env lock 应可获取");
    let previous = std::env::var_os("DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH");
    let _restore = LocalSmokeResultPathEnvRestore {
        previous,
        _guard: guard,
    };

    match value {
        Some(path) => std::env::set_var("DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH", path),
        None => std::env::remove_var("DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH"),
    }

    callback()
}

struct LocalSmokeResultPathEnvRestore<'a> {
    previous: Option<OsString>,
    _guard: std::sync::MutexGuard<'a, ()>,
}

impl Drop for LocalSmokeResultPathEnvRestore<'_> {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            std::env::set_var("DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH", previous);
        } else {
            std::env::remove_var("DDNET_MANAGER_LOCAL_SMOKE_RESULT_PATH");
        }
    }
}

fn local_smoke_result_report(
    status: LocalSmokeResultStatus,
    stage: &str,
) -> LocalSmokeResultReport {
    LocalSmokeResultReport {
        status,
        stage: stage.to_string(),
        message: Some("smoke detail".to_string()),
    }
}

fn test_download_job(id: &str, status: DownloadJobStatus, cache_path: String) -> DownloadJob {
    DownloadJob {
        id: id.to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        version: "2.62.4".to_string(),
        asset_url:
            "https://github.com/wxj881027/QmClient/releases/download/v2.62.4/QmClient-windows.zip"
                .to_string(),
        sha256: sha256_hex(b"verified payload"),
        size: b"verified payload".len() as u64,
        status,
        downloaded_bytes: b"verified payload".len() as u64,
        cache_path,
        error: None,
    }
}

#[test]
fn required_manifest_url_rejects_missing_or_blank_input() {
    assert_eq!(
        required_manifest_url(None).expect_err("缺少 manifest 地址应被拒绝"),
        "manifest url is not configured"
    );
    assert_eq!(
        required_manifest_url(Some("   ")).expect_err("空 manifest 地址应被拒绝"),
        "manifest url is not configured"
    );
}

#[test]
fn required_manifest_url_trims_configured_input() {
    let url = required_manifest_url(Some(
        "  https://raw.githubusercontent.com/example/manifest/main/manifest.json  ",
    ))
    .expect("非空 manifest 地址应被接受");

    assert_eq!(
        url,
        "https://raw.githubusercontent.com/example/manifest/main/manifest.json"
    );
}

#[test]
fn required_local_smoke_result_path_rejects_missing_or_blank_input() {
    with_local_smoke_result_path_env(None, || {
        assert_eq!(
            required_local_smoke_result_path().expect_err("缺少结果路径应被拒绝"),
            "local smoke result path is not configured"
        );
    });

    with_local_smoke_result_path_env(Some("   "), || {
        assert_eq!(
            required_local_smoke_result_path().expect_err("空结果路径应被拒绝"),
            "local smoke result path is not configured"
        );
    });
}

#[test]
fn required_local_smoke_result_path_trims_configured_input() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let output_path = temp_dir.path().join("smoke-result.json");
    let raw_path = format!("  {}  ", output_path.display());

    with_local_smoke_result_path_env(Some(&raw_path), || {
        assert_eq!(
            required_local_smoke_result_path().expect("结果路径应解析成功"),
            output_path
        );
    });
}

#[test]
fn write_local_smoke_result_report_requires_debug_local_smoke_gate() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let output_path = temp_dir.path().join("smoke-result.json");
    let report = local_smoke_result_report(LocalSmokeResultStatus::Failed, "check");

    with_local_smoke_result_path_env(Some(&output_path.to_string_lossy()), || {
        crate::local_smoke::with_local_smoke_test_env(false, || {
            assert_eq!(
                write_local_smoke_result_report(&report).expect_err("未开启 local smoke 应被拒绝"),
                "local smoke reporting is not enabled"
            );
        });
    });
}

#[test]
fn write_local_smoke_result_report_persists_json_payload() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let output_path = temp_dir.path().join("nested").join("smoke-result.json");
    let report = local_smoke_result_report(LocalSmokeResultStatus::Succeeded, "install");

    with_local_smoke_result_path_env(Some(&output_path.to_string_lossy()), || {
        crate::local_smoke::with_local_smoke_test_env(true, || {
            write_local_smoke_result_report(&report).expect("结果文件应写入成功");
        });
    });

    let persisted = std::fs::read_to_string(&output_path).expect("结果文件应可读取");
    let restored: LocalSmokeResultReport =
        serde_json::from_str(&persisted).expect("结果 JSON 应可反序列化");
    assert_eq!(restored, report);
    assert!(Path::new(&output_path).exists());
}

#[test]
fn local_smoke_result_temp_path_stays_next_to_result_file() {
    let result_path = Path::new("nested").join("smoke-result.json");

    let temp_path = local_smoke_result_temp_path(&result_path).expect("临时路径应可生成");

    assert_eq!(temp_path.parent(), result_path.parent());
    assert_eq!(
        temp_path.file_name().and_then(|name| name.to_str()),
        Some("smoke-result.json.tmp")
    );
}

#[test]
fn manifest_url_is_optional_for_catalog_update_sources() {
    let request = CheckClientUpdateRequest {
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        manifest_url: None,
        platform: Some("windows-x86_64".to_string()),
        network_route: None,
        use_manifest_source: false,
    };

    assert!(!request_requires_manifest_url(&request));
}

#[test]
fn manifest_url_is_required_for_advanced_manifest_source() {
    let request = CheckClientUpdateRequest {
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        manifest_url: None,
        platform: Some("windows-x86_64".to_string()),
        network_route: None,
        use_manifest_source: true,
    };

    assert!(request_requires_manifest_url(&request));
}

#[test]
fn load_download_job_snapshot_restores_registry_job_into_manager() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let cache_path = temp_dir.path().join("download-verified.zip");
    std::fs::write(&cache_path, b"verified payload").expect("测试缓存文件应写入成功");

    let registry = ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let manager = DownloadManager::default();
    let job = test_download_job(
        "download-verified",
        DownloadJobStatus::Verified,
        cache_path.to_string_lossy().replace('\\', "/"),
    );
    registry
        .upsert_download_job(&job)
        .expect("下载任务应写入注册表");

    let restored = load_download_job_snapshot(&manager, &registry, &job.id)
        .expect("应能从注册表恢复下载任务")
        .expect("注册表中应存在下载任务");

    assert_eq!(restored, job);
    assert_eq!(manager.get(&job.id).expect("内存任务查询应成功"), Some(job));
}

#[test]
fn list_download_job_recoveries_builds_verified_recovery_summary() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let cache_path = temp_dir.path().join("download-verified.zip");
    std::fs::write(&cache_path, b"verified payload").expect("测试缓存文件应写入成功");

    let registry = ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let job = test_download_job(
        "download-verified",
        DownloadJobStatus::Verified,
        cache_path.to_string_lossy().replace('\\', "/"),
    );
    registry
        .upsert_download_job(&job)
        .expect("下载任务应写入注册表");

    let recoveries = list_download_job_recoveries_from_registry(&registry, Some("qmclient-main"))
        .expect("恢复列表应构建成功");

    assert_eq!(recoveries.len(), 1);
    assert_eq!(recoveries[0].job, job);
    assert!(recoveries[0].can_install);
    assert!(!recoveries[0].can_retry);
    assert_eq!(
        recoveries[0].cache_state,
        crate::models::DownloadCacheState::Verified
    );
}

#[test]
fn enter_installing_snapshot_persists_installing_job_before_file_replacement() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let cache_path = temp_dir.path().join("download-verified.zip");
    std::fs::write(&cache_path, b"verified payload").expect("测试缓存文件应写入成功");

    let registry = ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let manager = DownloadManager::default();
    let job = test_download_job(
        "download-installing",
        DownloadJobStatus::Verified,
        cache_path.to_string_lossy().replace('\\', "/"),
    );
    manager.insert(job.clone()).expect("内存任务应写入成功");
    registry
        .upsert_download_job(&job)
        .expect("下载任务应写入注册表");

    let installing =
        enter_installing_snapshot(&manager, &registry, &job.id).expect("进入安装状态应成功");

    assert_eq!(installing.status, DownloadJobStatus::Installing);
    let persisted = registry
        .download_job_by_id(&job.id)
        .expect("下载任务应可查询")
        .expect("下载任务应存在");
    assert_eq!(persisted.status, DownloadJobStatus::Installing);
}

#[test]
fn enter_installing_snapshot_rolls_back_memory_when_registry_write_fails() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let cache_path = temp_dir.path().join("download-verified.zip");
    std::fs::write(&cache_path, b"verified payload").expect("测试缓存文件应写入成功");

    let registry = ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let manager = DownloadManager::default();
    let job = test_download_job(
        "download-installing-rollback",
        DownloadJobStatus::Verified,
        cache_path.to_string_lossy().replace('\\', "/"),
    );
    manager.insert(job.clone()).expect("内存任务应写入成功");
    rusqlite::Connection::open(&db_path)
        .expect("测试注册表连接应可打开")
        .execute("DROP TABLE download_jobs", [])
        .expect("测试应能破坏下载任务表");

    let error = enter_installing_snapshot(&manager, &registry, &job.id)
        .expect_err("注册表写入失败时应返回错误");

    assert!(error.contains("failed to upsert download job"));
    let restored = manager
        .get(&job.id)
        .expect("内存任务查询应成功")
        .expect("内存任务应仍存在");
    assert_eq!(restored.status, DownloadJobStatus::Verified);
}

#[test]
fn complete_download_job_snapshot_persists_completed_before_history_followup() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let db_path = temp_dir.path().join("ddnet-manager.sqlite");
    let cache_path = temp_dir.path().join("download-verified.zip");
    std::fs::write(&cache_path, b"verified payload").expect("测试缓存文件应写入成功");

    let registry = ClientRegistry::open(&db_path).expect("注册表应打开成功");
    let manager = DownloadManager::default();
    let job = test_download_job(
        "download-completed-first",
        DownloadJobStatus::Installing,
        cache_path.to_string_lossy().replace('\\', "/"),
    );
    manager.insert(job.clone()).expect("内存任务应写入成功");
    registry
        .upsert_download_job(&job)
        .expect("下载任务应写入注册表");

    let completed =
        complete_download_job_snapshot(&manager, &registry, &job.id).expect("完成状态应先持久化");
    rusqlite::Connection::open(&db_path)
        .expect("测试注册表连接应可打开")
        .execute("DROP TABLE install_history", [])
        .expect("测试应能破坏安装历史表");
    let persisted = registry
        .download_job_by_id(&job.id)
        .expect("下载任务应可查询")
        .expect("下载任务应存在");

    assert_eq!(completed.status, DownloadJobStatus::Completed);
    assert_eq!(persisted.status, DownloadJobStatus::Completed);
}
