//! SHA-256 校验与下载恢复摘要的测试。

use crate::download::{
    build_download_job_recovery, sha256_hex, verify_downloaded_file, DownloadJobRecoveryDecision,
};
use crate::models::{DownloadCacheState, DownloadJob, DownloadJobStatus};
use std::fs;

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

    assert!(wrong_size.to_string().contains("download size mismatch"));
    assert!(wrong_sha.to_string().contains("download sha256 mismatch"));
}

#[test]
fn failed_verified_unsupported_package_recovery_is_not_installable() {
    let temp_dir = tempfile::tempdir().expect("测试临时目录应创建成功");
    let bytes = b"ddnet-manager";
    let cache_path = temp_dir.path().join("download.zip");
    fs::write(&cache_path, bytes).expect("测试缓存文件应写入成功");
    let job: DownloadJob = DownloadJob {
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
        status: DownloadJobStatus::Failed,
        downloaded_bytes: bytes.len() as u64,
        cache_path: cache_path.to_string_lossy().replace('\\', "/"),
        error: Some(
            "automatic .dmg install requires macOS hdiutil and app bundle copy support".to_string(),
        ),
    };

    let recovery = build_download_job_recovery(&job).expect("恢复摘要应可构建");

    assert!(!recovery.can_install);
    assert!(!recovery.can_retry);
    assert_eq!(recovery.cache_state, DownloadCacheState::Verified);
    assert!(recovery.user_message.contains("不支持自动安装"));
}
