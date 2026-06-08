use crate::download::{
    auto_install_guard, build_download_job_recovery, create_download_job, download_asset_to_file,
    extract_package_to_staging, extract_zip_to_staging, install_staged_client,
    package_kind_for_asset_url, restore_rollback, rollback_dir_for, sha256_hex,
    validate_download_url_with_hosts, verify_downloaded_file, DownloadFileRequest,
    DownloadJobRecoveryDecision, PackageKind,
};
use crate::models::{
    ClientUpdateCheck, DownloadCacheState, DownloadJob, DownloadJobStatus, UpdateAction,
    UpdateAsset, UpdateSourceKind,
};
use std::fs;
use std::io::Write;

#[test]
fn sha256_hex_matches_known_value() {
    assert_eq!(
        sha256_hex(b"ddnet-manager"),
        "739340afd53a209817636fca6d95d15abba5e236a11e49ff33e810111f00a55e"
    );
}

#[test]
fn build_download_job_recovery_marks_verified_cache_as_installable() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let bytes = b"ddnet-manager";
    let cache_path = temp_dir.path().join("download.zip");
    fs::write(&cache_path, bytes).expect("测试缓存文件应写入成功");
    let job = DownloadJob {
        id: "download-verified".to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        version: "2.62.4".to_string(),
        asset_url:
            "https://github.com/wxj881027/QmClient/releases/download/v2.62.4/QmClient-windows.zip"
                .to_string(),
        sha256: sha256_hex(bytes),
        size: bytes.len() as u64,
        status: DownloadJobStatus::Verified,
        downloaded_bytes: bytes.len() as u64,
        cache_path: cache_path.to_string_lossy().replace('\\', "/"),
        error: None,
    };

    let recovery = build_download_job_recovery(&job).expect("恢复摘要应构建成功");

    assert_eq!(recovery.cache_state, DownloadCacheState::Verified);
    assert!(recovery.can_install);
    assert!(!recovery.can_retry);
}

#[test]
fn build_download_job_recovery_marks_missing_cache_as_retryable() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let missing_path = temp_dir.path().join("missing.zip");
    let job = DownloadJob {
        id: "download-missing".to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        version: "2.62.4".to_string(),
        asset_url:
            "https://github.com/wxj881027/QmClient/releases/download/v2.62.4/QmClient-windows.zip"
                .to_string(),
        sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        size: 1024,
        status: DownloadJobStatus::Failed,
        downloaded_bytes: 128,
        cache_path: missing_path.to_string_lossy().replace('\\', "/"),
        error: Some("download interrupted".to_string()),
    };

    let recovery = build_download_job_recovery(&job).expect("缺失缓存恢复摘要应构建成功");

    assert_eq!(recovery.cache_state, DownloadCacheState::Missing);
    assert!(!recovery.can_install);
    assert!(recovery.can_retry);
}

#[test]
fn build_download_job_recovery_marks_corrupted_cache_as_retryable() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let cache_path = temp_dir.path().join("download.zip");
    fs::write(&cache_path, b"broken").expect("损坏缓存文件应写入成功");
    let job = DownloadJob {
        id: "download-corrupted".to_string(),
        client_installation_id: "qmclient-main".to_string(),
        client_id: "qmclient".to_string(),
        channel: "stable".to_string(),
        version: "2.62.4".to_string(),
        asset_url:
            "https://github.com/wxj881027/QmClient/releases/download/v2.62.4/QmClient-windows.zip"
                .to_string(),
        sha256: sha256_hex(b"ddnet-manager"),
        size: b"ddnet-manager".len() as u64,
        status: DownloadJobStatus::Failed,
        downloaded_bytes: b"broken".len() as u64,
        cache_path: cache_path.to_string_lossy().replace('\\', "/"),
        error: Some("download sha256 mismatch".to_string()),
    };

    let recovery = build_download_job_recovery(&job).expect("损坏缓存恢复摘要应构建成功");

    assert_eq!(recovery.cache_state, DownloadCacheState::Corrupted);
    assert!(!recovery.can_install);
    assert!(recovery.can_retry);
}

#[test]
fn recovery_decision_uses_verified_cache_for_install() {
    let decision = DownloadJobRecoveryDecision::from_cache_state(
        DownloadJobStatus::Verified,
        DownloadCacheState::Verified,
    );

    assert!(decision.can_install);
    assert!(!decision.can_retry);
}

#[test]
fn recovery_decision_rejects_install_for_non_verified_jobs_even_with_verified_cache() {
    for status in [
        DownloadJobStatus::Pending,
        DownloadJobStatus::Downloading,
        DownloadJobStatus::Canceled,
        DownloadJobStatus::Completed,
    ] {
        let decision = DownloadJobRecoveryDecision::from_cache_state(
            status.clone(),
            DownloadCacheState::Verified,
        );

        assert!(!decision.can_install);
        assert_eq!(decision.can_retry, status != DownloadJobStatus::Completed);
    }
}

#[test]
fn recovery_decision_allows_install_retry_for_failed_install_with_verified_cache() {
    let decision = DownloadJobRecoveryDecision::from_cache_state(
        DownloadJobStatus::Failed,
        DownloadCacheState::Verified,
    );

    assert!(decision.can_install);
    assert!(!decision.can_retry);
}

#[test]
fn verify_downloaded_file_rejects_wrong_size_and_sha256() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let path = temp_dir.path().join("download.zip");
    fs::write(&path, b"ddnet-manager").expect("测试下载文件应写入成功");

    let wrong_size = verify_downloaded_file(&path, sha256_hex(b"ddnet-manager").as_str(), 1)
        .expect_err("错误 size 应被拒绝");
    let wrong_sha = verify_downloaded_file(
        &path,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        13,
    )
    .expect_err("错误 sha256 应被拒绝");

    assert!(wrong_size.contains("download size mismatch"));
    assert!(wrong_sha.contains("download sha256 mismatch"));
}

#[test]
fn create_download_job_uses_generated_cache_file_name() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let update = ClientUpdateCheck {
        client_id: "qmclient".to_string(),
        channel: "..\\bad".to_string(),
        current_version: None,
        latest_version: "C:/escape".to_string(),
        asset: UpdateAsset {
            platform: "windows-x86_64".to_string(),
            asset_url: "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip"
                .to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            size: 1,
        },
        needs_update: true,
        source_kind: UpdateSourceKind::Manifest,
        action: UpdateAction::Download,
        action_url: None,
        message: None,
    };

    let job = create_download_job("../evil", &update, temp_dir.path());
    let cache_path = std::path::PathBuf::from(job.cache_path);

    assert_eq!(cache_path.parent(), Some(temp_dir.path()));
    assert!(cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("download-") && name.ends_with(".zip")));
}

#[test]
fn create_download_job_preserves_tar_xz_suffix_in_cache_path() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let update =
        sample_update("https://github.com/ddnet/ddnet/releases/download/v1/qmclient-linux.tar.xz");

    let job = create_download_job("qmclient-linux", &update, temp_dir.path());
    let cache_path = std::path::PathBuf::from(job.cache_path);

    assert_eq!(cache_path.parent(), Some(temp_dir.path()));
    assert!(cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("download-") && name.ends_with(".tar.xz")));
}

#[test]
fn create_download_job_preserves_dmg_suffix_in_cache_path() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let update =
        sample_update("https://github.com/ddnet/ddnet/releases/download/v1/qmclient-macos.dmg");

    let job = create_download_job("qmclient-macos", &update, temp_dir.path());
    let cache_path = std::path::PathBuf::from(job.cache_path);

    assert_eq!(cache_path.parent(), Some(temp_dir.path()));
    assert!(cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("download-") && name.ends_with(".dmg")));
}

#[test]
fn create_download_job_uses_download_suffix_for_unknown_asset_kind() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let update =
        sample_update("https://github.com/ddnet/ddnet/releases/download/v1/qmclient-installer.7z");

    let job = create_download_job("qmclient-unknown", &update, temp_dir.path());
    let cache_path = std::path::PathBuf::from(job.cache_path);

    assert_eq!(cache_path.parent(), Some(temp_dir.path()));
    assert!(cache_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("download-") && name.ends_with(".download")));
}

#[test]
fn package_kind_for_asset_url_detects_supported_suffixes() {
    assert_eq!(
        package_kind_for_asset_url(
            "https://github.com/ddnet/ddnet/releases/download/v1/qmclient.zip"
        ),
        PackageKind::Zip
    );
    assert_eq!(
        package_kind_for_asset_url(
            "https://github.com/ddnet/ddnet/releases/download/v1/qmclient-linux.tar.xz"
        ),
        PackageKind::TarXz
    );
    assert_eq!(
        package_kind_for_asset_url(
            "https://github.com/ddnet/ddnet/releases/download/v1/qmclient-macos.dmg"
        ),
        PackageKind::Dmg
    );
    assert_eq!(
        package_kind_for_asset_url(
            "https://github.com/ddnet/ddnet/releases/download/v1/qmclient-installer.7z"
        ),
        PackageKind::Unknown
    );
}

#[test]
fn auto_install_guard_accepts_manager_owned_package_kinds() {
    auto_install_guard(PackageKind::Zip).expect("zip 应支持自动安装");
    auto_install_guard(PackageKind::TarXz).expect("tar.xz 应进入 Manager-owned 安装闭环");
    auto_install_guard(PackageKind::Dmg).expect("dmg 应进入 macOS Manager-owned 安装闭环");
}

#[test]
fn failed_verified_unsupported_package_recovery_is_not_installable() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let cache_path = temp_dir.path().join("client.dmg");
    fs::write(&cache_path, b"verified dmg payload").expect("测试缓存应写入成功");
    let mut job = create_download_job(
        "qmclient-main",
        &sample_update("https://github.com/ddnet/ddnet/releases/download/v1/qmclient-macos.dmg"),
        temp_dir.path(),
    );
    job.status = DownloadJobStatus::Failed;
    job.downloaded_bytes = b"verified dmg payload".len() as u64;
    job.cache_path = cache_path.to_string_lossy().replace('\\', "/");
    job.sha256 = sha256_hex(b"verified dmg payload");
    job.size = b"verified dmg payload".len() as u64;
    job.error = Some(
        "automatic .dmg install requires macOS hdiutil and app bundle copy support".to_string(),
    );

    let recovery = build_download_job_recovery(&job).expect("恢复摘要应可构建");

    assert!(!recovery.can_install);
    assert!(!recovery.can_retry);
    assert_eq!(
        recovery.cache_state,
        crate::models::DownloadCacheState::Verified
    );
    assert!(recovery.user_message.contains("不支持自动安装"));
}

#[test]
fn extract_zip_to_staging_extracts_safe_zip() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let zip_path = temp_dir.path().join("safe.zip");
    let staging_dir = temp_dir.path().join("staging");
    write_zip(&zip_path, &[("QmClient/DDNet.exe", b"exe".as_slice())]);

    extract_zip_to_staging(&zip_path, &staging_dir).expect("安全 zip 应解压成功");

    assert_eq!(
        fs::read(staging_dir.join("QmClient").join("DDNet.exe")).expect("解压文件应可读取"),
        b"exe"
    );
}

#[test]
fn extract_zip_to_staging_rejects_path_traversal() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let zip_path = temp_dir.path().join("evil.zip");
    let staging_dir = temp_dir.path().join("staging");
    write_zip(&zip_path, &[("../evil.txt", b"nope".as_slice())]);

    let error = extract_zip_to_staging(&zip_path, &staging_dir).expect_err("路径穿越应被拒绝");

    assert!(error.contains("unsafe zip entry path"));
    assert!(!temp_dir.path().join("evil.txt").exists());
}

#[test]
fn extract_tar_xz_to_staging_extracts_safe_archive() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let archive_path = temp_dir.path().join("client.tar.xz");
    let staging_dir = temp_dir.path().join("staging");
    write_tar_xz(
        &archive_path,
        &[
            ("QmClient/DDNet", b"exe".as_slice()),
            ("QmClient/storage.cfg", b"".as_slice()),
            ("QmClient/data/mapres/.keep", b"".as_slice()),
        ],
    );

    extract_package_to_staging(&archive_path, &staging_dir, PackageKind::TarXz)
        .expect("安全 tar.xz 应解包成功");

    assert_eq!(
        fs::read(staging_dir.join("QmClient").join("DDNet")).expect("解包文件应可读取"),
        b"exe"
    );
    assert!(staging_dir
        .join("QmClient")
        .join("data")
        .join("mapres")
        .exists());
}

#[test]
fn extract_tar_xz_to_staging_rejects_path_traversal() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let archive_path = temp_dir.path().join("evil.tar.xz");
    let staging_dir = temp_dir.path().join("staging");
    write_tar_xz(&archive_path, &[("../evil.txt", b"nope".as_slice())]);

    let error = extract_package_to_staging(&archive_path, &staging_dir, PackageKind::TarXz)
        .expect_err("tar.xz 路径穿越应被拒绝");

    assert!(error.contains("unsafe tar entry path"));
    assert!(!temp_dir.path().join("evil.txt").exists());
}

#[test]
fn extract_dmg_to_staging_has_platform_specific_manager_owned_boundary() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let dmg_path = temp_dir.path().join("client.dmg");
    let staging_dir = temp_dir.path().join("staging");
    fs::write(&dmg_path, b"not-a-real-dmg").expect("测试 dmg 应写入成功");

    let result = extract_package_to_staging(&dmg_path, &staging_dir, PackageKind::Dmg);

    if cfg!(target_os = "macos") {
        let error = result.expect_err("无效 dmg 应在 macOS 挂载阶段失败");
        assert!(error.contains("failed to attach dmg") || error.contains("failed to copy app"));
    } else {
        let error = result.expect_err("非 macOS 不能执行 dmg Manager-owned 安装");
        assert_eq!(
            error,
            "automatic .dmg install requires macOS hdiutil and app bundle copy support"
        );
    }
}

#[test]
fn tar_xz_staging_can_install_with_rollback() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let archive_path = temp_dir.path().join("client.tar.xz");
    let staging_dir = temp_dir.path().join("staging");
    let install_dir = temp_dir.path().join("QmClient");
    let rollback_dir = temp_dir.path().join("rollback");
    write_tar_xz(
        &archive_path,
        &[
            ("QmClient/DDNet", b"new".as_slice()),
            ("QmClient/storage.cfg", b"".as_slice()),
            ("QmClient/data/.keep", b"".as_slice()),
        ],
    );
    create_linux_client_dir(&install_dir, b"old");

    extract_package_to_staging(&archive_path, &staging_dir, PackageKind::TarXz)
        .expect("tar.xz 应安全解包到 staging");
    let staged =
        crate::download::find_staged_client_dir(&staging_dir).expect("staging 应包含健康客户端");
    install_staged_client(&staged, &install_dir, &rollback_dir).expect("tar.xz staging 应可安装");

    assert_eq!(
        fs::read(install_dir.join("DDNet")).expect("新 Linux 可执行文件应存在"),
        b"new"
    );
    assert_eq!(
        fs::read(rollback_dir.join("DDNet")).expect("旧安装应进入回滚目录"),
        b"old"
    );
}

#[test]
fn install_staged_app_bundle_preserves_app_directory_name() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let staged_app = temp_dir.path().join("DDNet-Official.app");
    let install_app = temp_dir.path().join("DDNet-Manager.app");
    let rollback_app = temp_dir.path().join("DDNet-Manager.rollback.app");
    create_app_bundle(&staged_app, b"new");
    create_app_bundle(&install_app, b"old");

    install_staged_client(&staged_app, &install_app, &rollback_app)
        .expect(".app bundle 应可作为 Manager-owned 安装根");

    assert!(install_app
        .join("Contents")
        .join("MacOS")
        .join("DDNet")
        .exists());
    assert!(rollback_app
        .join("Contents")
        .join("MacOS")
        .join("DDNet")
        .exists());
    assert!(crate::client_scan::validate_client_dir(&install_app).is_ok());
}

#[tokio::test]
async fn download_asset_to_file_rejects_private_hosts_before_network() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let cache_path = temp_dir.path().join("download.zip");

    for url in [
        "https://localhost/file.zip",
        "https://127.0.0.1/file.zip",
        "https://10.0.0.1/file.zip",
        "https://169.254.1.1/file.zip",
        "https://[::1]/file.zip",
        "https://[fc00::1]/file.zip",
        "https://[::ffff:127.0.0.1]/file.zip",
    ] {
        let error = download_asset_to_file(
            DownloadFileRequest {
                asset_url: url,
                cache_path: &cache_path,
                expected_size: 1,
                enabled_route_hosts: &[],
            },
            |_| true,
        )
        .await
        .expect_err("私网或本机下载地址应被拒绝");

        assert_eq!(error, "download url host must be public");
    }
}

#[tokio::test]
async fn download_asset_to_file_rejects_untrusted_public_host_before_network() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let cache_path = temp_dir.path().join("download.zip");

    let error = download_asset_to_file(
        DownloadFileRequest {
            asset_url: "https://example.com/file.zip",
            cache_path: &cache_path,
            expected_size: 1,
            enabled_route_hosts: &[],
        },
        |_| true,
    )
    .await
    .expect_err("非可信 host 应被拒绝");

    assert_eq!(error, "download url host is not trusted");
}

#[test]
fn validate_download_url_accepts_enabled_route_host() {
    let enabled_hosts = vec!["proxy.invalid".to_string()];

    validate_download_url_with_hosts(
            "https://proxy.invalid/proxy/https%3A%2F%2Fgithub.com%2Fddnet%2Fddnet%2Freleases%2Fdownload%2Fv1%2Fqmclient.zip",
            &enabled_hosts,
        )
        .expect("显式启用的代理 host 应可用于下载");
}

#[test]
fn validate_download_url_allows_local_smoke_hosts_when_enabled() {
    crate::local_smoke::with_local_smoke_test_env(true, || {
        for url in [
            "http://localhost/file.zip",
            "https://127.0.0.1/file.zip",
            "http://10.0.0.1/file.zip",
            "https://169.254.1.1/file.zip",
            "http://[::1]/file.zip",
            "https://[fc00::1]/file.zip",
        ] {
            validate_download_url_with_hosts(url, &[])
                .expect("显式开启 local smoke 后应允许本地下载地址");
        }
    });
}

#[test]
fn validate_download_url_still_rejects_public_http_when_local_smoke_enabled() {
    crate::local_smoke::with_local_smoke_test_env(true, || {
        let error = validate_download_url_with_hosts("http://example.com/file.zip", &[])
            .expect_err("local smoke 开关不应放通公网 HTTP 下载地址");

        assert_eq!(error, "download url must use https");
    });
}

#[test]
fn validate_download_url_rejects_ambiguous_numeric_hosts_even_when_enabled() {
    for host in ["127.1", "2130706433", "0177.0.0.1"] {
        let enabled_hosts = vec![host.to_string()];
        let url = format!("https://{host}/file.zip");

        let error = validate_download_url_with_hosts(&url, &enabled_hosts)
            .expect_err("歧义数字 host 即使显式启用也应拒绝");

        assert_eq!(error, "download url host must be public", "{host}");
    }
}

#[test]
fn validate_download_url_accepts_github_release_redirect_host() {
    validate_download_url_with_hosts(
        "https://release-assets.githubusercontent.com/github-production-release-asset/example.zip",
        &[],
    )
    .expect("GitHub Release 资产重定向 host 应可用于直连下载");
}

#[test]
fn install_staged_client_creates_rollback_and_activates_replacement() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let staged_dir = temp_dir.path().join("staged");
    let install_dir = temp_dir.path().join("QmClient");
    let rollback_dir = temp_dir.path().join("rollback");
    create_client_dir(&staged_dir, b"new");
    create_client_dir(&install_dir, b"old");

    install_staged_client(&staged_dir, &install_dir, &rollback_dir).expect("安装应成功");

    assert_eq!(
        fs::read(install_dir.join("DDNet.exe")).expect("新安装应存在"),
        b"new"
    );
    assert_eq!(
        fs::read(rollback_dir.join("DDNet.exe")).expect("回滚点应存在"),
        b"old"
    );
}

#[test]
fn install_staged_client_keeps_existing_install_when_staging_is_unhealthy() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let staged_dir = temp_dir.path().join("staged");
    let install_dir = temp_dir.path().join("QmClient");
    let rollback_dir = temp_dir.path().join("rollback");
    fs::create_dir(&staged_dir).expect("测试 staging 目录应创建成功");
    fs::write(staged_dir.join("DDNet.exe"), b"new").expect("测试 staging 文件应写入成功");
    create_client_dir(&install_dir, b"old");

    let error = install_staged_client(&staged_dir, &install_dir, &rollback_dir)
        .expect_err("不健康 staging 应失败");

    assert!(error.contains("replacement client is not healthy"));
    assert_eq!(
        fs::read(install_dir.join("DDNet.exe")).expect("旧安装应保留"),
        b"old"
    );
    assert!(!rollback_dir.exists());
}

#[test]
fn rollback_dir_for_uses_install_parent_to_avoid_cross_volume_rename() {
    let install_dir = std::path::Path::new("D:/Games/QmClient");
    let rollback_dir = rollback_dir_for(install_dir, "install-download-1");

    assert_eq!(
        rollback_dir,
        std::path::Path::new("D:/Games/QmClient.ddnet-manager-rollback-install-download-1")
            .to_path_buf()
    );
}

#[test]
fn restore_rollback_replaces_active_install_with_rollback() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let install_dir = temp_dir.path().join("QmClient");
    let rollback_dir = temp_dir.path().join("QmClient.rollback");
    create_client_dir(&install_dir, b"new");
    create_client_dir(&rollback_dir, b"old");

    restore_rollback(&install_dir, &rollback_dir).expect("回滚应恢复旧客户端");

    assert_eq!(
        fs::read(install_dir.join("DDNet.exe")).expect("恢复后的客户端应存在"),
        b"old"
    );
    assert!(!rollback_dir.exists());
}

fn write_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("测试 zip 文件应创建成功");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    for (name, bytes) in entries {
        zip.start_file(*name, options)
            .expect("测试 zip entry 应创建成功");
        zip.write_all(bytes).expect("测试 zip entry 内容应写入成功");
    }

    zip.finish().expect("测试 zip 应写入完成");
}

fn write_tar_xz(path: &std::path::Path, entries: &[(&str, &[u8])]) {
    let file = fs::File::create(path).expect("测试 tar.xz 文件应创建成功");
    let encoder = xz2::write::XzEncoder::new(file, 6);
    let mut builder = tar::Builder::new(encoder);

    for (name, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        if name.contains("..") {
            append_raw_tar_entry(RawTarEntry {
                builder: &mut builder,
                header: &mut header,
                name,
                bytes,
            });
        } else {
            builder
                .append_data(&mut header, *name, *bytes)
                .expect("测试 tar entry 应写入成功");
        }
    }

    let encoder = builder.into_inner().expect("测试 tar 应写入完成");
    encoder.finish().expect("测试 xz 应写入完成");
}

struct RawTarEntry<'a, W: std::io::Write> {
    builder: &'a mut tar::Builder<W>,
    header: &'a mut tar::Header,
    name: &'a str,
    bytes: &'a [u8],
}

fn append_raw_tar_entry<W: std::io::Write>(entry: RawTarEntry<'_, W>) {
    let mut raw = *entry.header.as_bytes();
    let name_bytes = entry.name.as_bytes();
    raw[..name_bytes.len()].copy_from_slice(name_bytes);
    raw[148..156].fill(b' ');
    let checksum: u32 = raw.iter().map(|byte| u32::from(*byte)).sum();
    let checksum_text = format!("{checksum:06o}\0 ");
    raw[148..156].copy_from_slice(checksum_text.as_bytes());
    entry
        .builder
        .get_mut()
        .write_all(&raw)
        .expect("测试 tar raw header 应写入成功");
    entry
        .builder
        .get_mut()
        .write_all(entry.bytes)
        .expect("测试 tar raw body 应写入成功");
    let padding = (512 - (entry.bytes.len() % 512)) % 512;
    if padding > 0 {
        entry
            .builder
            .get_mut()
            .write_all(&vec![0; padding])
            .expect("测试 tar padding 应写入成功");
    }
}

fn create_client_dir(path: &std::path::Path, executable_bytes: &[u8]) {
    fs::create_dir_all(path).expect("测试客户端目录应创建成功");
    fs::write(path.join("DDNet.exe"), executable_bytes).expect("测试可执行文件应写入成功");
    fs::write(path.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
    fs::create_dir(path.join("data")).expect("测试 data 目录应创建成功");
}

fn create_linux_client_dir(path: &std::path::Path, executable_bytes: &[u8]) {
    fs::create_dir_all(path).expect("测试 Linux 客户端目录应创建成功");
    fs::write(path.join("DDNet"), executable_bytes).expect("测试 Linux 可执行文件应写入成功");
    fs::write(path.join("storage.cfg"), b"").expect("测试 storage.cfg 应写入成功");
    fs::create_dir(path.join("data")).expect("测试 data 目录应创建成功");
}

fn create_app_bundle(path: &std::path::Path, executable_bytes: &[u8]) {
    let macos_dir = path.join("Contents").join("MacOS");
    let resources_dir = path.join("Contents").join("Resources");
    fs::create_dir_all(&macos_dir).expect("测试 app bundle MacOS 目录应创建成功");
    fs::create_dir_all(&resources_dir).expect("测试 app bundle Resources 目录应创建成功");
    fs::write(macos_dir.join("DDNet"), executable_bytes).expect("测试 bundle 可执行文件应写入成功");
    fs::write(resources_dir.join("storage.cfg"), b"").expect("测试 bundle storage.cfg 应写入成功");
    fs::create_dir(resources_dir.join("data")).expect("测试 bundle data 目录应创建成功");
}

fn sample_update(asset_url: &str) -> ClientUpdateCheck {
    ClientUpdateCheck {
        client_id: "qmclient".to_string(),
        channel: "..\\bad".to_string(),
        current_version: None,
        latest_version: "C:/escape".to_string(),
        asset: UpdateAsset {
            platform: "windows-x86_64".to_string(),
            asset_url: asset_url.to_string(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            size: 1,
        },
        needs_update: true,
        source_kind: UpdateSourceKind::Manifest,
        action: UpdateAction::Download,
        action_url: None,
        message: None,
    }
}
